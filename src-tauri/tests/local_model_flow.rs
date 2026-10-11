//! 本地模型接入（V1.2.1 阶段 D5 / `local_model.rs`，Ollama HTTP）集成测试。
//!
//! 用本机桩服务假装 Ollama（不需要真的装模型）：
//! 1. 开启「用本地模型打分」后，来源可信度由模型分数决定，报告写明 provider；
//! 2. 开启「用本地模型写报告」后，推测段来自模型输出，并给出「模型生成」警告；
//! 3. **模型不可用时自动降级**：打分退回规则档、报告退回模板实现，流程不中断；
//! 4. 默认（不配置本地模型）时一个请求都不发。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::research::ResearchService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::domain::research::{ResearchRequest, WarningKind};
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
            .create("本地模型测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn policy(&self) -> AiPolicyService {
        AiPolicyService::new(self.context())
    }

    fn research(&self) -> ResearchService {
        ResearchService::new(self.context())
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

    /// 打开 AI、配置本地模型；`endpoint` 为桩服务地址。
    fn configure_local_model(&self, endpoint: &str, compose: bool, score: bool) {
        self.policy()
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                local_model_endpoint: Some(endpoint.to_string()),
                local_model_name: Some("stub:latest".to_string()),
                local_model_compose: Some(compose),
                local_model_score: Some(score),
                ..Default::default()
            })
            .expect("配置本地模型");
    }
}

/// 一个「假装 Ollama」的本机服务：读请求体，按提示词类型返回打分 JSON 或普通文本。
///
/// 打分请求里会带 `- id=<候选 id> ...`，桩服务把出现过的 id 全部回填（分数 1.0），
/// 这样测试既能验证真实链路，又不依赖固定 id。
fn spawn_model_server(text_reply: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口");
    let port = listener.local_addr().expect("端口").port();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = handle(&mut stream, text_reply);
            let _ = counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    (format!("http://127.0.0.1:{port}"), calls)
}

fn handle(stream: &mut TcpStream, text_reply: &str) -> std::io::Result<()> {
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
    let body = String::from_utf8_lossy(&body).to_string();

    let model_reply = if body.contains("只输出 JSON") {
        // 打分请求：把所有候选 id 回填，分数 1.0
        // 请求体是一行 JSON（提示词里的换行是转义字符），因此直接在字符串里扫 `- id=`
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
    let payload = serde_json::json!({ "model": "stub:latest", "response": model_reply }).to_string();
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

#[test]
fn local_model_scores_sources_and_writes_the_report_when_enabled() {
    let fixture = Fixture::new();
    fixture.note("Rust 异步调度方案对比", "主流方案是对比 tokio 的调度实现。");
    fixture.note("Rust 异步调度实现要点", "实现要点在于任务唤醒与批量窃取。");
    let (endpoint, calls) = spawn_model_server("（桩模型）任务窃取是调度器的核心机制。");
    fixture.configure_local_model(&endpoint, true, true);

    let research = fixture.research();
    let session = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&session.id, false).expect("收集");

    // 打分由本地模型给出：分数 1.0 → 可信度 = 0.5×1 + 0.5×(级别折算)
    let audit = fixture.policy().audit_tail(20).expect("审计");
    assert_eq!(
        collected.decision_provider.as_deref(),
        Some("local.ollama"),
        "provider 标识应指向本地模型；审计：{audit:?}"
    );
    assert!(calls.load(Ordering::SeqCst) >= 1, "打分必须真的调用本地模型");
    for source in &collected.sources {
        let base = source.reliability as f32 / 5.0;
        let expected = 0.5 * 1.0 + 0.5 * base;
        assert!(
            (source.credibility - expected).abs() < 1e-5,
            "可信度应等于模型分与级别折算的混合：{} vs {}",
            source.credibility,
            expected
        );
    }

    let verified = research.verify(&session.id).expect("验证");
    let composed = research.compose(&session.id).expect("汇总");
    let report = composed.report.as_ref().expect("报告");
    assert_eq!(report.decision_provider, "local.ollama", "报告要写明打分来源");
    assert_eq!(report.composer, "local.ollama:stub:latest", "报告要写明生成来源");
    assert_eq!(
        report.speculation.first().map(String::as_str),
        Some("（桩模型）任务窃取是调度器的核心机制。"),
        "推测段应来自模型输出"
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.kind == WarningKind::ModelGenerated),
        "必须给出「模型生成、未经核对」警告"
    );
    assert!(verified.report.is_some());
}

