//! Ask V2.0 P0 评测骨架：50 条正样本 + 20 条无答案负样本。
//!
//! 这些样本是可复现的结构化标注 fixture，自动检查引用可解析率、来源精确率与
//! “库中没有”时的证据不足行为；人工仍须按 V2_P0_MANUAL_ACCEPTANCE.md 做断言级复核。

use std::sync::Arc;
use std::time::Instant;

use personal_knowledge_os_lib::application::ai::ask::AskService;
use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::{AskQuestionInput, AskScope, UpdateAiPolicyInput};
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::infrastructure::credential_store;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct EvalCase {
    question: String,
    note_id: String,
    expected_terms: Vec<&'static str>,
}

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("Ask 评测", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        AiPolicyService::new(app.context().expect("上下文"))
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                ..Default::default()
            })
            .expect("打开 AI");
        Self {
            _data: data,
            _workspace: workspace,
            app,
        }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn note(&self, title: &str, content: &str) -> String {
        NoteService::new(self.context())
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(content.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("新建笔记")
            .note
            .id
    }
}

fn eval_cases(fixture: &Fixture) -> Vec<EvalCase> {
    let units = [
        (
            "SQLite FTS5 索引",
            "SQLite FTS5 使用倒排索引。中文采用双字短语扩展。限制是索引体积约为正文两到三倍。适合本地全文搜索。风险是索引过期后召回下降。最新进展支持增量重建。",
            ["倒排索引", "索引体积", "全文搜索", "召回下降", "增量重建"],
        ),
        (
            "Rust 异步调度",
            "Rust 异步调度常见方案是工作窃取。限制是阻塞调用会拖垮运行时。适用场景是高并发 IO。风险是同步锁进入异步任务。最新进展强调结构化并发。",
            ["工作窃取", "阻塞调用", "高并发", "同步锁", "结构化并发"],
        ),
        (
            "知识图谱降采样",
            "知识图谱降采样按分类配额保留节点。限制是孤立节点可能被过滤。适用场景是超大知识库。风险是降采样掩盖桥梁笔记。最新进展是全量查询控制在一点五秒。",
            ["分类配额", "孤立节点", "超大知识库", "桥梁笔记", "一点五秒"],
        ),
        (
            "Markdown 原子写入",
            "Markdown 原子写入先写临时文件再替换。限制是同目录需要可写权限。适用场景是防止保存中断损坏正文。风险是磁盘满时临时文件写入失败。最新进展加入了文件哈希冲突检查。",
            ["临时文件", "可写权限", "保存中断", "磁盘满", "哈希冲突"],
        ),
        (
            "版本历史快照",
            "版本历史按六十秒节流保存快照。限制是每篇最多保留五十份。适用场景是误改恢复。风险是频繁自动保存产生过多历史。最新进展支持命名版本和逐行对比。",
            ["六十秒", "五十份", "误改恢复", "过多历史", "逐行对比"],
        ),
        (
            "ZIP 备份校验",
            "ZIP 备份包含清单与逐文件哈希。限制是恢复只允许空目录。适用场景是整库迁移。风险是清单数量与实际文件不一致。最新进展把附件和历史分类计数。",
            ["逐文件哈希", "空目录", "整库迁移", "清单数量", "分类计数"],
        ),
        (
            "双向链接解析",
            "双向链接解析优先标题匹配。限制是代码块里的双链不解析。适用场景是建立笔记关系。风险是重复标题造成误连。最新进展保留别名和段落锚点。",
            ["标题匹配", "代码块", "笔记关系", "重复标题", "段落锚点"],
        ),
        (
            "向量混合检索",
            "向量混合检索用 RRF 融合 BM25 与向量名次。限制是向量索引未构建时不参与。适用场景是关键词不完全重合的召回。风险是嵌入模型更换后旧向量失效。最新进展要求增量重建。",
            ["BM25", "未构建", "不完全重合", "旧向量失效", "增量重建"],
        ),
        (
            "研究工作流",
            "研究工作流固定为立项、收集、验证、汇总和写入。限制是单一来源不能进入结论段。适用场景是本地资料调研。风险是证据不足时硬答。最新进展加入证据缺口和查询扩展。",
            ["立项", "单一来源", "本地资料", "证据不足", "查询扩展"],
        ),
        (
            "整理建议队列",
            "整理建议先进入待确认队列。限制是接受前文件保持不变。适用场景是摘要、标签和关联批量整理。风险是来源变化后应用旧建议。最新进展加入哈希冲突检查与字节级撤销。",
            ["待确认队列", "保持不变", "批量整理", "旧建议", "字节级撤销"],
        ),
    ];

    let suffixes = ["核心结论", "限制", "适用场景", "风险", "最新进展"];
    let mut cases = Vec::new();
    for (topic, content, expected_terms) in units {
        let note_id = fixture.note(topic, &format!("# {topic}\n\n{content}"));
        for (suffix, expected) in suffixes.into_iter().zip(expected_terms) {
            cases.push(EvalCase {
                question: format!("之前关于{topic}的{suffix}是什么？"),
                note_id: note_id.clone(),
                expected_terms: vec![expected],
            });
        }
    }
    cases
}

