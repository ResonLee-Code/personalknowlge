//! 研究助手「运行中可随时中止」（AI_ARCHITECTURE_PLAN §9.2 / §9.7）。
//!
//! 中止按钮在忙碌时是可点的，因此中止与「正在跑的步骤」会**并发**：
//! 中止必须以磁盘上的状态为准，不能被运行中的步骤用自己那份内存快照覆盖回去，
//! 否则用户点了「中止」却看到会话又回到「已收集」，而且联网抓取还在继续。

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::research::ResearchService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::domain::research::{ResearchRequest, ResearchStatus};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("数据目录");
        let workspace = tempfile::tempdir().expect("工作区");
        let app =
            AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink)).expect("应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("中止测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }
}

/// 起一个「慢」检索源：睡一会儿再返回，好让中止落在步骤运行中。
fn spawn_slow_search_server(body: String, delay: Duration) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口");
    let address = listener.local_addr().expect("端口");
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            thread::sleep(delay);
            let _ = respond(&mut stream, &body);
        }
    });
    format!("127.0.0.1:{}", address.port())
}

fn respond(stream: &mut TcpStream, body: &str) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" {
            break;
        }
    }
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.as_bytes().len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

#[test]
fn an_aborted_session_cannot_be_advanced_further() {
    let fixture = Fixture::new();
    AiPolicyService::new(fixture.context())
        .update(UpdateAiPolicyInput { enabled: Some(true), ..Default::default() })
        .expect("打开 AI 总开关");
    NoteService::new(fixture.context())
        .create(CreateNoteInput {
            title: "Rust 异步运行时调度方案".to_string(),
            category_id: None,
            initial_content: Some("Rust 异步运行时调度的本地整理。".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("建本地笔记");

    let research = ResearchService::new(fixture.context());
    let session = research
        .start(ResearchRequest {
            topic: "Rust 异步运行时调度".to_string(),
            ..Default::default()
        })
        .expect("立项");
    research
        .abort(&session.id, Some("用户中止".to_string()))
        .expect("中止");

    // 中止之后不能再推进（状态机挡住），状态与中止原因都不变
    let error = research
        .collect(&session.id, false)
        .expect_err("已中止的会话不能继续收集");
    assert_eq!(error.code.as_str(), "INVALID_INPUT", "{}", error.message);
    let reloaded = research.get(&session.id).expect("读取会话");
    assert_eq!(reloaded.status, ResearchStatus::Aborted);
    assert_eq!(reloaded.abort_reason.as_deref(), Some("用户中止"));
}

/// 联网收集跑到一半被中止：抓取必须停下，且不再继续消耗预算。
#[test]
fn aborting_mid_collect_stops_the_network_work() {
    let fixture = Fixture::new();
    let host = spawn_slow_search_server(
        r#"{"results":[{"title":"Rust 异步运行时调度官方文档","url":"https://doc.rust-lang.org/book/","content":"调度器说明。"}]}"#
            .to_string(),
        Duration::from_millis(400),
    );
    AiPolicyService::new(fixture.context())
        .update(UpdateAiPolicyInput {
            enabled: Some(true),
            network: Some("allowNetwork".to_string()),
            confirm_network: Some(true),
            search_endpoint: Some(format!("http://{host}/search?q={{query}}&limit={{limit}}")),
            ..Default::default()
        })
        .expect("打开联网档位");
    NoteService::new(fixture.context())
        .create(CreateNoteInput {
            title: "Rust 异步运行时调度方案".to_string(),
            category_id: None,
            initial_content: Some("Rust 异步运行时调度的本地整理。".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("建本地笔记");

    let research = ResearchService::new(fixture.context());
    let session = research
        .start(ResearchRequest {
            topic: "Rust 异步运行时调度".to_string(),
            ..Default::default()
        })
        .expect("立项");

    let context = fixture.context();
    let id = session.id.clone();
    let collecting = thread::spawn(move || ResearchService::new(context).collect(&id, true));
    // 检索源要 400ms 才回应：在这之前中止
    thread::sleep(Duration::from_millis(80));
    research
        .abort(&session.id, Some("用户中止".to_string()))
        .expect("中止");
    let _ = collecting.join().expect("收集线程");

    let reloaded = research.get(&session.id).expect("读取会话");
    assert_eq!(
        reloaded.status,
        ResearchStatus::Aborted,
        "中止后状态必须是「已中止」：{}",
        reloaded.status.as_str()
    );
    assert_eq!(reloaded.abort_reason.as_deref(), Some("用户中止"));
    assert!(
        reloaded
            .steps
            .iter()
            .any(|step| step.detail.contains("收集中途被用户中止")),
        "要走「收集中途被中止」的分支（而不是把这一步跑完）：{:?}",
        reloaded.steps
    );
    assert!(
        reloaded
            .steps
            .iter()
            .all(|step| !step.detail.starts_with("本地材料")),
        "既然被中止了，就不该再写一条「收集完成」的记录：{:?}",
        reloaded.steps
    );
}
