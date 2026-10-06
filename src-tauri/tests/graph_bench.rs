//! 知识图谱与链接索引的性能基准（V1.1 M3 验收口径）。
//!
//! 默认不跑（标注 `#[ignore]`），手动执行：
//! ```bash
//! cargo test --offline --test graph_bench -- --ignored --nocapture
//! ```
//! 生成的合成工作区在临时目录中，进程结束即清理。

use std::sync::Arc;
use std::time::Instant;

use personal_knowledge_os_lib::application::category_service::CategoryService;
use personal_knowledge_os_lib::application::graph_service::GraphService;
use personal_knowledge_os_lib::application::index_service::IndexService;
use personal_knowledge_os_lib::application::link_service::LinkService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::graph::GraphQuery;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

fn note_count() -> usize {
    std::env::var("BENCH_NOTES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3000)
}
/// 每篇笔记的出链数量（形成 ~6000 条边的稠密图谱）
const LINKS_PER_NOTE: usize = 2;

fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                total += dir_size(&entry_path);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

#[test]
#[ignore = "性能基准，手动执行：cargo test --offline --test graph_bench -- --ignored --nocapture"]
fn bench_graph_with_3000_notes() {
    let data = tempfile::tempdir().expect("应用数据目录");
    let workspace: TempDir = tempfile::tempdir().expect("工作区目录");
    let root = workspace.path().to_path_buf();
    let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink)).expect("应用状态");
    WorkspaceService::new(Arc::clone(&app))
        .create("基准库", &root.to_string_lossy())
        .expect("创建工作区");
    let context = app.context().expect("工作区已打开");

    // 8 个分类，让分类过滤与配色都有真实分布
    let categories = CategoryService::new(Arc::clone(&context));
    let mut category_ids = Vec::new();
    for index in 0..8 {
        let category = categories
            .create(CreateCategoryInput {
                name: format!("基准分类{index}"),
                parent_id: None,
                create_directory: None,
            })
            .expect("创建分类");
        category_ids.push(category.id);
    }

    // 直接写文件，再走真实路径「扫描采纳 → 建索引 → 解析链接」：
    // 既避免逐篇 create 与文件监听扫描竞争（会被标记缺失），也更接近「导入一个已有目录」的场景。
    let category_dirs: Vec<String> =
        category_ids.iter().map(|_| String::new()).collect();
    let categories = CategoryService::new(Arc::clone(&context));
    let dirs: Vec<String> = categories
        .list()
        .expect("分类列表")
        .into_iter()
        .filter(|category| category.name.starts_with("基准分类"))
        .map(|category| category.dir_name)
        .collect();
    let _ = category_dirs;
    assert_eq!(dirs.len(), 8, "应有 8 个基准分类目录");

    let note_total = note_count();
    let started = Instant::now();
    for index in 0..note_total {
        let mut body = format!("# 基准笔记 {index}\n\n用于图谱与检索的合成数据。\n\n");
        for offset in 1..=LINKS_PER_NOTE {
            let target = (index + offset * 37) % note_total;
            body.push_str(&format!("- 参考 [[基准笔记 {target}]]\n"));
        }
        let directory = root.join(&dirs[index % dirs.len()]);
        std::fs::write(directory.join(format!("基准笔记 {index}.md")), body).expect("写入笔记文件");
    }
    let write_elapsed = started.elapsed();

    // 扫描采纳：文件监听与显式扫描可能并发（同一路径重复插入会触发唯一约束），
    // 因此这里按「重试 + 轮询」等待全部笔记进入库，测的是真实落地时间。
    let disk_files = context.storage.list_markdown_files().map(|files| files.len()).unwrap_or(0);
    println!("磁盘上可识别的 Markdown 文件：{disk_files}（期望 {}）", note_total + 1);
    let started = Instant::now();
    let index_service = IndexService::new(Arc::clone(&context));
    let mut last_error = String::new();
    let mut added = 0i64;
    loop {
        match index_service.scan_workspace() {
            Ok(report) => added += report.added,
            Err(error) => last_error = error.message.clone(),
        }
        let active = NoteService::new(Arc::clone(&context))
            .list(Default::default())
            .map(|result| result.total)
            .unwrap_or(0);
        if active as usize >= note_total + 1 {
            break;
        }
        assert!(
            started.elapsed().as_secs() < 60,
            "扫描超时：入库 {} 篇，active={}，最后一次错误：{}",
            added, active, last_error
        );
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    let scan_elapsed = started.elapsed();

    let notes = NoteService::new(Arc::clone(&context));
    let ids: Vec<String> = notes
        .list(Default::default())
        .expect("笔记列表")
        .items
        .into_iter()
        .map(|item| item.id)
        .collect();

    // 链接全量重建：代表最坏情况（首次打开已有目录）
    let started = Instant::now();
    let report = LinkService::new(Arc::clone(&context)).rebuild().expect("重建链接");
    let link_elapsed = started.elapsed();

    let started = Instant::now();
    let index = IndexService::new(Arc::clone(&context)).rebuild().expect("重建索引");
    let index_elapsed = started.elapsed();

    let graph = GraphService::new(Arc::clone(&context));
    let started = Instant::now();
    let all = graph.graph(GraphQuery::default()).expect("全量图谱");
    let all_elapsed = started.elapsed();

    let started = Instant::now();
    let filtered = graph
        .graph(GraphQuery {
            category_id: Some(category_ids[0].clone()),
            ..Default::default()
        })
        .expect("分类过滤");
    let filtered_elapsed = started.elapsed();

    let started = Instant::now();
    let focused = graph
        .graph(GraphQuery {
            focus_note_id: Some(ids[ids.len() / 2].clone()),
            hops: Some(2),
            ..Default::default()
        })
        .expect("2 跳聚焦");
    let focused_elapsed = started.elapsed();

    let started = Instant::now();
    let orphans_off = graph
        .graph(GraphQuery { include_orphans: Some(false), ..Default::default() })
        .expect("关闭孤立节点");
    let orphans_elapsed = started.elapsed();

    let db_bytes = std::fs::metadata(root.join(".knowledge/database.sqlite"))
        .map(|meta| meta.len())
        .unwrap_or(0);
    let knowledge_bytes = dir_size(&root.join(".knowledge"));

    println!("\n===== 知识图谱基准（{note_total} 篇笔记，每篇 {LINKS_PER_NOTE} 条出链）=====");
    println!("写入 Markdown 文件      : {:?}", write_elapsed);
    println!(
        "扫描采纳（含解析链接）  : {:?}（累计新增 {} 篇）",
        scan_elapsed, added
    );
    println!(
        "links_rebuild           : {:?}（扫描 {} 篇 / 链接 {} 条 / 已解析 {} / 未解析 {}）",
        link_elapsed, report.scanned_notes, report.links, report.resolved, report.unresolved
    );
    println!(
        "FTS 索引重建            : {:?}（已索引 {} / 共 {}）",
        index_elapsed, index.indexed_notes, index.total_notes
    );
    println!(
        "graph_get 全量          : {:?}（返回 {} 节点 / {} 边，截断={}，库内共 {} 节点 / {} 边）",
        all_elapsed,
        all.nodes.len(),
        all.edges.len(),
        all.truncated,
        all.total_nodes,
        all.total_edges
    );
    println!(
        "graph_get 分类过滤      : {:?}（{} 节点 / {} 边）",
        filtered_elapsed,
        filtered.nodes.len(),
        filtered.edges.len()
    );
    println!(
        "graph_get 2 跳聚焦      : {:?}（{} 节点 / {} 边）",
        focused_elapsed,
        focused.nodes.len(),
        focused.edges.len()
    );
    println!(
        "graph_get 关闭孤立节点  : {:?}（{} 节点 / {} 边）",
        orphans_elapsed,
        orphans_off.nodes.len(),
        orphans_off.edges.len()
    );
    println!("database.sqlite         : {:.1} MB", db_bytes as f64 / 1024.0 / 1024.0);
    println!(".knowledge 目录合计     : {:.1} MB", knowledge_bytes as f64 / 1024.0 / 1024.0);
    println!("=================================================\n");

    // 基准同时充当回归护栏：全量查询必须在 1.5s 内返回（M3 验收口径）
    assert!(
        all_elapsed.as_millis() < 1500,
        "graph_get 全量查询超过 1.5s：{:?}",
        all_elapsed
    );
    assert_eq!(all.total_nodes, note_total + 1, "欢迎笔记 + 合成笔记");
}
