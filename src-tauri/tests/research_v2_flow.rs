//! Research 2.0 P0：计划编辑、模板复用、同课题重跑对比与中止恢复。

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::research::ResearchService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::domain::research::{
    ResearchPlanEdit, ResearchRequest, ResearchStatus,
};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

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
            .create("Research 2.0 测试", &workspace.path().to_string_lossy())
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

    fn research(&self) -> ResearchService {
        ResearchService::new(self.context())
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

fn request(topic: &str) -> ResearchRequest {
    ResearchRequest {
        topic: topic.to_string(),
        language: Some("zh-CN".to_string()),
        source_types: vec!["local".to_string(), "official".to_string()],
        ..Default::default()
    }
}

fn spawn_search_server(queries: Arc<Mutex<Vec<String>>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定检索源");
    let port = listener.local_addr().expect("检索源地址").port();
    thread::spawn(move || {
        for stream in listener.incoming().take(3) {
            let Ok(mut stream) = stream else { break };
            let _ = handle_search_request(&mut stream, &queries);
        }
    });
    format!("127.0.0.1:{port}")
}

fn handle_search_request(
    stream: &mut TcpStream,
    queries: &Mutex<Vec<String>>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    if let Some(target) = request_line.split_whitespace().nth(1) {
        queries.lock().expect("查询记录锁").push(target.to_string());
    }
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" {
            break;
        }
    }
    let body = r#"{"results":[]}"#;
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

#[test]
fn edited_plan_can_be_saved_as_template_and_reused_for_a_new_topic() {
    let fixture = Fixture::new();
    let research = fixture.research();
    let session = research
        .start(request("Rust 异步运行时调度"))
        .expect("立项");
    let edited = research
        .update_plan(
            &session.id,
            ResearchPlanEdit {
                sub_questions: Some(vec![
                    "核心机制是什么？".to_string(),
                    "生产环境风险有哪些？".to_string(),
                ]),
                time_window_days: Some(365),
                language: Some("en-US".to_string()),
                source_types: Some(vec!["official".to_string(), "paper".to_string()]),
            },
        )
        .expect("编辑计划");
    let plan = edited.plan.expect("计划");
    assert_eq!(plan.sub_questions.len(), 2);
    assert_eq!(plan.time_window_days, 365);
    assert_eq!(plan.language, "en-US");
    assert_eq!(plan.source_types, vec!["official", "paper"]);

    let template = research
        .save_plan_template(&session.id, "技术选型调研")
        .expect("保存模板");
    assert_eq!(template.topic, "Rust 异步运行时调度");
    let reused = research
        .start(ResearchRequest {
            topic: "Go runtime 调度".to_string(),
            plan_template_id: Some(template.id),
            ..Default::default()
        })
        .expect("套用模板");
    let reused_plan = reused.plan.expect("复用计划");
    assert_eq!(reused_plan.sub_questions.len(), 2);
    assert_eq!(reused_plan.time_window_days, 365);
    assert_eq!(reused_plan.language, "en-US");
    assert!(reused_plan
        .sub_questions
        .iter()
        .all(|item| !item.question.contains("Rust 异步运行时调度")));
}

#[test]
fn rerun_compares_added_removed_sources_and_changed_claims() {
    let fixture = Fixture::new();
    fixture.note(
        "Rust 异步调度方案",
        "主流方案是 tokio 工作窃取调度，优点是吞吐高。",
    );
    fixture.note(
        "Rust 异步调度实现",
        "实现依赖任务唤醒、本地队列和批量窃取。",
    );
    let removed_id = fixture.note("Rust 异步调度旧版本", "旧版本资料讨论过时实现。");
    let research = fixture.research();
    let first = research.start(request("Rust 异步调度")).expect("首次立项");
    let custom_questions = vec![
        "工作窃取调度如何实现？".to_string(),
        "生产环境的取消与观测风险是什么？".to_string(),
    ];
    research
        .update_plan(
            &first.id,
            ResearchPlanEdit {
                sub_questions: Some(custom_questions.clone()),
                ..Default::default()
            },
        )
        .expect("编辑首次计划");
    research.collect(&first.id, false).expect("首次收集");
    research.verify(&first.id, false).expect("首次验证");
    let first = research.compose(&first.id, false).expect("首次报告");
    assert!(first.report.is_some());

    NoteService::new(fixture.context())
        .delete(&removed_id)
        .expect("删除旧来源");
    fixture.note(
        "Rust 异步调度 2026 新进展",
        "新版本方案强调结构化并发、取消安全与可观测性。",
    );
    let second = research.rerun(&first.id).expect("同课题重跑");
    assert_eq!(second.rerun_of.as_deref(), Some(first.id.as_str()));
    assert_eq!(
        second
            .plan
            .as_ref()
            .expect("重跑计划")
            .sub_questions
            .iter()
            .map(|item| item.question.clone())
            .collect::<Vec<_>>(),
        custom_questions,
        "重跑必须沿用上次手改的子问题"
    );
    research.collect(&second.id, false).expect("重跑收集");
    research.verify(&second.id, false).expect("重跑验证");
    let second = research.compose(&second.id, false).expect("重跑报告");
    let comparison = research.compare(&first.id, &second.id).expect("对比");
    assert!(!comparison.added_sources.is_empty());
    assert!(!comparison.removed_sources.is_empty());
    assert!(!comparison.changed_claims.is_empty());
}

