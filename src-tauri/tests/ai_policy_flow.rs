//! AI 策略闸门与审计的集成测试（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §6、§9.7、§9.9）。
//!
//! 关键验收：**默认口径下不发起任何网络请求**——把地址指向一个「必然连不上」的端口，
//! 如果闸门漏了，错误会是连接失败而不是 `NETWORK_DISABLED` / `AI_DISABLED`。

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::tools::AiTools;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::{ResearchBudget, UpdateAiPolicyInput};
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
            .create("AI 闸门测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn policy(&self) -> AiPolicyService {
        AiPolicyService::new(self.context())
    }

    fn tools(&self) -> AiTools {
        AiTools::new(self.context())
    }
}

/// 起一个只服务一次的本机 HTTP 服务，返回 (地址, 命中标记)。
fn spawn_once_server(body: &'static str) -> (String, Arc<std::sync::atomic::AtomicBool>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口");
    let address = listener.local_addr().expect("端口");
    let touched = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = Arc::clone(&touched);
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
            let _ = respond(&mut stream, body);
        }
    });
    (format!("http://{address}"), touched)
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
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

#[test]
fn network_tools_are_blocked_by_default_and_never_touch_the_network() {
    let fixture = Fixture::new();
    let tools = fixture.tools();
    let policy = fixture.policy();

    // 默认策略：总开关关闭、联网仅本地、抓取正文关闭
    let defaults = policy.policy();
    assert!(!defaults.enabled);
    assert!(!defaults.allow_fetch_body);
    assert_eq!(defaults.budget, ResearchBudget::default());

    // 指向必然连不上的地址：如果闸门漏了，错误会是连接失败
    let unreachable = "http://127.0.0.1:1/secret";
    let denied = tools.web_fetch(unreachable, true).expect_err("总开关关闭应拒绝");
    assert_eq!(denied.code.as_str(), "AI_DISABLED", "{}", denied.message);

    // 打开总开关，但联网档位仍是「仅本地」
    policy
        .update(UpdateAiPolicyInput { enabled: Some(true), ..Default::default() })
        .expect("开启总开关");
    let denied = tools.web_fetch(unreachable, true).expect_err("仅本地应拒绝");
    assert_eq!(denied.code.as_str(), "NETWORK_DISABLED");

    // 切到允许联网但未确认
    policy
        .update(UpdateAiPolicyInput { network: Some("allowNetwork".into()), ..Default::default() })
        .expect("切换档位");
    let unconfirmed = tools.web_fetch(unreachable, false).expect_err("未确认应拒绝");
    assert_eq!(unconfirmed.code.as_str(), "NETWORK_DISABLED");
    let not_confirmed_yet = tools.web_fetch(unreachable, true).expect_err("档位未确认应拒绝");
    assert_eq!(not_confirmed_yet.code.as_str(), "NETWORK_DISABLED");

    // 确认档位，但「抓取正文」仍关闭
    policy
        .update(UpdateAiPolicyInput { confirm_network: Some(true), ..Default::default() })
        .expect("确认联网");
    let no_body = tools.web_fetch(unreachable, true).expect_err("未开启抓取正文应拒绝");
    assert_eq!(no_body.code.as_str(), "NETWORK_DISABLED");

    // 开启抓取正文，但把该域名加入黑名单
    policy
        .update(UpdateAiPolicyInput {
            allow_fetch_body: Some(true),
            domain_denylist: Some(vec!["127.0.0.1".into()]),
            ..Default::default()
        })
        .expect("开启抓取正文 + 黑名单");
    let blocked = tools.web_fetch(unreachable, true).expect_err("黑名单应拒绝");
    assert!(blocked.message.contains("黑名单"), "{}", blocked.message);

    // 换成非空白名单（不含该域名）→ 仍然拒绝
    policy
        .update(UpdateAiPolicyInput {
            domain_denylist: Some(vec![]),
            domain_allowlist: Some(vec!["docs.example.com".into()]),
            ..Default::default()
        })
        .expect("设置白名单");
    let not_allowed = tools.web_fetch(unreachable, true).expect_err("白名单外应拒绝");
    assert!(not_allowed.message.contains("白名单"), "{}", not_allowed.message);

    // 所有拒绝路径都不得发起请求（审计里 outbound 全为 false）
    let tail = policy.audit_tail(20).expect("审计");
    assert!(tail.len() >= 6, "应记录每一次拒绝：{}", tail.len());
    assert!(
        tail.iter().all(|entry| !entry.outbound),
        "被拒绝的调用不应标记为已外发：{:?}",
        tail
    );
}

#[test]
fn gate_opens_for_allowlisted_host_and_records_audit() {
    let fixture = Fixture::new();
    let policy = fixture.policy();
    let (url, touched) = spawn_once_server("<html><body><article><h1>允许抓取</h1><p>正文一段，足够长以便抽取。</p></article></body></html>");

    policy
        .update(UpdateAiPolicyInput {
            enabled: Some(true),
            network: Some("allowNetwork".into()),
            confirm_network: Some(true),
            allow_fetch_body: Some(true),
            domain_allowlist: Some(vec!["127.0.0.1".into()]),
            budget: Some(ResearchBudget { max_subquestions: 3, max_pages: 4, max_duration_secs: 60 }),
            ..Default::default()
        })
        .expect("配置策略");

    let page = fixture
        .tools()
        .web_fetch(&url, true)
        .expect("白名单内 + 已确认应允许抓取");
    assert!(page.markdown.contains("允许抓取"), "{}", page.markdown);
    assert!(
        touched.load(std::sync::atomic::Ordering::SeqCst),
        "放行后应真的发起了请求"
    );

    let policy_now = policy.policy();
    assert_eq!(policy_now.budget.max_pages, 4, "预算按设置生效");
    assert!(policy_now.network_confirmed_at.is_some());

    let tail = policy.audit_tail(5).expect("审计");
    let latest = tail.first().expect("至少一条");
    assert_eq!(latest.capability, "web_fetch");
    assert!(latest.outbound, "成功的抓取应标记为已外发");
    assert_eq!(latest.outcome, "ok");

    // 审计文件是可读的 JSONL（不含正文）
    let audit = fixture
        .context()
        .paths
        .log_dir()
        .join("ai-audit.jsonl");
    let raw = std::fs::read_to_string(&audit).expect("审计文件存在");
    assert!(raw.contains("web_fetch"));
    assert!(!raw.contains("允许抓取"), "审计不得写入正文");
}
