//! 流式生成（Track C1）集成测试。
//!
//! 用一个本机桩服务分别模拟两种真实协议（都不需要真的装 Ollama / 连云端）：
//! - **Ollama**：`POST /api/generate` 的 `stream: true`（NDJSON，一行一个 JSON）；
//! - **在线模型**：`POST /chat/completions` 的 `stream: true`（SSE，`data:` 行）。
//!
//! 覆盖：
//! 1. 两种协议都**按块回调**（不是一次性给全文）；
//! 2. `research_compose` 会推 `ai://research-compose-delta`，且**增量**到达；
//! 3. **中途断流**（声明了更长的 Content-Length 却提前关连接）时：这一档发空串 + `failed`
//!    （**界面上不留下半截内容**），并自动降级到模板实现。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use personal_knowledge_os_lib::application::ai::local_model::OllamaModel;
use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::remote_model::RemoteModel;
use personal_knowledge_os_lib::application::ai::research::ResearchService;
use personal_knowledge_os_lib::application::ai::traits::{GenerationRequest, GenerativeModel};
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::EventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::domain::research::ResearchRequest;
use personal_knowledge_os_lib::state::AppInner;
use serde_json::Value;
use tempfile::TempDir;

/// 记录事件出口：断言"界面会收到什么"。
#[derive(Default)]
struct RecordingSink {
    events: Mutex<Vec<(String, Value)>>,
}

impl EventSink for RecordingSink {
    fn emit(&self, event: &str, payload: Value) {
        self.events.lock().expect("事件锁").push((event.to_string(), payload));
    }
}

/// 推测段增量：(providerId, text, state)。
type ComposeDelta = (String, String, String);

impl RecordingSink {
    fn compose_deltas(&self) -> Vec<ComposeDelta> {
        self.events
            .lock()
            .expect("事件锁")
            .iter()
            .filter(|(name, _)| name == "ai://research-compose-delta")
            .filter_map(|(_, payload)| {
                Some((
                    payload.get("providerId")?.as_str()?.to_string(),
                    payload.get("text")?.as_str()?.to_string(),
                    payload.get("state")?.as_str()?.to_string(),
                ))
            })
            .collect()
    }
}

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
    recorder: Arc<RecordingSink>,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let recorder = Arc::new(RecordingSink::default());
        let app =
            AppInner::new(data.path().to_path_buf(), Arc::clone(&recorder) as Arc<dyn EventSink>)
                .expect("初始化应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("流式测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app, recorder }
    }

    fn research(&self) -> ResearchService {
        ResearchService::new(self.app.context().expect("工作区已打开"))
    }

    fn note(&self, title: &str, content: &str) {
        NoteService::new(self.app.context().expect("工作区已打开"))
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(content.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("新建笔记");
    }

    /// 打开 AI、把本地模型指到桩服务，并开启「用本地模型写报告」。
    fn configure_local_model(&self, endpoint: &str, model: &str) {
        AiPolicyService::new(self.app.context().expect("工作区已打开"))
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                local_model_endpoint: Some(endpoint.to_string()),
                local_model_name: Some(model.to_string()),
                local_model_compose: Some(true),
                ..Default::default()
            })
            .expect("配置本地模型");
    }
}

enum StubMode {
    /// 正常：声明的 Content-Length 与实际一致。
    Complete,
    /// 截断：声明更长的 Content-Length 却提前关连接（模拟"传到一半断了"）。
    Truncated,
}

/// 起一个本机桩服务：对任何请求都按行回 `lines`（行间 flush + 小睡，制造真正的"分块"）。
fn spawn_lines_server(lines: Vec<String>, content_type: &'static str, mode: StubMode) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口");
    let port = listener.local_addr().expect("端口").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = serve_lines(&mut stream, &lines, content_type, &mode);
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn serve_lines(
    stream: &mut TcpStream,
    lines: &[String],
    content_type: &str,
    mode: &StubMode,
) -> std::io::Result<()> {
    // 先把请求读干净（请求行 + 头 + 体），避免客户端写阻塞
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" {
            break;
        }
        if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; length];
    if length > 0 {
        reader.read_exact(&mut body)?;
    }

    let payload = lines.join("\n");
    let declared = match mode {
        StubMode::Complete => payload.len(),
        StubMode::Truncated => payload.len() + 4096,
    };
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes())?;
    stream.flush()?;
    for line in lines {
        stream.write_all(line.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;
        thread::sleep(Duration::from_millis(15));
    }
    Ok(())
}

/// Ollama 风格的 NDJSON（**带** `done:true` 收尾）。
fn ollama_ndjson_lines() -> Vec<String> {
    vec![
        r#"{"model":"stub","response":"第一块","done":false}"#.to_string(),
        r#"{"model":"stub","response":"第二块","done":false}"#.to_string(),
        r#"{"model":"stub","response":"","done":true}"#.to_string(),
    ]
}

/// Ollama 风格的 NDJSON（**没有**收尾行：配合 Truncated 制造"断流"）。
fn ollama_ndjson_without_done() -> Vec<String> {
    vec![
        r#"{"model":"stub","response":"第一块","done":false}"#.to_string(),
        r#"{"model":"stub","response":"第二块","done":false}"#.to_string(),
    ]
}

/// OpenAI 风格的 SSE。
fn sse_lines() -> Vec<String> {
    vec![
        r#"data: {"choices":[{"delta":{"content":"甲"}}]}"#.to_string(),
        String::new(),
        r#"data: {"choices":[{"delta":{"content":"乙"}}]}"#.to_string(),
        String::new(),
        "data: [DONE]".to_string(),
        String::new(),
    ]
}

