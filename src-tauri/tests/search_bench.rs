//! 中文分词基准（默认 1 万笔记）：索引体积、查询延迟与召回代理指标。
//!
//! 手动执行（约 2–4 分钟）：
//! ```bash
//! BENCH_NOTES=10000 cargo test --offline --test search_bench -- --ignored --nocapture
//! ```
//! 目的：在决定是否引入外部分词/预分词字段之前，先用数据回答三个问题——
//! 索引体积是否可接受、检索延迟是否可接受、现有「单字 + 双字」策略的召回到不到位。

use std::sync::Arc;
use std::time::{Duration, Instant};

use personal_knowledge_os_lib::application::category_service::CategoryService;
use personal_knowledge_os_lib::application::index_service::IndexService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

/// 每篇正文里的中文段落（混合常见双字词与四字短语）
const CJK_BODY: &str = "全文检索索引策略与查询优化：倒排索引、分词策略、召回率与精确率权衡，\
中文检索需要双字扩展，长尾词需要更好的分词；知识管理与个人知识库的关系是长期积累。";

const PROBE_COUNT: usize = 40;

fn note_count() -> usize {
    std::env::var("BENCH_NOTES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10_000)
}

fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                total += dir_size(&path);
            } else if let Ok(meta) = path.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// 稳定的十六进制短码（用于生成「独特术语」，不引入随机性便于复现）
fn code_of(index: usize) -> String {
    format!("{:04X}", (index * 2654435761) % 0xFFFF)
}

