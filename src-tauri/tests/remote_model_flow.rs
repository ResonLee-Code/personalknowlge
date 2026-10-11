//! 在线模型接入（OpenAI 兼容端点）集成测试。
//!
//! 全部用**本机桩服务**假装云端（不真外发、不依赖任何外部服务），验证四件事：
//! 1. 授权后在线模型真的被调用，报告如实写明 provider，请求头里带 Key；
//! 2. **没授权就一个字节都不发**（不是靠调用时记得判断，而是根本不构造 provider）；
//! 3. 在线失败 → 降级到本地模型 → 再失败 → 模板实现，流程不中断；
//! 4. API Key 不进工作区、不出现在任何响应与审计里（只回显「配没配」）。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::research::ResearchService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::domain::research::{ResearchRequest, WarningKind};
use personal_knowledge_os_lib::infrastructure::credential_store;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

const API_KEY: &str = "sk-test-secret-key";

struct Fixture {
    data: TempDir,
    workspace: TempDir,
    app: Arc<AppInner>,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("在线模型测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { data, workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn policy(&self) -> AiPolicyService {
        AiPolicyService::new(self.context())
    }

    fn note(&self, title: &str, content: &str) {
        NoteService::new(self.context())
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(content.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("新建笔记");
    }

    /// 配置在线模型（`authorize` 决定是否走「首次确认」这一步）。
    fn configure_remote(&self, base: &str, authorize: bool, compose: bool, score: bool) {
        credential_store::set_remote_model_key(self.data.path(), Some(API_KEY)).expect("写凭据");
        self.policy()
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                remote_model_base_url: Some(base.to_string()),
                remote_model_name: Some("stub-model".to_string()),
                remote_model_compose: Some(compose),
                remote_model_score: Some(score),
                confirm_remote_model: authorize.then_some(true),
                ..Default::default()
            })
            .expect("配置在线模型");
    }

    fn configure_local(&self, endpoint: &str, compose: bool, score: bool) {
        self.policy()
            .update(UpdateAiPolicyInput {
                local_model_endpoint: Some(endpoint.to_string()),
                local_model_name: Some("stub:latest".to_string()),
                local_model_compose: Some(compose),
                local_model_score: Some(score),
                ..Default::default()
            })
            .expect("配置本地模型");
    }
}

/// 桩服务的可观测信息：被调用次数 + 每次请求带的 `Authorization` 头。
struct Stub {
    endpoint: String,
    /// 同一台桩服务的根地址（本地 Ollama 形状的接口挂在 `/api/generate` 下）
    root: String,
    calls: Arc<AtomicUsize>,
    auth_headers: Arc<Mutex<Vec<String>>>,
}

impl Stub {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn authorizations(&self) -> Vec<String> {
        self.auth_headers.lock().expect("锁").clone()
    }

    fn local_endpoint(&self) -> String {
        self.root.clone()
    }
}

/// 一个「假装 OpenAI 兼容端点」的本机服务：读请求体，按提示词类型返回打分 JSON 或普通文本。
fn spawn_remote_server(text_reply: &'static str) -> Stub {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口");
    let port = listener.local_addr().expect("端口").port();
    let calls = Arc::new(AtomicUsize::new(0));
    let auth_headers: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let counter = Arc::clone(&calls);
    let seen = Arc::clone(&auth_headers);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = handle(&mut stream, text_reply, &seen);
            let _ = counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    Stub {
        endpoint: format!("http://localhost:{port}/v1"),
        root: format!("http://localhost:{port}"),
        calls,
        auth_headers,
    }
}

fn handle(
    stream: &mut TcpStream,
    text_reply: &str,
    seen: &Arc<Mutex<Vec<String>>>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    // 同一台桩服务同时充当「在线模型的 OpenAI 兼容端点」与「本地的 Ollama」：
    // 按路径分辨该用哪种响应形状，这样才能在同一测试里验证「在线失败 → 降级到本地」。
    let ollama_shape = request_line.contains("/api/generate");
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" {
            break;
        }
        let lowered = header.to_ascii_lowercase();
        if let Some(value) = lowered.strip_prefix("authorization:") {
            seen.lock().expect("锁").push(value.trim().to_string());
        }
        if let Some(value) = lowered.strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; length];
    if length > 0 {
        reader.read_exact(&mut body)?;
    }
    let body = String::from_utf8_lossy(&body).to_string();

    // 打分请求里带 `- id=<候选 id>`：把所有出现过的 id 回填（分数 1.0），不依赖固定 id
    let content = if body.contains("只输出 JSON") {
        let ids: Vec<String> = body
            .match_indices("- id=")
            .filter_map(|(index, _)| {
                let rest = &body[index + 5..];
                let end = rest
                    .find(|ch: char| ch.is_whitespace() || ch == '"' || ch == '\\')
                    .unwrap_or(rest.len());
                let id = rest[..end].to_string();
                if id.is_empty() {
                    None
                } else {
                    Some(id)
                }
            })
            .collect();
        let scores: Vec<serde_json::Value> = ids
            .iter()
            .map(|id| serde_json::json!({ "id": id, "score": 1.0, "reason": "桩模型：直接相关" }))
            .collect();
        serde_json::json!({ "scores": scores }).to_string()
    } else {
        text_reply.to_string()
    };
    let payload = if ollama_shape {
        serde_json::json!({ "model": "stub:latest", "response": content }).to_string()
    } else {
        serde_json::json!({
            "choices": [{ "message": { "role": "assistant", "content": content } }]
        })
        .to_string()
    };
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.as_bytes().len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn request(topic: &str) -> ResearchRequest {
    ResearchRequest { topic: topic.to_string(), ..Default::default() }
}

