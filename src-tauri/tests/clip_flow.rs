//! 网页剪藏集成测试（V1.2.1 阶段 C1）。
//!
//! 用**本地 HTTP 服务**覆盖真实抓取路径（不去访问外网，测试可离线稳定运行）：
//! 302 跳转 → 最终地址、HTML 解析成 Markdown、非 HTML 与 404 的错误提示。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use personal_knowledge_os_lib::application::clip_service::clip;

/// 一条预设响应。
struct Route {
    status: &'static str,
    content_type: &'static str,
    location: Option<&'static str>,
    body: String,
}

impl Route {
    fn html(body: &str) -> Self {
        Self {
            status: "200 OK",
            content_type: "text/html; charset=utf-8",
            location: None,
            body: body.to_string(),
        }
    }

    fn redirect(to: &'static str) -> Self {
        Self { status: "302 Found", content_type: "text/html", location: Some(to), body: String::new() }
    }

    fn raw(status: &'static str, content_type: &'static str, body: &str) -> Self {
        Self { status, content_type, location: None, body: body.to_string() }
    }
}

/// 极简 HTTP 服务：按路径返回预设响应，服务若干次请求后退出。
fn spawn_server(routes: Vec<(&'static str, Route)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定本地端口");
    let address = listener.local_addr().expect("读取端口");
    thread::spawn(move || {
        // 最多服务 2× 路由数（302 会多一次请求），避免测试永久等待
        for _ in 0..(routes.len() * 2 + 1) {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let _ = handle(stream, &routes);
        }
    });
    format!("http://{address}")
}

fn handle(mut stream: TcpStream, routes: &[(&'static str, Route)]) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    // 吃掉请求头
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" {
            break;
        }
    }
    let path = request_line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let route = routes
        .iter()
        .find(|(candidate, _)| *candidate == path)
        .map(|(_, route)| route);
    let (status, content_type, location, body) = match route {
        Some(route) => (
            route.status,
            route.content_type,
            route.location,
            route.body.clone(),
        ),
        None => ("404 Not Found", "text/plain", None, "not found".to_string()),
    };

    let mut response = format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n");
    if let Some(location) = location {
        response.push_str(&format!("Location: {location}\r\n"));
    }
    response.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ));
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

const PAGE: &str = r#"<!doctype html>
<html><head><title>测试页</title><meta property="og:site_name" content="本地测试站" /></head>
<body>
  <nav>导航不该出现</nav>
  <article>
    <h1>剪藏测试标题</h1>
    <p>这是<strong>正文</strong>，包含<a href="/link">相对链接</a>。</p>
    <ul><li>第一项</li></ul>
  </article>
</body></html>"#;

#[test]
fn clips_a_local_page_through_a_redirect() {
    let base = spawn_server(vec![
        ("/", Route::redirect("/page")),
        ("/page", Route::html(PAGE)),
    ]);

    let page = clip(&format!("{base}/")).expect("抓取应成功（跟随 302）");
    // 标题优先级：og:title → h1 → <title>；本页有 h1，所以取 h1
    assert_eq!(page.title, "剪藏测试标题");
    assert_eq!(page.site.as_deref(), Some("本地测试站"));
    assert!(page.final_url.ends_with("/page"), "最终地址应跟随跳转：{}", page.final_url);
    assert!(page.markdown.contains("# 剪藏测试标题"), "{}", page.markdown);
    assert!(page.markdown.contains("**正文**"), "{}", page.markdown);
    assert!(page.markdown.contains("- 第一项"), "{}", page.markdown);
    assert!(!page.markdown.contains("导航不该出现"), "{}", page.markdown);
    assert!(page.word_count > 0);
    assert!(!page.fetched_at.is_empty());
    assert!(page.bytes > 0);
}

#[test]
fn reports_clear_errors_for_non_html_and_missing_pages() {
    let base = spawn_server(vec![
        ("/file.pdf", Route::raw("200 OK", "application/pdf", "%PDF-1.7")),
        ("/missing", Route::raw("404 Not Found", "text/plain", "nope")),
    ]);

    let binary = clip(&format!("{base}/file.pdf")).expect_err("非 HTML 应报错");
    assert!(
        binary.message.contains("不是网页") || binary.message.contains("Content-Type"),
        "错误信息应说明只支持 HTML：{}",
        binary.message
    );

    let missing = clip(&format!("{base}/missing")).expect_err("404 应报错");
    assert!(!missing.message.is_empty());

    // 地址校验发生在发请求之前：非法地址不应触网
    assert!(clip("file:///etc/passwd").is_err());
    assert!(clip("不是地址").is_err());
}