#[test]
fn aborted_collection_can_resume_without_duplicating_finished_sources() {
    let fixture = Fixture::new();
    fixture.note("Rust 异步调度方案", "tokio 使用工作窃取调度。");
    fixture.note("Rust 异步调度实现", "任务唤醒与本地队列。");
    let research = fixture.research();
    let session = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&session.id, false).expect("首次收集");
    assert!(!collected.sources.is_empty());
    let count = collected.sources.len();
    let aborted = research
        .abort(&session.id, Some("测试恢复".to_string()))
        .expect("中止");
    assert_eq!(aborted.status, ResearchStatus::Aborted);
    let resumed = research.resume_aborted(&session.id).expect("恢复");
    assert_eq!(resumed.status, ResearchStatus::Planned);
    assert_eq!(resumed.sources.len(), count);
    let collected_again = research.collect(&session.id, false).expect("继续收集");
    assert_eq!(
        collected_again
            .sources
            .iter()
            .map(|source| source.note_id.clone().or(source.url.clone()))
            .collect::<std::collections::HashSet<_>>()
            .len(),
        collected_again.sources.len(),
        "恢复后不能重复来源"
    );
    assert_eq!(collected_again.sources.len(), count);
}

#[test]
fn source_type_plan_filters_out_local_material_when_local_is_not_requested() {
    let fixture = Fixture::new();
    fixture.note("Rust 异步调度方案", "tokio 使用工作窃取调度。");
    let research = fixture.research();
    let session = research
        .start(ResearchRequest {
            topic: "Rust 异步调度".to_string(),
            source_types: vec!["official".to_string(), "paper".to_string()],
            ..Default::default()
        })
        .expect("立项");
    let collected = research.collect(&session.id, false).expect("收集");
    assert!(
        collected.sources.is_empty(),
        "未选择 local 时不应把本地笔记塞入来源"
    );
}

#[test]
fn local_note_applicable_version_reaches_source_and_report() {
    let fixture = Fixture::new();
    let note_id = fixture.note(
        "Rust 异步调度版本说明",
        "tokio 的工作窃取调度在目标版本中可用。",
    );
    let context = fixture.context();
    let notes = NoteService::new(Arc::clone(&context));
    let detail = notes.get(&note_id).expect("读取笔记");
    let raw = context
        .storage
        .read_raw(&detail.note.file_path)
        .expect("读取原文");
    let raw = raw.replacen(
        "---\n",
        "---\napplicable_version: 1.82\n",
        1,
    );
    context
        .storage
        .write_atomic(&detail.note.file_path, &raw)
        .expect("写入适用版本");

    let research = fixture.research();
    let session = research
        .start(request("Rust 异步调度"))
        .expect("立项");
    let collected = research.collect(&session.id, false).expect("收集");
    let source = collected
        .sources
        .iter()
        .find(|source| source.note_id.as_deref() == Some(note_id.as_str()))
        .expect("本地来源");
    assert_eq!(source.applicable_version.as_deref(), Some("1.82"));

    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, false).expect("汇总");
    let report = composed.report.expect("报告");
    assert!(
        report.markdown.contains("适用版本 1.82"),
        "报告必须记录来源适用版本：{}",
        report.markdown
    );
}

#[test]
fn web_collection_uses_the_edited_plan_queries() {
    let fixture = Fixture::new();
    let queries = Arc::new(Mutex::new(Vec::new()));
    let host = spawn_search_server(Arc::clone(&queries));
    AiPolicyService::new(fixture.context())
        .update(UpdateAiPolicyInput {
            enabled: Some(true),
            network: Some("allowNetwork".to_string()),
            confirm_network: Some(true),
            search_endpoint: Some(format!("http://{host}/search?q={{query}}&limit={{limit}}")),
            ..Default::default()
        })
        .expect("打开联网检索");

    let research = fixture.research();
    let session = research.start(request("Rust 异步调度")).expect("立项");
    research
        .update_plan(
            &session.id,
            ResearchPlanEdit {
                sub_questions: Some(vec![
                    "unique-web-marker 的生产风险是什么？".to_string(),
                    "取消安全如何验证？".to_string(),
                ]),
                ..Default::default()
            },
        )
        .expect("编辑计划");
    research.collect(&session.id, true).expect("联网收集");

    let captured = queries.lock().expect("查询记录锁").join("\n");
    assert!(
        captured.contains("unique-web-marker"),
        "外部检索必须使用编辑后的计划查询，而不是只搜索课题标题：{captured}"
    );
}