fn seed_notes(fixture: &Fixture) {
    fixture.note("Rust 异步调度方案对比", "主流方案是对比 tokio 的调度实现。");
    fixture.note("Rust 异步调度实现要点", "实现要点在于任务唤醒与批量窃取。");
}

#[test]
fn authorized_remote_model_scores_and_writes_the_report() {
    let fixture = Fixture::new();
    seed_notes(&fixture);
    let stub = spawn_remote_server("（桩模型）任务窃取是调度器的核心机制。");
    fixture.configure_remote(&stub.endpoint, true, true, true);

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&session.id, false).expect("收集");
    assert_eq!(
        collected.decision_provider.as_deref(),
        Some("remote.openai-compatible"),
        "打分应由在线模型给出"
    );
    assert!(stub.calls() >= 1, "授权后必须真的调用在线模型");
    // Key 只在请求头里出现，而且必须带上
    assert!(
        !stub.authorizations().is_empty()
            && stub
                .authorizations()
                .iter()
                .all(|value| value == &format!("bearer {API_KEY}")),
        "每次请求都要带 Authorization: Bearer <key>；实际：{:?}",
        stub.authorizations()
    );

    research.verify(&session.id).expect("验证");
    let composed = research.compose(&session.id).expect("汇总");
    let report = composed.report.as_ref().expect("报告");
    assert_eq!(report.decision_provider, "remote.openai-compatible");
    assert_eq!(report.composer, "remote.openai-compatible:stub-model");
    assert_eq!(
        report.speculation.first().map(String::as_str),
        Some("（桩模型）任务窃取是调度器的核心机制。")
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.kind == WarningKind::ModelGenerated),
        "必须给出「模型生成、未经核对」警告"
    );
}

#[test]
fn without_authorization_nothing_is_sent() {
    let fixture = Fixture::new();
    seed_notes(&fixture);
    let stub = spawn_remote_server("（不应该被调用）");
    // 配置齐全 + 两个开关都打开，但**没有走首次确认**
    fixture.configure_remote(&stub.endpoint, false, true, true);
    let policy = fixture.policy().policy();
    assert!(policy.remote_model_base_url.is_some(), "地址本身是合法的，应当被保存");
    assert!(policy.remote_model_key_set, "Key 已配置（只回显布尔值）");
    assert!(
        policy.remote_model_authorized_at.is_none(),
        "没有确认过就不能出现授权时间戳"
    );

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&session.id, false).expect("收集");
    assert_eq!(collected.decision_provider.as_deref(), Some("rule"), "未授权要退回规则档");
    research.verify(&session.id).expect("验证");
    let composed = research.compose(&session.id).expect("汇总");
    assert_eq!(
        composed.report.as_ref().expect("报告").composer,
        "template.draft",
        "未授权时报告只能来自模板实现"
    );
    assert_eq!(stub.calls(), 0, "未授权时一个请求都不能发出去");

    // 拒绝必须留下审计（而且是 outbound=false，如实标注「没有外发」）
    let audit = fixture.policy().audit_tail(20).expect("审计");
    assert!(
        audit
            .iter()
            .any(|entry| entry.note.as_deref() == Some("REMOTE_MODEL_UNCONFIRMED")
                && !entry.outbound),
        "未授权要记一条 outbound=false 的拒绝审计：{audit:?}"
    );
}

#[test]
fn remote_failure_falls_back_to_local_model_then_template() {
    let fixture = Fixture::new();
    seed_notes(&fixture);
    // 在线模型指向一个必然连不上的端口；同时配好本地模型桩
    fixture.configure_remote("http://localhost:9/v1", true, true, true);
    let local = spawn_remote_server("（本地桩模型）任务窃取是调度器的核心机制。");
    fixture.configure_local(&local.local_endpoint(), true, true);

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&session.id, false).expect("收集不应失败");
    assert_eq!(
        collected.decision_provider.as_deref(),
        Some("local.ollama"),
        "在线不可用要降到本地模型，而不是直接掉到规则档"
    );
    assert!(local.calls() >= 1, "降级后应当真的调用本地模型");
    assert!(collected.sources.iter().all(|source| source.credibility > 0.0));

    research.verify(&session.id).expect("验证");
    let composed = research.compose(&session.id).expect("汇总不应失败");
    let report = composed.report.as_ref().expect("报告");
    assert_eq!(report.composer, "local.ollama:stub:latest");
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.kind == WarningKind::ModelGenerated),
        "降级到本地模型同样要标注「模型生成」"
    );
}