fn request(prompt: &str, allow_remote: bool) -> GenerationRequest {
    GenerationRequest {
        prompt: prompt.to_string(),
        evidence: Vec::new(),
        max_tokens: Some(64),
        allow_remote,
    }
}

#[test]
fn ollama_ndjson_streams_per_chunk() {
    let endpoint =
        spawn_lines_server(ollama_ndjson_lines(), "application/x-ndjson", StubMode::Complete);
    let model = OllamaModel::new(&endpoint, "stub:latest");

    let mut chunks: Vec<String> = Vec::new();
    let response = model
        .complete_streaming(&request("写推测段", false), &mut |delta| chunks.push(delta.to_string()))
        .expect("流式生成");

    assert_eq!(chunks, vec!["第一块".to_string(), "第二块".to_string()], "必须按块回调");
    assert_eq!(response.text, "第一块第二块");
    assert_eq!(response.model, "local.ollama:stub:latest");
}

#[test]
fn remote_sse_streams_per_chunk_and_respects_the_outbound_guard() {
    let base = spawn_lines_server(sse_lines(), "text/event-stream", StubMode::Complete);
    let model = RemoteModel::new(&format!("{base}/v1"), "stub-chat", "sk-test");

    let mut chunks: Vec<String> = Vec::new();
    let response = model
        .complete_streaming(&request("写推测段", true), &mut |delta| chunks.push(delta.to_string()))
        .expect("流式生成");
    assert_eq!(chunks, vec!["甲".to_string(), "乙".to_string()], "必须按块回调");
    assert_eq!(response.text, "甲乙");

    // 没有「允许外发」标记时，一个请求都不该发（二次把关）
    let blocked = model.complete_streaming(&request("写推测段", false), &mut |_| {});
    assert!(blocked.is_err(), "未标记允许外发时必须拒绝");
}

/// 对方忽略 `stream: true`（回的是普通 JSON）时不能判成失败——
/// 只是少了个逐字效果，不该把一个本来能用的在线模型废掉。
#[test]
fn remote_falls_back_when_the_server_ignores_streaming() {
    let body =
        vec![r#"{"choices":[{"message":{"role":"assistant","content":"非流式全文"}}]}"#.to_string()];
    let base = spawn_lines_server(body, "application/json; charset=utf-8", StubMode::Complete);
    let model = RemoteModel::new(&format!("{base}/v1"), "stub-chat", "sk-test");

    let mut chunks: Vec<String> = Vec::new();
    let response = model
        .complete_streaming(&request("写推测段", true), &mut |delta| chunks.push(delta.to_string()))
        .expect("忽略 stream 时也要能拿到内容");
    assert_eq!(response.text, "非流式全文");
    assert_eq!(chunks, vec!["非流式全文".to_string()]);
}

#[test]
fn compose_streams_incrementally_and_finalises_with_done() {
    let fixture = Fixture::new();
    fixture.note("Rust 异步调度方案对比", "主流方案是对比 tokio 的调度实现。");
    fixture.note("Rust 异步调度实现要点", "实现要点在于任务唤醒与批量窃取。");
    let endpoint =
        spawn_lines_server(ollama_ndjson_lines(), "application/x-ndjson", StubMode::Complete);
    fixture.configure_local_model(&endpoint, "stub:latest");

    let research = fixture.research();
    let session = research
        .start(ResearchRequest { topic: "Rust 异步调度".to_string(), ..Default::default() })
        .expect("立项");
    research.collect(&session.id, false).expect("收集");
    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, false).expect("汇总");

    let deltas = fixture.recorder.compose_deltas();
    assert!(deltas.len() >= 3, "至少要有开始 / 增量 / 结束：{deltas:?}");
    // 增量性：出现过"比最终文本短"的中间态（不是一次性给全文）
    assert!(
        deltas.iter().any(|(_, text, state)| state == "streaming" && text == "第一块"),
        "应当先看到半截文本：{deltas:?}"
    );
    let (done_provider, done_text, done_state) = deltas.last().cloned().expect("结束事件");
    assert_eq!(done_state, "done");
    assert_eq!(done_provider, "local.ollama:stub:latest");
    let report = composed.report.as_ref().expect("报告");
    assert_eq!(done_text, report.speculation.join("\n"), "结束事件要与报告一致");
}

#[test]
fn compose_clears_the_preview_and_falls_back_when_the_stream_breaks() {
    let fixture = Fixture::new();
    fixture.note("Rust 异步调度方案对比", "主流方案是对比 tokio 的调度实现。");
    fixture.note("Rust 异步调度实现要点", "实现要点在于任务唤醒与批量窃取。");
    let endpoint =
        spawn_lines_server(ollama_ndjson_without_done(), "application/x-ndjson", StubMode::Truncated);
    fixture.configure_local_model(&endpoint, "stub:latest");

    let research = fixture.research();
    let session = research
        .start(ResearchRequest { topic: "Rust 异步调度".to_string(), ..Default::default() })
        .expect("立项");
    research.collect(&session.id, false).expect("收集");
    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, false).expect("汇总要降级而不是失败");

    let deltas = fixture.recorder.compose_deltas();
    assert!(
        deltas.iter().any(|(provider, text, state)| provider == "local.ollama:stub:latest"
            && text.is_empty()
            && state == "failed"),
        "这一档断流后必须发**空串** + failed（不留下半截内容）：{deltas:?}"
    );
    let report = composed.report.as_ref().expect("报告");
    assert_eq!(report.composer, "template.draft", "断流后应降级到模板实现");
}
