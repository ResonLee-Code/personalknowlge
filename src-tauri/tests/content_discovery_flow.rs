use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::traits::{
    DecisionChoice, DecisionKind, DecisionModel, DecisionResult, DecisionState,
};
use personal_knowledge_os_lib::application::content_discovery::ContentDiscoveryService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::content_discovery::{
    ContentCaptureInput, ContentCaptureSaveInput, ContentCaptureStatus, ContentDiscoveryOptions,
    ContentDiscoveryRunInput,
};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
}

struct CandidateDecision;

impl DecisionModel for CandidateDecision {
    fn id(&self) -> &'static str {
        "test.content-candidates"
    }

    fn is_local(&self) -> bool {
        true
    }

    fn decide(
        &self,
        kind: DecisionKind,
        state: &DecisionState,
    ) -> personal_knowledge_os_lib::core::error::Result<Vec<DecisionResult>> {
        Ok(state
            .candidates
            .iter()
            .map(|candidate| {
                let low = candidate.note_id.contains("/noise");
                DecisionResult {
                    note_id: candidate.note_id.clone(),
                    choice: if low {
                        DecisionChoice::Drop
                    } else {
                        DecisionChoice::Use
                    },
                    score: if low { 0.05 } else { 0.9 },
                    confidence: 0.95,
                    reason: if low {
                        "明显无关"
                    } else {
                        "与当前页面高度相关"
                    }
                    .to_string(),
                    kind,
                }
            })
            .collect())
    }
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("内容发现测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        AiPolicyService::new(app.context().expect("工作区上下文"))
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                network: Some("allowNetwork".to_string()),
                confirm_network: Some(true),
                ..Default::default()
            })
            .expect("开启测试联网策略");
        Self {
            _data: data,
            _workspace: workspace,
            app,
        }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区上下文")
    }

    fn discovery(&self) -> ContentDiscoveryService {
        ContentDiscoveryService::with_loopback_for_tests(self.context())
    }
}

fn spawn_content_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定内容测试端口");
    let address = listener.local_addr().expect("读取端口");
    let changing_version = Arc::new(AtomicUsize::new(0));
    thread::spawn(move || {
        for stream in listener.incoming().take(128).flatten() {
            let _ = handle(stream, Arc::clone(&changing_version));
        }
    });
    format!("http://{address}")
}