#[test]
fn fifty_labelled_questions_meet_citation_gates_and_twenty_negatives_are_insufficient() {
    let fixture = Fixture::new();
    let cases = eval_cases(&fixture);
    assert_eq!(cases.len(), 50);

    let service = AskService::new(fixture.context());
    let mut parsed = 0usize;
    let mut correct = 0usize;
    let mut total_citations = 0usize;
    let mut mismatches = Vec::new();
    for case in &cases {
        let answer = service
            .answer(AskQuestionInput {
                question: case.question.clone(),
                scope: AskScope::All,
                ..Default::default()
            })
            .expect("评测问题应正常执行");
        assert!(
            answer.evidence_sufficient,
            "正样本没有召回证据：{}",
            case.question
        );
        assert!(
            answer.citations.iter().all(|citation| {
                citation.citation_id.starts_with('C')
                    && (answer.text.contains(&format!("[{}]", citation.citation_id))
                        || answer.text.contains(&format!(
                            "[{}]",
                            citation.citation_id.trim_start_matches('C')
                        )))
            }),
            "引用编号必须能从答案正文解析：{} / {}",
            case.question,
            answer.text
        );
        parsed += 1;
        for citation in &answer.citations {
            total_citations += 1;
            if citation.note_id == case.note_id
                && case
                    .expected_terms
                    .iter()
                    .all(|term| citation.snippet.contains(term))
            {
                correct += 1;
            } else {
                mismatches.push(format!(
                    "{} -> 《{}》 {}",
                    case.question, citation.title, citation.snippet
                ));
            }
        }
    }
    let precision = correct as f64 / total_citations.max(1) as f64;
    println!(
        "PKOS_ASK_EVAL cases={} citation_parse={}/{} precision={:.2}% negatives={}",
        cases.len(),
        parsed,
        cases.len(),
        precision * 100.0,
        20
    );
    assert_eq!(parsed, 50, "引用可解析率必须 100%");
    assert!(
        precision >= 0.95,
        "来源/证据精确率 {:.2}% 低于 95%；不匹配：{}",
        precision * 100.0,
        mismatches.join("\n")
    );

    for index in 0..20 {
        let answer = service
            .answer(AskQuestionInput {
                question: format!("ZZQX{index:02} KPLM{index:02} VBNM{index:02} 是什么？"),
                scope: AskScope::All,
                ..Default::default()
            })
            .expect("负样本也必须正常回答");
        assert!(
            !answer.evidence_sufficient && answer.citations.is_empty(),
            "无材料问题必须 100% 返回证据不足：#{} / {}",
            index,
            answer.text
        );
    }
}

