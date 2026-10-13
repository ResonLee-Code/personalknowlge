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

    // 冲突判定请求（Track C5）先认：桩模型一律判「存在冲突」。
    // 注意要放在打分分支之前——两种提示词都含「只输出 JSON」。
    let content = if body.contains("只有材料之间") {
        serde_json::json!({ "conflict": true, "reason": "桩模型：两条材料结论相反" }).to_string()
    } else if body.contains("只输出 JSON") {
        // 打分请求里带 `- id=<候选 id>`：把所有出现过的 id 回填（分数 1.0），不依赖固定 id
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

    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, false).expect("汇总");
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
    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, false).expect("汇总");
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

    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, false).expect("汇总不应失败");
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

    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, false).expect("汇总不应失败");
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

/// 开启「逐次外发确认」（Track C2）。
fn enable_per_send_confirmation(fixture: &Fixture) {
    fixture
        .policy()
        .update(UpdateAiPolicyInput {
            confirm_each_outbound: Some(true),
            ..Default::default()
        })
        .expect("开启逐次外发确认");
}

/// Track C2：开了「逐次确认」而本次没确认 —— **一个请求都不发**，自动降级并说明原因。
#[test]
fn per_send_confirmation_blocks_every_request_when_not_confirmed() {
    let fixture = Fixture::new();
    seed_notes(&fixture);
    let stub = spawn_remote_server("（桩模型）不该被调用。");
    fixture.configure_remote(&stub.endpoint, true, true, false);
    enable_per_send_confirmation(&fixture);

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度")).expect("立项");
    research.collect(&session.id, false).expect("收集");
    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, false).expect("汇总要降级而不是失败");

    assert_eq!(stub.calls(), 0, "未逐次确认时一个请求都不能发出去");
    let report = composed.report.as_ref().expect("报告");
    assert_eq!(report.composer, "template.draft", "应降级到模板实现");
    assert!(
        report.warnings.iter().any(|warning| warning.message.contains("逐次外发确认")),
        "报告要说清为什么没用在线模型：{:?}",
        report.warnings.iter().map(|warning| &warning.message).collect::<Vec<_>>()
    );
}

/// Track C2：本次显式确认后才真的外发。
#[test]
fn per_send_confirmation_allows_the_request_when_confirmed() {
    let fixture = Fixture::new();
    seed_notes(&fixture);
    let stub = spawn_remote_server("（桩模型）任务窃取是调度器的核心机制。");
    fixture.configure_remote(&stub.endpoint, true, true, false);
    enable_per_send_confirmation(&fixture);

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度")).expect("立项");
    research.collect(&session.id, false).expect("收集");
    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, true).expect("汇总");

    assert!(stub.calls() >= 1, "逐次确认后应当真的外发");
    let report = composed.report.as_ref().expect("报告");
    assert_eq!(report.composer, "remote.openai-compatible:stub-model");
}

/// Track C2：预览要**只读**地说清「发给谁、发什么」，且未授权时如实说不发。
#[test]
fn outbound_preview_lists_host_and_snippets_without_sending() {
    let fixture = Fixture::new();
    seed_notes(&fixture);
    let stub = spawn_remote_server("（桩模型）不会被调用。");
    fixture.configure_remote(&stub.endpoint, true, true, false);

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度")).expect("立项");
    research.collect(&session.id, false).expect("收集");
    research.verify(&session.id, false).expect("验证");

    let preview = research.outbound_preview(&session.id).expect("预览");
    assert!(preview.will_send, "已配置并授权 → 会外发：{preview:?}");
    assert_eq!(preview.host.as_deref(), Some("localhost"), "要写清发给谁");
    assert!(!preview.snippets.is_empty(), "必须把将要发送的片段列出来");
    assert_eq!(stub.calls(), 0, "预览不能发任何请求");

    // 未授权：预览如实说「不会外发」
    let other = Fixture::new();
    seed_notes(&other);
    let other_stub = spawn_remote_server("（桩模型）不会被调用。");
    other.configure_remote(&other_stub.endpoint, false, true, false);
    let other_research = ResearchService::new(other.context());
    let other_session = other_research.start(request("Rust 异步调度")).expect("立项");
    other_research.collect(&other_session.id, false).expect("收集");
    other_research.verify(&other_session.id, false).expect("验证");
    let other_preview = other_research.outbound_preview(&other_session.id).expect("预览");
    assert!(!other_preview.will_send, "未授权 → 不会外发");
    assert!(other_preview.reason.contains("授权"), "{}", other_preview.reason);
    assert_eq!(other_stub.calls(), 0);
}

/// 拿一个"没人听"的端口：绑定后立刻释放，用作"本地模型未部署"的地址。
fn dead_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口");
    let port = listener.local_addr().expect("端口").port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