#[test]
fn unreachable_local_model_falls_back_without_breaking_the_flow() {
    let fixture = Fixture::new();
    fixture.note("Rust 异步调度方案对比", "主流方案是对比 tokio 的调度实现。");
    fixture.note("Rust 异步调度实现要点", "实现要点在于任务唤醒与批量窃取。");
    // 指向一个必然连不上的端口
    fixture.configure_local_model("http://127.0.0.1:9", true, true);

    let research = fixture.research();
    let session = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&session.id, false).expect("收集不应失败");
    assert_eq!(
        collected.decision_provider.as_deref(),
        Some("rule"),
        "本地模型失败必须降级到规则档"
    );
    assert!(collected
        .sources
        .iter()
        .all(|source| source.credibility > 0.0));

    research.verify(&session.id).expect("验证");
    let composed = research.compose(&session.id).expect("汇总不应失败");
    let report = composed.report.as_ref().expect("报告");
    assert_eq!(report.composer, "template.draft", "生成失败要退回模板实现");
    assert!(
        report.warnings.iter().any(|warning| warning.message.contains("退回模板")),
        "降级原因要在警告块里说明：{:?}",
        report
            .warnings
            .iter()
            .map(|warning| warning.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn local_model_is_never_called_when_not_configured() {
    let fixture = Fixture::new();
    fixture.note("Rust 异步调度方案对比", "主流方案是对比 tokio 的调度实现。");
    let (endpoint, calls) = spawn_model_server("（不应该被调用）");
    // 只填地址与模型名，但两个开关都关着 → 不应调用
    fixture.configure_local_model(&endpoint, false, false);

    let research = fixture.research();
    let session = research.start(request("Rust 异步调度")).expect("立项");
    let collected = research.collect(&session.id, false).expect("收集");
    assert_eq!(collected.decision_provider.as_deref(), Some("rule"));
    research.verify(&session.id).expect("验证");
    research.compose(&session.id).expect("汇总");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "开关关闭时一个请求都不能发");

    // 非本机地址会被规范化掉（本地模型只允许本机）
    let policy = fixture
        .policy()
        .update(UpdateAiPolicyInput {
            local_model_endpoint: Some("http://example.com:11434".to_string()),
            ..Default::default()
        })
        .expect("更新策略");
    assert!(
        policy.local_model_endpoint.is_none(),
        "远程地址不得被当成本地模型（需要单独授权，本版不做）"
    );
}

/// 纵深防御：**库里的地址没经过写入路径的规范化**时（例如有人直接改了设置），
/// 读取侧必须再校验一次「只连本机」，绝不能让笔记内容发到远程去。
#[test]
fn a_stored_remote_endpoint_is_never_used_even_if_it_bypassed_normalization() {
    let fixture = Fixture::new();
    fixture.note("Rust 异步调度方案对比", "主流方案是对比 tokio 的调度实现。");
    let (endpoint, calls) = spawn_model_server("（不应该被调用）");
    fixture.configure_local_model(&endpoint, true, true);

    // 绕过写入路径：直接往设置里塞一个远程地址（模拟「库被改过」）
    fixture
        .context()
        .update_settings(|settings| {
            settings.ai.local_model_endpoint = Some("http://evil.example.com:11434".to_string());
        })
        .expect("直接写设置");

    let service = fixture.policy();
    assert!(
        service.local_model().is_none(),
        "远程地址在读取侧就必须被拒绝，而不是照发"
    );

    // 走完整流程：报告仍由模板实现（有模板警告），且一个请求都没发到桩服务
    let research = fixture.research();
    let session = research.start(request("Rust 异步调度")).expect("立项");
    research.collect(&session.id, false).expect("收集");
    research.verify(&session.id).expect("验证");
    let composed = research.compose(&session.id).expect("汇总");
    let report = composed.report.as_ref().expect("报告");
    assert_eq!(report.composer, "template.draft", "远程地址不得被用来生成报告");
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.kind == WarningKind::TemplateGenerated),
        "要如实标注报告来自模板：{:?}",
        report.warnings
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0, "一个请求都不能发出去");
}