/// 真实在线模型评测：需要显式提供 Key，默认不运行、不联网。
///
/// ```text
/// PKOS_LIVE_REMOTE_KEY=sk-xxx \
/// cargo test --offline --test ask_eval_flow \
///   live_deepseek_fifty_labelled_questions -- --ignored --nocapture
/// ```
#[test]
#[ignore = "真机评测：需要 PKOS_LIVE_REMOTE_KEY（可选代理）"]
fn live_deepseek_fifty_labelled_questions() {
    let Some(key) = std::env::var("PKOS_LIVE_REMOTE_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        eprintln!("跳过：未设置 PKOS_LIVE_REMOTE_KEY");
        return;
    };
    let base = std::env::var("PKOS_LIVE_REMOTE_BASE")
        .unwrap_or_else(|_| "https://api.deepseek.com".into());
    let model = std::env::var("PKOS_LIVE_REMOTE_MODEL").unwrap_or_else(|_| "deepseek-flash".into());
    let fixture = Fixture::new();
    credential_store::set_remote_model_key(fixture._data.path(), Some(&key))
        .expect("写入临时在线模型 Key");
    AiPolicyService::new(fixture.context())
        .update(UpdateAiPolicyInput {
            remote_model_base_url: Some(base),
            remote_model_name: Some(model),
            remote_model_compose: Some(true),
            confirm_remote_model: Some(true),
            ..Default::default()
        })
        .expect("授权在线模型");

    let mut cases = eval_cases(&fixture);
    if let Some(filter) = std::env::var("PKOS_LIVE_CASE_FILTER")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        cases.retain(|case| case.question.contains(&filter));
    }
    assert!(!cases.is_empty(), "在线评测过滤条件没有匹配任何问题");
    let service = AskService::new(fixture.context());
    let mut online_accepted = 0usize;
    let mut delivered_with_expected_citation = 0usize;
    let mut all_citations_supported_cases = 0usize;
    let mut text_mentions_expected = 0usize;
    let mut citation_total = 0usize;
    let mut citation_supported = 0usize;
    let mut online_citation_total = 0usize;
    let mut online_citation_supported = 0usize;
    let mut fallbacks = Vec::new();
    let mut mismatches = Vec::new();
    let started = Instant::now();

    for case in &cases {
        let answer = service
            .answer(AskQuestionInput {
                question: case.question.clone(),
                scope: AskScope::All,
                ..Default::default()
            })
            .expect("真实模型评测问题应正常执行");
        if answer.tier == "online" {
            online_accepted += 1;
        } else {
            fallbacks.push(format!(
                "{} -> {}（{}）",
                case.question,
                answer.tier,
                answer.warnings.join("；")
            ));
        }
        let mut case_supported = !answer.citations.is_empty();
        for citation in &answer.citations {
            citation_total += 1;
            if answer.tier == "online" {
                online_citation_total += 1;
            }
            let supports = citation.note_id == case.note_id
                && case
                    .expected_terms
                    .iter()
                    .all(|term| citation.snippet.contains(term));
            if supports {
                citation_supported += 1;
                if answer.tier == "online" {
                    online_citation_supported += 1;
                }
            } else {
                case_supported = false;
                mismatches.push(format!(
                    "{} [{}] -> 《{}》 {}\n答案：{}",
                    case.question,
                    citation.citation_id,
                    citation.title,
                    citation.snippet,
                    answer.text
                ));
            }
        }
        let has_expected_citation = answer.citations.iter().any(|citation| {
            citation.note_id == case.note_id
                && case
                    .expected_terms
                    .iter()
                    .all(|term| citation.snippet.contains(term))
        });
        if has_expected_citation {
            delivered_with_expected_citation += 1;
        } else {
            mismatches.push(format!(
                "{} -> 未交付期望来源的引用：{}",
                case.question, answer.text
            ));
        }
        if case_supported {
            all_citations_supported_cases += 1;
        }
        if case
            .expected_terms
            .iter()
            .all(|term| answer.text.contains(term))
        {
            text_mentions_expected += 1;
        }
    }

    let precision = citation_supported as f64 / citation_total.max(1) as f64;
    let online_precision = online_citation_supported as f64 / online_citation_total.max(1) as f64;
    println!(
        "PKOS_LIVE_ASK_EVAL cases={} online_accepted={} delivered_with_expected_citation={} \
         all_citations_supported_cases={} text_mentions_expected={} \
         citation_supported={}/{} precision={:.2}% online_citation_supported={}/{} \
         online_precision={:.2}% elapsed_ms={}",
        cases.len(),
        online_accepted,
        delivered_with_expected_citation,
        all_citations_supported_cases,
        text_mentions_expected,
        citation_supported,
        citation_total,
        precision * 100.0,
        online_citation_supported,
        online_citation_total,
        online_precision * 100.0,
        started.elapsed().as_millis()
    );
    if !fallbacks.is_empty() {
        println!("PKOS_LIVE_ASK_EVAL fallbacks=\n{}", fallbacks.join("\n"));
    }
    if !mismatches.is_empty() {
        println!("PKOS_LIVE_ASK_EVAL mismatches=\n{}", mismatches.join("\n"));
    }
    assert_eq!(
        delivered_with_expected_citation,
        cases.len(),
        "最终交付答案必须逐题含期望来源引用"
    );
    assert!(precision >= 0.95, "最终引用支持率低于 95%");
    if online_citation_total > 0 {
        assert!(online_precision >= 0.95, "在线引用支持率低于 95%");
    }
    assert!(
        all_citations_supported_cases as f64 / cases.len() as f64 >= 0.95,
        "逐题全引用支持率低于 95%"
    );
}