fn handle(mut stream: TcpStream, changing_version: Arc<AtomicUsize>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" {
            break;
        }
    }
    let path = request_line.split_whitespace().nth(1).unwrap_or("/");
    if path == "/redirect-to-blocked" {
        let response =
            "HTTP/1.1 302 Found\r\nLocation: /blocked-by-robots\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        stream.write_all(response.as_bytes())?;
        stream.flush()?;
        return Ok(());
    }
    let (content_type, body) = match path {
        "/robots.txt" => (
            "text/plain; charset=utf-8",
            "User-agent: *\nAllow: /\nDisallow: /blocked-by-robots\n".to_string(),
        ),
        "/article" => (
            "text/html; charset=utf-8",
            r#"<!doctype html><html><head><title>内容发现测试文章</title></head><body>
              <nav>导航链接</nav>
              <article>
                <h1>内容发现测试文章</h1>
                <p>这是用于验证单页抓取与候选链接发现的正文。Rust 异步调度需要在本地知识中持续积累。</p>
                <a href="/guide">关联指南</a>
                <a href="/broken">损坏页面</a>
                <a href="/hidden" hidden>隐藏资料</a>
                <a href="/noise">无关购物</a>
                <a href="https://example.org/external">外部资料</a>
                <a href="mailto:test@example.com">邮件</a>
              </article>
            </body></html>"#
                .to_string(),
        ),
        "/guide" => (
            "text/html; charset=utf-8",
            r#"<!doctype html><html><head><title>关联指南</title></head><body>
              <article><h1>关联指南</h1><p>这一页补充了调度策略和适用场景。</p></article>
            </body></html>"#
                .to_string(),
        ),
        "/hidden" => (
            "text/html; charset=utf-8",
            r#"<!doctype html><html><head><title>隐藏资料</title></head><body>
              <article><h1>隐藏资料</h1><p>只有开启隐藏链接选项后才应抓取该页面。</p></article>
            </body></html>"#
                .to_string(),
        ),
        "/broken" => (
            "text/html; charset=utf-8",
            "<html><body><h1>服务暂时不可用</h1></body></html>".to_string(),
        ),
        "/changing" => {
            let version = if changing_version.fetch_add(1, Ordering::SeqCst) == 0 {
                1
            } else {
                2
            };
            (
                "text/html; charset=utf-8",
                format!(
                    "<html><head><title>持续更新文章</title></head><body><article><h1>持续更新文章</h1><p>这是第 {version} 版内容。来源每次检查都会返回不同的正文，用于验证更新预览。</p></article></body></html>"
                ),
            )
        }
        "/many" => {
            let links = (1..=30)
                .map(|index| format!(r#"<a href="/many/{index}">页面 {index}</a>"#))
                .collect::<String>();
            (
                "text/html; charset=utf-8",
                format!(
                    "<html><head><title>多页面文档</title></head><body><article><h1>多页面文档</h1>{links}</article></body></html>"
                ),
            )
        }
        path if path.starts_with("/many/") => (
            "text/html; charset=utf-8",
            format!(
                "<html><head><title>{path}</title></head><body><article><h1>{path}</h1><p>用于验证全局页面预算的独立正文。</p></article></body></html>"
            ),
        ),
        "/noise" => (
            "text/html; charset=utf-8",
            "<html><body><article><h1>购物清单</h1><p>完全无关内容。</p></article></body></html>"
                .to_string(),
        ),
        "/blocked-by-robots" => (
            "text/html; charset=utf-8",
            "<html><body><article><h1>不应抓取</h1><p>重定向目标被 robots.txt 禁止。</p></article></body></html>"
                .to_string(),
        ),
        _ => ("text/plain; charset=utf-8", "not found".to_string()),
    };
    let status = if path == "/broken" {
        "500 Internal Server Error"
    } else {
        "200 OK"
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

#[test]
fn discovers_selects_crawls_and_saves_public_content() {
    let fixture = Fixture::new();
    let base = spawn_content_server();
    let discovery = fixture.discovery();
    let created = discovery
        .create(ContentCaptureInput {
            url: format!("{base}/article"),
            shared_text: None,
            user_note: Some("用于调度方案整理".to_string()),
            confirmed: true,
        })
        .expect("创建内容发现记录");

    assert_eq!(created.status, ContentCaptureStatus::AwaitingSelection);
    assert_eq!(created.page_count, 1);
    assert!(created
        .summary
        .as_deref()
        .is_some_and(|value| !value.is_empty()));
    let guide = created
        .candidates
        .iter()
        .find(|candidate| candidate.url.ends_with("/guide"))
        .expect("关联指南候选");
    assert!(guide.eligible);
    let hidden = created
        .candidates
        .iter()
        .find(|candidate| candidate.url.ends_with("/hidden"))
        .expect("隐藏候选");
    assert!(hidden.hidden);
    assert!(!hidden.eligible);
    assert!(
        created
            .candidates
            .iter()
            .all(|candidate| !candidate.url.starts_with("mailto:")),
        "非 http 链接必须忽略"
    );

    let crawled = discovery
        .crawl_selected(ContentDiscoveryRunInput {
            capture_id: created.id.clone(),
            selected_urls: vec![guide.url.clone()],
            options: ContentDiscoveryOptions::default(),
            confirmed: true,
        })
        .expect("抓取选中链接");
    assert_eq!(crawled.status, ContentCaptureStatus::Ready);
    assert_eq!(crawled.page_count, 2);
    assert!(crawled
        .pages
        .iter()
        .any(|page| page.url.ends_with("/guide")));

    let saved = discovery
        .save(ContentCaptureSaveInput {
            capture_id: created.id.clone(),
            title: Some("调度资料发现".to_string()),
            category_id: None,
            tags: None,
            keep_full_text: Some(true),
        })
        .expect("写入普通笔记");
    let note = NoteService::new(fixture.context())
        .get(&saved.note_id)
        .expect("读取写入笔记");
    assert_eq!(note.note.title, "调度资料发现");
    assert!(note.content.contains("## 摘要"));
    assert!(note.content.contains("关联指南"));
    assert!(note.content.contains("## 原文"));
    assert_eq!(
        note.note.source_ref.as_deref(),
        Some(created.final_url.as_str())
    );
    assert!(
        note.tags.iter().any(|tag| tag.name == "网页发现"),
        "内容发现建议的来源标签必须随笔记一起入库"
    );
    assert!(
        discovery.ignore(&created.id).is_err(),
        "已入库内容不能被接口误标为忽略"
    );
}

#[test]
fn recrawling_can_include_hidden_static_links_without_duplicates() {
    let fixture = Fixture::new();
    let base = spawn_content_server();
    let discovery = fixture.discovery();
    let created = discovery
        .create(ContentCaptureInput {
            url: format!("{base}/article"),
            shared_text: None,
            user_note: None,
            confirmed: true,
        })
        .expect("创建内容");
    let guide = created
        .candidates
        .iter()
        .find(|candidate| candidate.url.ends_with("/guide"))
        .expect("关联指南");
    let hidden = created
        .candidates
        .iter()
        .find(|candidate| candidate.url.ends_with("/hidden"))
        .expect("隐藏页面");

    let first = discovery
        .crawl_selected(ContentDiscoveryRunInput {
            capture_id: created.id.clone(),
            selected_urls: vec![guide.url.clone()],
            options: ContentDiscoveryOptions::default(),
            confirmed: true,
        })
        .expect("首次抓取");
    assert_eq!(first.page_count, 2);

    let second = discovery
        .crawl_selected(ContentDiscoveryRunInput {
            capture_id: created.id,
            selected_urls: vec![guide.url.clone(), guide.url.clone(), hidden.url.clone()],
            options: ContentDiscoveryOptions {
                include_hidden: true,
                ..Default::default()
            },
            confirmed: true,
        })
        .expect("补充隐藏链接");
    assert_eq!(second.page_count, 3, "重复 URL 不应重复写入页面");
}

#[test]
fn decision_model_scores_candidates_before_selection() {
    let fixture = Fixture::new();
    let base = spawn_content_server();
    let discovery = ContentDiscoveryService::with_decision_model_for_tests(
        fixture.context(),
        Arc::new(CandidateDecision),
    );
    let created = discovery
        .create(ContentCaptureInput {
            url: format!("{base}/article"),
            shared_text: None,
            user_note: Some("只保留调度资料".to_string()),
            confirmed: true,
        })
        .expect("Decision 筛选候选");

    assert_eq!(
        created.decision_provider.as_deref(),
        Some("test.content-candidates")
    );
    let guide = created
        .candidates
        .iter()
        .find(|candidate| candidate.url.ends_with("/guide"))
        .expect("关联指南");
    assert!(guide.selected);
    assert_eq!(guide.relevance_score, Some(0.9));
    let noise = created
        .candidates
        .iter()
        .find(|candidate| candidate.url.ends_with("/noise"))
        .expect("无关候选");
    assert!(!noise.eligible);
    assert_eq!(
        noise.blocked_reason.as_deref(),
        Some("decisionLowRelevance")
    );
}

#[test]
fn failed_child_page_is_persisted_without_aborting_the_capture() {
    let fixture = Fixture::new();
    let base = spawn_content_server();
    let discovery = fixture.discovery();
    let created = discovery
        .create(ContentCaptureInput {
            url: format!("{base}/article"),
            shared_text: None,
            user_note: None,
            confirmed: true,
        })
        .expect("创建内容");
    let broken = created
        .candidates
        .iter()
        .find(|candidate| candidate.url.ends_with("/broken"))
        .expect("损坏页面候选");

    let crawled = discovery
        .crawl_selected(ContentDiscoveryRunInput {
            capture_id: created.id.clone(),
            selected_urls: vec![broken.url.clone()],
            options: ContentDiscoveryOptions::default(),
            confirmed: true,
        })
        .expect("单页失败不应中止整个任务");

    assert_eq!(crawled.status, ContentCaptureStatus::Ready);
    assert_eq!(crawled.page_count, 1);
    let persisted = discovery.get(&created.id).expect("重新读取内容");
    let failed = persisted
        .pages
        .iter()
        .find(|page| page.canonical_url == broken.canonical_url)
        .expect("失败页面必须落库，刷新后仍可解释");
    assert_eq!(failed.status, "failed");
    assert!(failed
        .error
        .as_deref()
        .is_some_and(|value| value.contains("HTTP 500")));
}

#[test]
fn refresh_reports_source_changes_without_overwriting_notes() {
    let fixture = Fixture::new();
    let base = spawn_content_server();
    let discovery = fixture.discovery();
    let created = discovery
        .create(ContentCaptureInput {
            url: format!("{base}/changing"),
            shared_text: None,
            user_note: None,
            confirmed: true,
        })
        .expect("创建内容");
    assert!(!created.source_changed);

    let refreshed = discovery.refresh(&created.id, true).expect("检查来源更新");
    assert!(refreshed.source_changed);
    assert!(refreshed.last_refreshed_at.is_some());
    assert!(
        refreshed
            .summary
            .as_deref()
            .is_some_and(|value| value.contains("第 2 版")),
        "刷新后的摘要应反映新正文：{:?}",
        refreshed.summary
    );

    let unchanged = discovery.refresh(&created.id, true).expect("再次检查来源");
    assert!(!unchanged.source_changed);
    assert!(unchanged.note_id.is_none(), "刷新预览不能创建正式笔记");
}

#[test]
fn ignored_capture_can_be_collected_again() {
    let fixture = Fixture::new();
    let base = spawn_content_server();
    let discovery = fixture.discovery();
    let input = || ContentCaptureInput {
        url: format!("{base}/article"),
        shared_text: None,
        user_note: None,
        confirmed: true,
    };
    let first = discovery.create(input()).expect("首次收集内容");
    discovery.ignore(&first.id).expect("忽略内容");

    let second = discovery.create(input()).expect("忽略后重新收集");
    assert_ne!(first.id, second.id, "忽略的记录不应阻断再次收集同一 URL");
}

#[test]
fn redirected_target_must_still_pass_robots() {
    let fixture = Fixture::new();
    let base = spawn_content_server();
    let discovery = fixture.discovery();

    let error = discovery
        .create(ContentCaptureInput {
            url: format!("{base}/redirect-to-blocked"),
            shared_text: None,
            user_note: None,
            confirmed: true,
        })
        .expect_err("重定向目标被 robots 禁止时必须拒绝抓取");

    assert!(
        error.message.contains("robots.txt"),
        "错误应明确指出 robots 限制：{}",
        error.message
    );
}

#[test]
fn repeated_selection_never_exceeds_the_page_budget() {
    let fixture = Fixture::new();
    let base = spawn_content_server();
    let discovery = fixture.discovery();
    let created = discovery
        .create(ContentCaptureInput {
            url: format!("{base}/many"),
            shared_text: None,
            user_note: None,
            confirmed: true,
        })
        .expect("创建多页面内容");
    let selected = created
        .candidates
        .iter()
        .map(|candidate| candidate.url.clone())
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 30);

    let first = discovery
        .crawl_selected(ContentDiscoveryRunInput {
            capture_id: created.id.clone(),
            selected_urls: selected.clone(),
            options: ContentDiscoveryOptions::default(),
            confirmed: true,
        })
        .expect("首轮抓取");
    assert_eq!(first.page_count, 20, "包含根页面在内的全局上限是 20");

    let second = discovery
        .crawl_selected(ContentDiscoveryRunInput {
            capture_id: created.id,
            selected_urls: selected.into_iter().skip(19).collect(),
            options: ContentDiscoveryOptions::default(),
            confirmed: true,
        })
        .expect("再次勾选抓取");
    assert_eq!(second.page_count, 20, "多次抓取不能突破同一个页面预算");
}