/// 造一对互相矛盾的材料（同一子问题下：一条支持、一条警示）。
fn seed_opposing_notes(fixture: &Fixture) {
    fixture.note(
        "Rust 异步调度方案推荐实践",
        "推荐使用工作窃取队列，这种方案适用于高并发场景。",
    );
    fixture.note(
        "Rust 异步调度方案的风险",
        "这种方案存在风险：误用会拖垮运行时，不建议在生产环境直接照搬。",
    );
}

/// Track C5：冲突判定升级为语义档 —— 模型可用时由模型判 CONFLICT，报告注明档位。
#[test]
fn semantic_conflict_judgement_uses_the_model_when_available() {
    let fixture = Fixture::new();
    seed_opposing_notes(&fixture);
    let stub = spawn_remote_server("（桩模型）推测段。");
    // 打分侧开启 → 语义档可用（compose 侧没必要开，避免混淆生成来源）
    fixture.configure_remote(&stub.endpoint, true, false, true);

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度方案")).expect("立项");
    research.collect(&session.id, false).expect("收集");
    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, false).expect("汇总");

    let report = composed.report.as_ref().expect("报告");
    assert_eq!(
        report.conflict_provider.as_deref(),
        Some("remote.openai-compatible"),
        "冲突应当由模型判定"
    );
    assert!(!report.conflicts.is_empty(), "桩模型判冲突 → 第 3 段应有冲突");
}

/// Track C5：模型不可用时**回退规则档**，并在报告里注明是规则档判的。
#[test]
fn conflict_falls_back_to_the_rule_tier_and_says_so() {
    let fixture = Fixture::new();
    seed_opposing_notes(&fixture);
    // 本地模型开关打开，但地址没人听 → 语义档不可用 → 规则档
    fixture
        .policy()
        .update(UpdateAiPolicyInput { enabled: Some(true), ..Default::default() })
        .expect("开启 AI");
    fixture.configure_local(&dead_endpoint(), true, true);

    let research = ResearchService::new(fixture.context());
    let session = research.start(request("Rust 异步调度方案")).expect("立项");
    research.collect(&session.id, false).expect("收集");
    research.verify(&session.id, false).expect("验证");
    let composed = research.compose(&session.id, false).expect("汇总");

    let report = composed.report.as_ref().expect("报告");
    assert_eq!(
        report.conflict_provider.as_deref(),
        Some("rule"),
        "模型不可用要回退规则档，并如实注明"
    );
    assert!(!report.conflicts.is_empty(), "规则档仍要认出对立表述");
}

/// Track D2 / N6：打分统一走决策引擎——**每次都有可解释 Trace**，
/// 且**同输入第二次吃缓存**（不再打模型），报告里也能看到这条 Trace。
#[test]
fn scoring_records_a_trace_and_hits_the_cache_on_a_repeat_run() {
    let fixture = Fixture::new();
    seed_notes(&fixture);
    let stub = spawn_remote_server("（桩模型）推测段。");
    fixture.configure_remote(&stub.endpoint, true, false, true);

    let research = ResearchService::new(fixture.context());
    let first = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&first.id, false).expect("收集");

    let trace = collected.decision_trace.as_ref().expect("打分必须留下 Trace");
    assert_eq!(trace.kind, "RELEVANCE");
    assert_eq!(trace.cache, "miss", "第一次必然未命中");
    assert_eq!(trace.provider, "remote.openai-compatible");
    assert!(
        trace
            .attempts
            .iter()
            .any(|attempt| attempt.ok && attempt.provider == "remote.openai-compatible"),
        "Trace 要记下是谁判的：{trace:?}"
    );
    let calls_after_first = stub.calls();
    assert!(calls_after_first >= 1, "第一次应当真的调了在线模型");

    // 同一批材料再走一遍（内容相同 → 键相同）：应当直接吃缓存，不再打模型
    let second = research.start(request("Rust 异步调度")).expect("再次立项");
    let again = research.collect(&second.id, false).expect("再次收集");
    let trace_again = again.decision_trace.as_ref().expect("Trace");
    assert_eq!(trace_again.cache, "hit", "同输入第二次应当命中缓存：{trace_again:?}");
    assert_eq!(stub.calls(), calls_after_first, "命中缓存就不该再调模型");

    // 报告里也要能看到这条 Trace（"每次打分都有可解释 Trace"）
    research.verify(&second.id, false).expect("验证");
    let composed = research.compose(&second.id, false).expect("汇总");
    let report = composed.report.as_ref().expect("报告");
    let report_trace = report.decision_trace.as_ref().expect("报告要带上打分的 Trace");
    assert_eq!(report_trace.cache, "hit");
}
