//! 模型自检（Track C4）集成测试：用本机桩服务假装 Ollama 的 `GET /api/tags`。
//!
//! 覆盖"为什么是这一档"里两种**真实链路**（不依赖真的装 Ollama）：
//! 1. **桩服务可用** → 自检判本地档，provider 与报告 composer 同源；
//! 2. **未部署**（端口没人听） → 自检退回模板，并说清"连不上"。
//!
//! 「未授权」那条在 `application/ai/diagnose.rs` 的单测里覆盖（不需要起服务）。

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use personal_knowledge_os_lib::application::ai::diagnose::{
    evaluate, DiagnoseInputs, HttpLocalModelProbe,
};
use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
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
            .create("自检测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn service(&self) -> AiPolicyService {
        AiPolicyService::new(self.app.context().expect("工作区已打开"))
    }

    /// 打开 AI、把本地模型指到 `endpoint`，并开启「用本地模型写报告」。
    fn configure_local_model(&self, endpoint: &str, model: &str) {
        self.service()
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                local_model_endpoint: Some(endpoint.to_string()),
                local_model_name: Some(model.to_string()),
                local_model_compose: Some(true),
                ..Default::default()
            })
            .expect("配置本地模型");
    }

    /// 按命令层的口径组装输入并自检（真机探测器）。
    fn diagnose(&self) -> personal_knowledge_os_lib::application::ai::diagnose::AiDiagnosis {
        let service = self.service();
        let policy = service.policy();
        let (remote_base_name, remote_key_present) = service.remote_diagnosis();
        evaluate(
            DiagnoseInputs {
                policy: &policy,
                local: service.local_model(),
                remote_base_name,
                remote_key_present,
            },
            &HttpLocalModelProbe,
        )
    }
}

/// 一个"假装 Ollama"的本机服务：对任何请求都回一份 `/api/tags` 形状的模型列表。
fn spawn_tags_server(model_name: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口");
    let port = listener.local_addr().expect("端口").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = respond_tags(&mut stream, model_name);
        }
    });
    format!("http://127.0.0.1:{port}")
}

fn respond_tags(stream: &mut TcpStream, model_name: &str) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" {
            break;
        }
    }
    let body = format!(r#"{{"models":[{{"name":"{model_name}"}}]}}"#);
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())
}

/// 拿一个"没人听"的端口：绑定后立刻释放，用作"未部署"的地址。
fn dead_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口");
    let port = listener.local_addr().expect("端口").port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

#[test]
fn stub_ollama_makes_the_diagnosis_report_the_local_tier() {
    let fixture = Fixture::new();
    let endpoint = spawn_tags_server("stub:latest");
    fixture.configure_local_model(&endpoint, "stub:latest");

    let diagnosis = fixture.diagnose();

    assert_eq!(diagnosis.tier, "local", "有桩服务时应当判本地档：{diagnosis:?}");
    assert_eq!(
        diagnosis.provider_id, "local.ollama:stub:latest",
        "provider 标识要与报告 composer 同源"
    );
    let local = diagnosis.checks.iter().find(|check| check.tier == "local").expect("本地档核对");
    assert!(local.available, "本地档应可用：{}", local.detail);
    assert!(local.detail.contains("就绪"), "要说清为什么可用：{}", local.detail);
}

#[test]
fn missing_daemon_falls_back_to_template_and_says_why() {
    let fixture = Fixture::new();
    fixture.configure_local_model(&dead_endpoint(), "stub:latest");

    let diagnosis = fixture.diagnose();

    assert_eq!(diagnosis.tier, "template", "连不上时应退回模板：{diagnosis:?}");
    assert_eq!(diagnosis.provider_id, "template.draft");
    let local = diagnosis.checks.iter().find(|check| check.tier == "local").expect("本地档核对");
    assert!(!local.available);
    assert!(local.detail.contains("连不上"), "要说清是连不上：{}", local.detail);
    // 模板档永远可用，且要说明它不外发
    let template = diagnosis.checks.iter().find(|check| check.tier == "template").expect("模板核对");
    assert!(template.available);
}

#[test]
fn unconfigured_ai_reports_template_without_touching_anything() {
    let fixture = Fixture::new();

    let diagnosis = fixture.diagnose();

    assert!(!diagnosis.enabled);
    assert_eq!(diagnosis.tier, "template");
    assert!(
        diagnosis.checks.iter().all(|check| check.tier == "template" || !check.available),
        "没配任何模型时，在线/本地都不该被说成可用"
    );
}