#[test]
#[ignore = "性能基准，手动执行：BENCH_NOTES=10000 cargo test --offline --test search_bench -- --ignored --nocapture"]
fn bench_search_with_10k_notes() {
    let total_notes = note_count();
    let data = tempfile::tempdir().expect("应用数据目录");
    let workspace: TempDir = tempfile::tempdir().expect("工作区目录");
    let root = workspace.path().to_path_buf();
    let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink)).expect("应用状态");
    WorkspaceService::new(Arc::clone(&app))
        .create("检索基准", &root.to_string_lossy())
        .expect("创建工作区");
    let context = app.context().expect("工作区已打开");

    // 分类目录
    let categories = CategoryService::new(Arc::clone(&context));
    let mut dirs = Vec::new();
    for index in 0..8 {
        let category = categories
            .create(CreateCategoryInput {
                name: format!("检索分类{index}"),
                parent_id: None,
                create_directory: None,
            })
            .expect("创建分类");
        dirs.push(category.dir_name);
    }

    // 写入 Markdown（直接写文件，避免逐篇写库的额外开销）
    let write_started = Instant::now();
    for index in 0..total_notes {
        let code = code_of(index);
        let body = format!(
            "# 基准笔记 {index}\n\n{CJK_BODY}\n\n- 独特术语：术语{code}\n- English token: term{code}\n- 标签线索：{{批次{}}}\n",
            index % 50
        );
        let directory = root.join(&dirs[index % dirs.len()]);
        std::fs::write(directory.join(format!("基准笔记 {index}.md")), body).expect("写入笔记");
    }
    let write_elapsed = write_started.elapsed();

    // 可选：停掉文件监听，单独测量「采纳本身」的成本（BENCH_STOP_WATCHER=1）
    if std::env::var("BENCH_STOP_WATCHER").as_deref() == Ok("1") {
        if let Ok(mut guard) = context.watcher.lock() {
            *guard = None;
            println!("已停用文件监听（BENCH_STOP_WATCHER=1）");
        }
    }

    // 扫描采纳 + 建索引
    let index_service = IndexService::new(Arc::clone(&context));
    let scan_started = Instant::now();
    // 触发一次扫描；之后只轮询计数（文件监听会在后台继续采纳），
    // 避免反复调用 scan_workspace 触发重复的全量重建
    let mut first_scan = true;
    loop {
        if first_scan {
            let one_pass = Instant::now();
            let _ = index_service.scan_workspace();
            println!("首次显式扫描（单趟）    : {:?}", one_pass.elapsed());
            first_scan = false;
        }
        let counted = context
            .db
            .with_conn(|conn| {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM notes WHERE status = 'active'",
                    [],
                    |row| row.get(0),
                )?;
                Ok(count)
            })
            .unwrap_or(0);
        if counted as usize >= total_notes + 1 {
            break;
        }
        assert!(scan_started.elapsed() < Duration::from_secs(600), "扫描超时：{counted} 篇");
        std::thread::sleep(Duration::from_millis(200));
    }
    let scan_elapsed = scan_started.elapsed();

    let rebuild_started = Instant::now();
    let status = index_service.rebuild().expect("重建索引");
    let rebuild_elapsed = rebuild_started.elapsed();

    let search = SearchService::new(Arc::clone(&context));

    // 召回代理：40 个独特术语（中文「术语XXXX」+ 英文 termXXXX），检查期望笔记是否命中/排名
    let mut recall_at_1 = 0usize;
    let mut recall_at_5 = 0usize;
    // 分写法统计：混合写法（术语XXXX）与纯英文（termXXXX）分别看召回
    let mut cjk_recall_at_5 = 0usize;
    let mut latin_recall_at_5 = 0usize;
    let mut latencies: Vec<Duration> = Vec::new();
    let mut cjk_phrase_hits = 0usize;
    let probes = PROBE_COUNT.min(total_notes);
    for probe in 0..probes {
        let index = (probe * 7919) % total_notes; // 稳定打散，覆盖不同分类
        let code = code_of(index);
        for (keyword, is_cjk) in [(format!("术语{code}"), true), (format!("term{code}"), false)] {
            let started = Instant::now();
            let response = search
                .search(SearchQuery { keyword: keyword.clone(), page_size: Some(10), ..Default::default() })
                .expect("检索");
            latencies.push(started.elapsed());
            let expected = format!("基准笔记 {index}");
            match response.hits.iter().position(|hit| hit.note.title == expected) {
                Some(0) => {
                    recall_at_1 += 1;
                    recall_at_5 += 1;
                    if is_cjk {
                        cjk_recall_at_5 += 1;
                    } else {
                        latin_recall_at_5 += 1;
                    }
                }
                Some(position) if position < 5 => {
                    recall_at_5 += 1;
                    if is_cjk {
                        cjk_recall_at_5 += 1;
                    } else {
                        latin_recall_at_5 += 1;
                    }
                }
                _ => {}
            }
        }

        // 中文短语（4 字）召回：正文里固定出现，期望所有命中里包含至少一篇基准笔记
        let started = Instant::now();
        let phrase = search
            .search(SearchQuery { keyword: "查询优化".into(), page_size: Some(10), ..Default::default() })
            .expect("中文短语检索");
        latencies.push(started.elapsed());
        if phrase.hits.iter().any(|hit| hit.note.title.starts_with("基准笔记")) {
            cjk_phrase_hits += 1;
        }
    }

    let probe_total = probes * 2;
    latencies.sort();
    let avg = latencies.iter().map(|d| d.as_micros()).sum::<u128>() / latencies.len().max(1) as u128;
    let p95 = latencies[((latencies.len() as f64 * 0.95) as usize).min(latencies.len() - 1)];

    let db_bytes = std::fs::metadata(root.join(".knowledge/database.sqlite"))
        .map(|meta| meta.len())
        .unwrap_or(0);
    let knowledge_bytes = dir_size(&root.join(".knowledge"));

    println!("\n===== 中文检索基准（{total_notes} 篇笔记）=====");
    println!("写入 Markdown           : {:?}", write_elapsed);
    println!("扫描采纳（含建索引）    : {:?}", scan_elapsed);
    println!(
        "FTS 重建                : {:?}（已索引 {} / 共 {}）",
        rebuild_elapsed, status.indexed_notes, status.total_notes
    );
    println!(
        "database.sqlite         : {:.1} MB（每篇约 {:.1} KB）",
        db_bytes as f64 / 1024.0 / 1024.0,
        db_bytes as f64 / 1024.0 / total_notes as f64
    );
    println!(".knowledge 目录合计     : {:.1} MB", knowledge_bytes as f64 / 1024.0 / 1024.0);
    println!(
        "检索延迟（含摘要高亮）  : 平均 {:.1} ms / p95 {:.1} ms（{} 次）",
        avg as f64 / 1000.0,
        p95.as_micros() as f64 / 1000.0,
        latencies.len()
    );
    println!(
        "独特术语 recall@1 / @5  : {:.1}% / {:.1}%（中文+英文各 {} 次）",
        recall_at_1 as f64 * 100.0 / probe_total as f64,
        recall_at_5 as f64 * 100.0 / probe_total as f64,
        probes
    );
    println!(
        "混合写法 术语XXXX recall@5: {:.1}%（{} 次）",
        cjk_recall_at_5 as f64 * 100.0 / probes as f64,
        probes
    );
    println!(
        "纯英文 termXXXX recall@5 : {:.1}%（{} 次）",
        latin_recall_at_5 as f64 * 100.0 / probes as f64,
        probes
    );
    println!(
        "中文四字短语命中率      : {:.1}%（“查询优化”，{} 次）",
        cjk_phrase_hits as f64 * 100.0 / probes as f64,
        probes
    );
    println!("=================================================\n");

    // 护栏：1 万笔记的检索延迟必须仍在交互区间内
    assert!(
        p95.as_millis() < 200,
        "p95 检索延迟过高：{} ms",
        p95.as_millis()
    );
    // 召回是「测量项」而不是「硬门槛」：这里只提示，避免基准因数据特征中断
    if recall_at_5 < probe_total {
        println!(
            "⚠️ recall@5 = {}/{}：混合中英文写法（术语XXXX）存在召回缺口，详见 CHANGELOG 结论",
            recall_at_5, probe_total
        );
    }
}
