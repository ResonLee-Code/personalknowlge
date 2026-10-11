//! 网页剪藏集成测试（V1.2.1 阶段 C1）。
//!
//! 用**本地 HTTP 服务**覆盖真实抓取路径（不去访问外网，测试可离线稳定运行）：
//! 302 跳转 → 最终地址、HTML 解析成 Markdown、非 HTML 与 404 的错误提示。

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

use personal_knowledge_os_lib::application::clip_service::clip;

/// 一条预设响应。
struct Route {
    status: &'static str,
    content_type: &'static str,
    location: Option<&'static str>,
    body: Vec<u8>,
}

impl Route {
    fn html(body: &str) -> Self {
        Self {
            status: "200 OK",
            content_type: "text/html; charset=utf-8",
            location: None,
            body: body.as_bytes().to_vec(),
        }
    }

    /// 原始字节响应：用来构造非 UTF-8 的页面（例如 GBK 编码的站点）。
    fn bytes(content_type: &'static str, body: Vec<u8>) -> Self {
        Self { status: "200 OK", content_type, location: None, body }
    }

    fn redirect(to: &'static str) -> Self {
        Self { status: "302 Found", content_type: "text/html", location: Some(to), body: Vec::new() }
    }

    fn raw(status: &'static str, content_type: &'static str, body: &str) -> Self {
        Self { status, content_type, location: None, body: body.as_bytes().to_vec() }
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
        None => ("404 Not Found", "text/plain", None, b"not found".to_vec()),
    };

    let mut headers = format!("HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n");
    if let Some(location) = location {
        headers.push_str(&format!("Location: {location}\r\n"));
    }
    headers.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    stream.write_all(headers.as_bytes())?;
    stream.write_all(&body)?;
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

/// GBK 页面（Content-Type 声明 charset=gbk）必须被正确解码：
/// 这是 CHANGELOG 里「ureq + charset 以便正确解码 GBK 页面」那条承诺的证据。
///
/// 页面里「中文内容」四个字用 **GBK 字节**（D6 D0 CE C4 A4…）拼出来，
/// 若按 UTF-8 硬解会变成乱码。
#[test]
fn decodes_gbk_page_declared_in_content_type() {
    // "标题：" -> B1 EA CC E2 A3 BA ; "中文内容" -> D6 D0 CE C4 C4 DA C8 DD
    let mut body: Vec<u8> = b"<html><head><title>\xB1\xEA\xCC\xE2\xA3\xBA\xD6\xD0\xCE\xC4</title></head><body><h1>\xB1\xEA\xCC\xE2\xA3\xBA\xD6\xD0\xCE\xC4</h1><p>\xD6\xD0\xCE\xC4\xC4\xDA\xC8\xDD</p></body></html>".to_vec();
    let base = spawn_server(vec![(
        "/gbk",
        Route::bytes("text/html; charset=gbk", std::mem::take(&mut body)),
    )]);

    let page = clip(&format!("{base}/gbk")).expect("GBK 页面也应抓得到");
    assert!(
        page.markdown.contains("中文内容"),
        "GBK 页面必须正确解码（否则是乱码）：\n{}",
        page.markdown
    );
    assert!(
        page.title.contains("标题") || page.markdown.contains("标题"),
        "标题也要解码正确：{}",
        page.title
    );
    assert!(
        page.fetched_at.len() >= 10,
        "抓取时间要记下来（时效口径）"
    );
}

/// 只在 `<meta charset="gbk">` 里声明编码、Content-Type 不带 charset 的页面
/// 同样必须正确解码：这种写法在中文站点里很常见。
///
/// 回归背景：此前整页直接按 UTF-8 读，遇到这类页面会**整页失败**，
/// 而且错误信息把它误报成「超过 3 MB 的页面请改用其它方式保存」，误导用户。
#[test]
fn decodes_gbk_page_declared_only_in_meta() {
    let body: Vec<u8> = b"<html><head><meta charset=\"gbk\"><title>\xB1\xEA\xCC\xE2</title></head><body><h1>\xB1\xEA\xCC\xE2</h1><p>\xD6\xD0\xCE\xC4\xC4\xDA\xC8\xDD</p></body></html>".to_vec();
    let base = spawn_server(vec![("/meta-gbk", Route::bytes("text/html", body))]);

    let page = clip(&format!("{base}/meta-gbk")).expect("声明了 meta charset 的页面必须能抓下来");
    assert!(
        page.markdown.contains("中文内容"),
        "meta charset=gbk 的页面要正确解码：\n{}",
        page.markdown
    );
    assert!(page.title.contains("标题"), "标题也要解码正确：{}", page.title);
}

/// 既没有 Content-Type charset、也没有 meta 声明：先试严格 UTF-8，
/// 失败再按 GBK 兜底（中文网页最常见的非 UTF-8 编码），不能整页失败。
#[test]
fn falls_back_to_gbk_when_no_charset_is_declared_at_all() {
    let body: Vec<u8> =
        b"<html><head><title>\xB1\xEA\xCC\xE2</title></head><body><p>\xD6\xD0\xCE\xC4\xC4\xDA\xC8\xDD</p></body></html>"
            .to_vec();
    let base = spawn_server(vec![("/bare", Route::bytes("text/html", body))]);

    let page = clip(&format!("{base}/bare")).expect("没有任何编码声明时也要尽力解出来");
    assert!(
        page.markdown.contains("中文内容"),
        "无声明时应按 GBK 兜底解码：\n{}",
        page.markdown
    );
}