#[test]
fn online_failure_without_any_other_tier_falls_back_to_template() {
    let fixture = Fixture::new();
    seed_notes(&fixture);
    fixture.configure_remote("http://localhost:9/v1", true, true, true);

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&session.id, false).expect("收集不应失败");
    assert_eq!(collected.decision_provider.as_deref(), Some("rule"));

    research.verify(&session.id).expect("验证");
    let composed = research.compose(&session.id).expect("汇总不应失败");
    let report = composed.report.as_ref().expect("报告");
    assert_eq!(report.composer, "template.draft");
    assert!(
        report.warnings.iter().any(|warning| warning.message.contains("退回")),
        "降级原因要在警告块里说明：{:?}",
        report
            .warnings
            .iter()
            .map(|warning| warning.message.clone())
            .collect::<Vec<_>>()
    );
}

/// 换了地址 = 换了收件人：授权立刻作废，之后不能再发（哪怕开关还开着、Key 还在）。
#[test]
fn changing_the_endpoint_revokes_the_authorization_and_stops_every_request() {
    let fixture = Fixture::new();
    seed_notes(&fixture);
    let stub = spawn_remote_server("（桩模型）第一段。");
    fixture.configure_remote(&stub.endpoint, true, true, true);

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度")).expect("立项");
    research.collect(&session.id, false).expect("收集");
    let before = stub.calls();
    assert!(before >= 1);

    // 换地址：授权作废
    fixture
        .policy()
        .update(UpdateAiPolicyInput {
            remote_model_base_url: Some("https://another.example.com/v1".to_string()),
            ..Default::default()
        })
        .expect("改地址");
    assert!(
        fixture.policy().policy().remote_model_authorized_at.is_none(),
        "换地址要重新确认"
    );

    // 再跑一轮：一个请求都不能发到旧地址
    let session2 = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&session2.id, false).expect("收集");
    assert_eq!(collected.decision_provider.as_deref(), Some("rule"));
    assert_eq!(stub.calls(), before, "换地址后不得再向旧地址发请求");
}

/// 纵深防御：**绕过写入路径**直接改设置库塞进一个裸 IP 地址，
/// 读取侧也必须拒绝——否则「内容发给谁」就取决于谁改了库。
#[test]
fn a_tampered_endpoint_in_the_database_is_never_used() {
    let fixture = Fixture::new();
    seed_notes(&fixture);
    let stub = spawn_remote_server("（不应该被调用）");
    fixture.configure_remote(&stub.endpoint, true, true, true);

    fixture
        .context()
        .update_settings(|settings| {
            settings.ai.remote_model_base_url = Some("https://10.0.0.5/v1".to_string());
        })
        .expect("直接写设置");

    let service = fixture.policy();
    assert!(service.remote_model().is_none(), "裸 IP 在读取侧就必须被拒绝");
    assert!(service.ensure_remote_model_allowed("research_score").is_err());

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&session.id, false).expect("收集");
    assert_eq!(collected.decision_provider.as_deref(), Some("rule"));
    assert_eq!(stub.calls(), 0, "改库也发不出去");
}

/// API Key 的边界：存在应用数据目录（不进工作区），响应里只有布尔回显。
#[test]
fn the_api_key_never_leaks_into_settings_or_responses() {
    let fixture = Fixture::new();
    let stub = spawn_remote_server("（不应该被调用）");
    fixture.configure_remote(&stub.endpoint, true, false, false);

    // 凭据文件在工作区之外
    assert!(
        fixture.data.path().join("ai-credentials.json").exists(),
        "Key 应写在应用数据目录"
    );
    assert!(
        !fixture.workspace.path().join("ai-credentials.json").exists(),
        "工作区里绝不能出现凭据文件（会被备份/导出带走）"
    );

    let settings_json = serde_json::to_string(&fixture.policy().policy()).expect("序列化策略");
    assert!(!settings_json.contains(API_KEY), "策略响应里不得包含 Key：{settings_json}");
    assert!(settings_json.contains("\"remoteModelKeySet\":true"), "只回显「已配置」");

    // 审计里也不得有 Key
    let audit_json =
        serde_json::to_string(&fixture.policy().audit_tail(20).expect("审计")).expect("序列化审计");
    assert!(!audit_json.contains(API_KEY), "审计里不得包含 Key");

    // 清除后回显恢复为 false
    credential_store::set_remote_model_key(fixture.data.path(), None).expect("清除");
    assert!(!fixture.policy().policy().remote_model_key_set);
}
