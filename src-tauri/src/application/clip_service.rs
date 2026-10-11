//! 网页剪藏（V1.2.1 阶段 C1）：抓取公开网页 → 抽取正文 → 转 Markdown。
//!
//! 设计取舍：
//! - 只抓 **http/https 公开页面**，不处理登录态、不执行脚本、不递归爬站（单页剪藏）；
//! - 正文抽取用启发式容器（`article` / `main` / `[role=main]` / 常见正文类名），
//!   命中失败回退 `body`；`script/style/nav/footer/aside/form` 等噪音子树整块跳过；
//! - HTML 解析复用 **已在依赖图里** 的 `dom_query`（tauri-utils 已经带入 html5ever），
//!   HTTP 用 `ureq`（rustls + charset），不引入 reqwest/tokio；
//! - 转换结果是**纯 Markdown 正文**；来源元数据（URL / 抓取时间）由调用方写进 frontmatter，
//!   保持「Markdown 是唯一事实来源、单一写入路径」的既有约定。

use std::time::Duration;

use dom_query::{Document, NodeRef};
use serde::{Deserialize, Serialize};

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::text::{count_words, markdown_to_plain_text};
use crate::core::time::now_iso;

/// 单页最多读取的字节数（3 MB）：剪藏不是爬虫，超限直接截断失败。
const MAX_BYTES: u64 = 3 * 1024 * 1024;
/// 转换后 Markdown 的字符上限（12 万），避免一篇剪藏把笔记与索引撑爆。
const MAX_MARKDOWN_CHARS: usize = 120_000;
/// 整体超时（连接 + 读取）。
const TIMEOUT: Duration = Duration::from_secs(20);
const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; PersonalKnowledgeOS/1.2; +local-first note clipper)";

/// 噪声子树：整块跳过，不进入 Markdown。
const NOISE_TAGS: [&str; 15] = [
    "script",
    "style",
    "noscript",
    "iframe",
    "svg",
    "canvas",
    "form",
    "nav",
    "footer",
    "aside",
    "button",
    "select",
    "template",
    "dialog",
    "video",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClippedPage {
    /// 用户输入的原始地址
    pub url: String,
    /// 跟随跳转后的最终地址（相对链接按它转绝对）
    pub final_url: String,
    pub title: String,
    /// 站点名（og:site_name 或域名）
    pub site: Option<String>,
    /// 转换后的 Markdown 正文
    pub markdown: String,
    /// 正文字数（与笔记的字数口径一致）
    pub word_count: i64,
    /// 抓取时间（ISO 8601）
    pub fetched_at: String,
    /// 实际读取的字节数
    pub bytes: usize,
}

/// 解码网页字节。
///
/// 关键事实（读 ureq 源码确认过）：`ureq` 打开 `charset` 特性后，**只有在 Content-Type 里
/// 声明了非 UTF-8 的 charset 时**才会把正文转成 UTF-8 再交给我们；没声明（或声明的是
/// `charset=utf-8`）时是 **PassThrough**，我们拿到的是**原始字节**。
/// 所以这里必须分两种情况处理，否则会对已经解好的文本再解一次（变成乱码）。
///
/// 无声明时的兜底顺序：BOM → HTML `<meta charset>` → 严格 UTF-8 → GBK。
/// 中文站点里「只在 meta 里写 gbk」非常常见，而 GBK 解码能力已经在依赖树里
/// （`ureq` 的 charset 特性带入 `encoding_rs`），这里直接复用，不新增第三方依赖。
fn decode_html(bytes: &[u8], content_type: &str) -> String {
    if ureq_already_decoded(content_type) {
        // ureq 已转成 UTF-8：只做一次容错转换，绝不再按 charset 解一遍
        return String::from_utf8_lossy(bytes).into_owned();
    }
    if let Some((encoding, _)) = encoding_rs::Encoding::for_bom(bytes) {
        let (text, _, _) = encoding.decode(bytes);
        return text.into_owned();
    }
    if let Some(label) = charset_from_meta(bytes) {
        if let Some(encoding) = encoding_rs::Encoding::for_label(label.as_bytes()) {
            let (text, _, _) = encoding.decode(bytes);
            return text.into_owned();
        }
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            // 严格 UTF-8 失败：按 GBK 解（中文网页最常见的非 UTF-8 编码）。
            // 真解错时正文里会出现替换字符——那是「这里可能解错了」的可见信号，不会静默糊掉。
            let (text, _, _) = encoding_rs::GBK.decode(bytes);
            text.into_owned()
        }
    }
}

/// 与 `ureq` 内部同一套判断：Content-Type 里声明了**非 UTF-8** 的 charset 时它已经解码过。
fn ureq_already_decoded(content_type: &str) -> bool {
    charset_from_content_type(content_type)
        .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
        .is_some_and(|encoding| encoding != encoding_rs::UTF_8)
}

/// 从 `Content-Type` 里取 charset（值已小写）。
fn charset_from_content_type(content_type: &str) -> Option<String> {
    let (_, rest) = content_type.split_once("charset=")?;
    let label = rest
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(['"', '\''])
        .to_string();
    (!label.is_empty()).then_some(label)
}

/// 从前 4 KB 里找 `<meta charset=...>`（也覆盖 `content="text/html; charset=..."`）。
fn charset_from_meta(bytes: &[u8]) -> Option<String> {
    let head = &bytes[..bytes.len().min(4096)];
    let text = String::from_utf8_lossy(head).to_ascii_lowercase();
    let index = text.find("charset=")?;
    let rest = &text[index + "charset=".len()..];
    let label: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .collect();
    (!label.is_empty()).then_some(label)
}

/// 抓取并转换一个网页。**只读**，不写库。
pub fn clip(raw_url: &str) -> Result<ClippedPage> {
    let url = normalize_url(raw_url)?;
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .build(),
    );
    let response = agent
        .get(&url)
        .header("User-Agent", USER_AGENT)
        .header("Accept", "text/html,application/xhtml+xml;q=0.9,*/*;q=0.5")
        .call()
        .map_err(|error| {
            AppError::new(
                ErrorCode::ExportFailed,
                format!("抓取失败：{error}（请确认地址可公开访问）"),
            )
        })?;

    let final_url = {
        use ureq::ResponseExt;
        response.get_uri().to_string()
    };
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !content_type.is_empty() && !content_type.contains("html") {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            format!("该地址不是网页（Content-Type: {content_type}），剪藏只支持 HTML 页面。"),
        ));
    }

    let mut response = response;
    // 读**原始字节**再自己解码：Content-Type 不带 charset、只靠 `<meta charset="gbk">` 声明的
    // 中文站点很常见，按 UTF-8 硬读会直接失败（而且旧错误信息把它误报成「超过 3 MB」）。
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .map_err(|error| {
            let too_large = format!("{error}").contains("limit");
            let hint = if too_large {
                format!("页面超过 {} MB，请改用其它方式保存。", MAX_BYTES / 1024 / 1024)
            } else {
                "请检查网络与该地址是否可公开访问。".to_string()
            };
            AppError::new(ErrorCode::ExportFailed, format!("读取网页失败：{error}（{hint}）"))
        })?;
    if bytes.is_empty() {
        return Err(AppError::new(
            ErrorCode::ExportFailed,
            "读取网页失败：页面返回了空内容。".to_string(),
        ));
    }
    let html = decode_html(&bytes, &content_type);

    let (title, markdown, site) = html_to_markdown(&html, &final_url)?;
    if markdown.trim().is_empty() {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "没有抽取到正文（页面可能要求登录、或正文由脚本渲染）。",
        ));
    }
    let word_count = count_words(&markdown_to_plain_text(&markdown)) as i64;
    Ok(ClippedPage {
        url: raw_url.trim().to_string(),
        final_url,
        title,
        site,
        markdown,
        word_count,
        fetched_at: now_iso(),
        bytes: html.len(),
    })
}

/// 地址规范化：缺协议补 `https://`，只接受 http/https。
fn normalize_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("请填写网页地址。"));
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err(AppError::invalid_input("网页地址里不能有空格。"));
    }
    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.contains("://") {
        return Err(AppError::invalid_input("只支持 http / https 地址。"));
    } else {
        format!("https://{trimmed}")
    };
    let host = with_scheme
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .unwrap_or("");
    if host.is_empty() || !host.contains('.') {
        return Err(AppError::invalid_input("网页地址不完整（缺少域名）。"));
    }
    Ok(with_scheme)
}

/// HTML → (标题, Markdown, 站点名)。纯函数，便于单测。
pub fn html_to_markdown(html: &str, base_url: &str) -> Result<(String, String, Option<String>)> {
    let document = Document::from(html);
    let title = page_title(&document, base_url);
    let site = site_name(&document, base_url);
    let root = pick_content_root(&document);
    let mut out = String::new();
    render_node(&root, base_url, &mut out, &mut Context::default());
    let markdown = tidy(&out);
    Ok((title, markdown, site))
}

fn page_title(document: &Document, base_url: &str) -> String {
    if let Some(value) = meta_content(document, "og:title").filter(|value| !value.is_empty()) {
        return value;
    }
    if let Some(h1) = document.select_single("h1").nodes().first() {
        let text = h1.text().trim().to_string();
        if !text.is_empty() && text.chars().count() <= 200 {
            return text;
        }
    }
    let title = document.select_single("title").text().trim().to_string();
    if !title.is_empty() {
        return title;
    }
    // 兜底：用域名，避免剪出一篇没标题的笔记（用户可以再改）
    base_url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .unwrap_or("剪藏")
        .to_string()
}

fn site_name(document: &Document, base_url: &str) -> Option<String> {
    if let Some(value) = meta_content(document, "og:site_name").filter(|value| !value.is_empty()) {
        return Some(value);
    }
    base_url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .map(|host| host.trim_start_matches("www.").to_string())
        .filter(|host| !host.is_empty())
}

fn meta_content(document: &Document, property: &str) -> Option<String> {
    for selector in [
        format!("meta[property=\"{property}\"]"),
        format!("meta[name=\"{property}\"]"),
    ] {
        let selection = document.select(&selector);
        if let Some(node) = selection.nodes().first() {
            if let Some(content) = node.attr("content") {
                let text = content.trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

/// 正文容器启发式：优先语义标签，再看常见正文类名；都不合适时回退 `body`。
fn pick_content_root(document: &Document) -> NodeRef<'_> {
    const CANDIDATES: [&str; 14] = [
        "article",
        "main",
        "[role=main]",
        "#content",
        "#main",
        ".markdown-body",
        ".post-content",
        ".article-content",
        ".entry-content",
        ".content",
        ".post",
        ".article",
        ".rich_media_content", // 微信公众号
        "body",
    ];
    let mut best: Option<(NodeRef<'_>, usize)> = None;
    for selector in CANDIDATES {
        let selection = document.select(selector);
        let Some(node) = selection.nodes().first() else {
            continue;
        };
        let length = node.text().trim().chars().count();
        // 太短的容器（导航/侧栏）不作为正文
        if length < 120 && selector != "body" {
            continue;
        }
        let is_body = selector == "body";
        match &best {
            Some((_, best_len)) if *best_len >= length => {}
            _ => best = Some((node.clone(), length)),
        }
        if is_body {
            break;
        }
    }
    best.map(|(node, _)| node)
        .or_else(|| document.select_single("body").nodes().first().cloned())
        .unwrap_or_else(|| document.html_root())
}

#[derive(Default)]
struct Context {
    /// 列表嵌套深度（用于缩进）
    list_depth: usize,
    /// 是否在 pre 内（保留空白）
    in_pre: bool,
}

fn render_node(node: &NodeRef<'_>, base_url: &str, out: &mut String, ctx: &mut Context) {
    if node.is_text() {
        let text = node.text().to_string();
        if ctx.in_pre {
            out.push_str(&text);
        } else {
            out.push_str(&collapse_ws(&text));
        }
        return;
    }
    let Some(name) = node.node_name().map(|name| name.to_ascii_lowercase()) else {
        return;
    };
    if NOISE_TAGS.contains(&name.as_str()) {
        return;
    }
    match name.as_str() {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = name[1..].parse::<usize>().unwrap_or(1).clamp(1, 6);
            let text = inline(node, base_url);
            if !text.is_empty() {
                out.push_str(&format!("\n\n{} {}\n\n", "#".repeat(level), text));
            }
        }
        "p" => {
            let text = inline(node, base_url);
            if !text.is_empty() {
                out.push_str(&format!("\n\n{text}\n\n"));
            }
        }
        "br" => out.push('\n'),
        "hr" => out.push_str("\n\n---\n\n"),
        "strong" | "b" => wrap(out, node, base_url, ctx, "**", "**"),
        "em" | "i" => wrap(out, node, base_url, ctx, "*", "*"),
        "del" | "s" => wrap(out, node, base_url, ctx, "~~", "~~"),
        "code" => {
            let text = node.text().to_string();
            if ctx.in_pre {
                out.push_str(&text);
            } else if !text.trim().is_empty() {
                out.push_str(&format!("`{}`", text.trim()));
            }
        }
        "pre" => {
            let language = find_first_tag(node, "code")
                .and_then(|code| code.attr("class"))
                .and_then(|class| {
                    class
                        .split_whitespace()
                        .find_map(|item| item.strip_prefix("language-").map(str::to_string))
                })
                .unwrap_or_default();
            let text = node.text().to_string();
            let text = text.trim_end_matches('\n');
            if !text.trim().is_empty() {
                out.push_str(&format!("\n\n```{language}\n{text}\n```\n\n"));
            }
        }
        "a" => {
            let text = inline(node, base_url);
            let href = node.attr("href").map(|value| value.trim().to_string());
            match href {
                Some(href) if !href.starts_with("javascript:") && !text.is_empty() => {
                    let absolute = absolutize(base_url, &href);
                    if absolute == text || href.starts_with('#') {
                        out.push_str(&text);
                    } else {
                        out.push_str(&format!("[{text}]({absolute})"));
                    }
                }
                _ => out.push_str(&text),
            }
        }
        "img" => {
            let src = node
                .attr("src")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty() && !value.starts_with("data:"));
            if let Some(src) = src {
                let alt = node.attr("alt").map(|value| value.trim().to_string()).unwrap_or_default();
                out.push_str(&format!("\n\n![{alt}]({}) \n\n", absolutize(base_url, &src)));
            }
        }
        "ul" | "ol" => {
            out.push('\n');
            let ordered = name == "ol";
            let mut index = 0usize;
            ctx.list_depth += 1;
            for child in node.element_children() {
                if child.node_name().map(|n| n.to_ascii_lowercase()).as_deref() != Some("li") {
                    continue;
                }
                index += 1;
                render_list_item(&child, base_url, out, ctx, ordered, index);
            }
            ctx.list_depth -= 1;
            out.push('\n');
        }
        // 结构异常的 li（没有 ul/ol 父节点）按普通段落处理
        "li" => {
            let text = inline(node, base_url);
            if !text.is_empty() {
                out.push_str(&format!("\n- {text}"));
            }
        }
        "blockquote" => {
            let mut inner = String::new();
            for child in node.children() {
                render_node(&child, base_url, &mut inner, ctx);
            }
            let quoted: Vec<String> = tidy(&inner)
                .lines()
                .map(|line| format!("> {line}"))
                .collect();
            if !quoted.is_empty() {
                out.push_str(&format!("\n\n{}\n\n", quoted.join("\n")));
            }
        }
        "table" => {
            out.push_str(&render_table(node, base_url));
        }
        "figcaption" => {
            let text = inline(node, base_url);
            if !text.is_empty() {
                out.push_str(&format!("\n\n*{text}*\n\n"));
            }
        }
        _ => {
            for child in node.children() {
                render_node(&child, base_url, out, ctx);
            }
        }
    }
}

/// 渲染一个列表项：先输出「标记 + 行内文本」，再渲染嵌套的 ul/ol（保持层级缩进）。
fn render_list_item(
    node: &NodeRef<'_>,
    base_url: &str,
    out: &mut String,
    ctx: &mut Context,
    ordered: bool,
    index: usize,
) {
    let indent = "  ".repeat(ctx.list_depth.saturating_sub(1));
    let marker = if ordered { format!("{index}.") } else { "-".to_string() };

    let mut inline_buffer = String::new();
    let mut nested = String::new();
    for child in node.children() {
        let name = child.node_name().map(|value| value.to_ascii_lowercase());
        match name.as_deref() {
            Some("ul") | Some("ol") => render_node(&child, base_url, &mut nested, ctx),
            _ => render_node(&child, base_url, &mut inline_buffer, ctx),
        }
    }
    let text = tidy_inline(&inline_buffer);
    if !text.is_empty() {
        out.push_str(&format!("\n{indent}{marker} {text}"));
    }
    if !nested.trim().is_empty() {
        out.push_str(&nested);
    }
}

fn wrap(out: &mut String, node: &NodeRef<'_>, base_url: &str, _ctx: &mut Context, open: &str, close: &str) {
    let text = inline(node, base_url);
    if !text.is_empty() {
        out.push_str(&format!("{open}{text}{close}"));
    }
}

/// 行内渲染：把子树拉平成单行（行内元素不加换行）。
fn inline(node: &NodeRef<'_>, base_url: &str) -> String {
    let mut buffer = String::new();
    let mut ctx = Context::default();
    for child in node.children() {
        render_node(&child, base_url, &mut buffer, &mut ctx);
    }
    tidy_inline(&buffer)
}

fn render_table(node: &NodeRef<'_>, base_url: &str) -> String {
    let rows = collect_tags(node, "tr");
    let mut lines: Vec<String> = Vec::new();
    let mut columns = 0usize;
    for row in &rows {
        let cells: Vec<String> = row
            .element_children()
            .iter()
            .filter(|cell| {
                matches!(
                    cell.node_name().map(|name| name.to_ascii_lowercase()).as_deref(),
                    Some("th") | Some("td")
                )
            })
            .map(|cell| inline(cell, base_url).replace('|', "\\|"))
            .collect();
        if cells.is_empty() {
            continue;
        }
        columns = columns.max(cells.len());
        lines.push(format!("| {} |", cells.join(" | ")));
    }
    if lines.is_empty() {
        return String::new();
    }
    let separator = format!("|{}|", " --- |".repeat(columns.max(1)));
    lines.insert(1.min(lines.len()), separator);
    format!("\n\n{}\n\n", lines.join("\n"))
}

/// 深度优先找第一个指定标签（`select` 只在 Selection 上，这里用遍历即可）。
fn find_first_tag<'a>(node: &NodeRef<'a>, tag: &str) -> Option<NodeRef<'a>> {
    for child in node.element_children() {
        let name = child.node_name().map(|value| value.to_ascii_lowercase());
        if name.as_deref() == Some(tag) {
            return Some(child);
        }
        if let Some(found) = find_first_tag(&child, tag) {
            return Some(found);
        }
    }
    None
}

/// 深度优先收集所有指定标签的节点（按文档顺序）。
fn collect_tags<'a>(node: &NodeRef<'a>, tag: &str) -> Vec<NodeRef<'a>> {
    let mut found = Vec::new();
    for child in node.element_children() {
        let name = child.node_name().map(|value| value.to_ascii_lowercase());
        if name.as_deref() == Some(tag) {
            found.push(child.clone());
        }
        found.extend(collect_tags(&child, tag));
    }
    found
}

/// 相对链接转绝对；已是绝对地址（或协议相对）时原样返回（补协议）。
fn absolutize(base_url: &str, target: &str) -> String {
    let target = target.trim();
    if target.starts_with("http://") || target.starts_with("https://") {
        return target.to_string();
    }
    if let Some(rest) = target.strip_prefix("//") {
        let scheme = base_url.split("://").next().unwrap_or("https");
        return format!("{scheme}://{rest}");
    }
    let (scheme, rest) = match base_url.split_once("://") {
        Some((scheme, rest)) => (scheme, rest),
        None => return target.to_string(),
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if target.starts_with('/') {
        return format!("{scheme}://{host}{target}");
    }
    // 相对路径：按 base 的目录拼接（不做 .. 归一化，够用且不会丢信息）
    let path = rest.split(['?', '#']).next().unwrap_or("");
    let dir = match path.rfind('/') {
        Some(index) if index > 0 => &path[..=index],
        _ => "/",
    };
    format!("{scheme}://{host}{dir}{target}")
}

/// 空白折叠：连续空白 → 单个空格（行内用，保留中文原样）。
fn collapse_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    out
}

/// 行内文本整理：折叠空白 + 去掉首尾空格（中文标点前后不加空格）。
fn tidy_inline(text: &str) -> String {
    collapse_ws(text).trim().to_string()
}

/// 整篇整理：折叠 3+ 连续换行、去掉行尾空白、限制长度。
fn tidy(markdown: &str) -> String {
    let mut compact = String::with_capacity(markdown.len());
    let mut newlines = 0usize;
    for line in markdown.lines() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            newlines += 1;
            if newlines > 1 {
                continue;
            }
        } else {
            newlines = 0;
        }
        compact.push_str(trimmed);
        compact.push('\n');
    }
    let trimmed = compact.trim();
    if trimmed.chars().count() > MAX_MARKDOWN_CHARS {
        let mut limited: String = trimmed.chars().take(MAX_MARKDOWN_CHARS).collect();
        limited.push_str("\n\n> 内容超过 12 万字，其余部分已截断（可打开原网页查看完整内容）。\n");
        return limited;
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"<!doctype html>
<html><head>
  <title>朴素标题 - 某站</title>
  <meta property="og:title" content="深入理解 SQLite FTS5" />
  <meta property="og:site_name" content="技术小站" />
  <style>body { color: red }</style>
</head><body>
  <nav><a href="/">首页</a><a href="/about">关于</a></nav>
  <article>
    <h1>深入理解 SQLite FTS5</h1>
    <p>FTS5 是 SQLite 的全文检索模块，支持 <strong>短语查询</strong> 与 <em>前缀匹配</em>。</p>
    <h2>中文处理</h2>
    <p>中文需要自己做分词扩展，常见做法是单字 + 相邻双字，例如 <code>全文搜索</code>。</p>
    <ul>
      <li>单字词条</li>
      <li>相邻双字
        <ul><li>嵌套项</li></ul>
      </li>
    </ul>
    <blockquote><p>引用：索引体积约为正文的 2~3 倍。</p></blockquote>
    <pre><code class="language-sql">CREATE VIRTUAL TABLE docs USING fts5(title, body);</code></pre>
    <table>
      <tr><th>方案</th><th>体积</th></tr>
      <tr><td>单字</td><td>大</td></tr>
    </table>
    <p>参考 <a href="/posts/fts5">官方文档</a> 与 <a href="https://sqlite.org/fts5.html">FTS5 文档</a>。</p>
    <p><img src="/img/fts5.png" alt="架构图" /></p>
    <script>console.log('不该出现')</script>
  </article>
  <aside>侧栏广告不该出现</aside>
  <footer>版权所有不该出现</footer>
</body></html>"#;

    #[test]
    fn converts_article_html_to_markdown() {
        let (title, markdown, site) =
            html_to_markdown(FIXTURE, "https://example.com/posts/2026/09/23/a.html").expect("转换");

        assert_eq!(title, "深入理解 SQLite FTS5", "标题优先取 og:title");
        assert_eq!(site.as_deref(), Some("技术小站"));

        assert!(markdown.contains("## 中文处理"), "{markdown}");
        assert!(markdown.contains("**短语查询**"), "粗体应保留：{markdown}");
        assert!(markdown.contains("*前缀匹配*"), "斜体应保留：{markdown}");
        assert!(markdown.contains("`全文搜索`"), "行内代码应保留：{markdown}");
        assert!(markdown.contains("- 单字词条"), "{markdown}");
        assert!(markdown.contains("  - 嵌套项"), "嵌套列表应缩进：{markdown}");
        assert!(markdown.contains("> 引用：索引体积"), "引用应加 >：{markdown}");
        assert!(
            markdown.contains("```sql\nCREATE VIRTUAL TABLE"),
            "代码块应带语言：{markdown}"
        );
        assert!(markdown.contains("| 方案 | 体积 |"), "表格应转 GFM：{markdown}");
        assert!(markdown.contains("| --- | --- |"), "{markdown}");
        // 根相对路径 → 主机 + 路径；绝对链接保持不变
        assert!(
            markdown.contains("[官方文档](https://example.com/posts/fts5)"),
            "相对链接应转绝对：{markdown}"
        );
        assert!(
            markdown.contains("](https://sqlite.org/fts5.html)"),
            "绝对链接保持：{markdown}"
        );
        assert!(
            markdown.contains("![架构图](https://example.com/img/fts5.png)"),
            "图片应转绝对地址：{markdown}"
        );

        // 噪音子树不得出现
        for noise in ["该出现", "侧栏广告", "版权所有", "console.log", "color: red"] {
            assert!(
                !markdown.contains(noise),
                "噪音 `{noise}` 不应进入正文：{markdown}"
            );
        }
    }

    #[test]
    fn falls_back_to_body_when_no_container_matches() {
        let html = "<html><body><div><p>只有一段普通正文，没有任何语义容器。</p></div></body></html>";
        let (title, markdown, _) = html_to_markdown(html, "https://example.com/x").expect("转换");
        assert!(markdown.contains("只有一段普通正文"));
        assert_eq!(title, "example.com", "没有标题时回退域名");
    }

    #[test]
    fn rejects_non_http_and_incomplete_urls() {
        assert!(normalize_url("").is_err());
        assert!(normalize_url("file:///etc/passwd").is_err());
        assert!(normalize_url("javascript:alert(1)").is_err());
        assert!(normalize_url("https://a b.com").is_err());
        assert!(normalize_url("没有域名").is_err());
        assert_eq!(
            normalize_url("example.com/a").expect("补协议"),
            "https://example.com/a"
        );
    }

    #[test]
    fn title_falls_back_to_html_title_then_domain() {
        // 有 <title> 但没有 h1 / og:title → 用 <title>
        let html = "<html><head><title>站点里的文章标题</title></head><body><p>正文一段。</p></body></html>";
        let (title, _, _) = html_to_markdown(html, "https://example.com/a/b").expect("转换");
        assert_eq!(title, "站点里的文章标题");

        // 什么都没有 → 用域名兜底（避免剪出无标题笔记）
        let bare = "<html><body><p>只有正文。</p></body></html>";
        let (title, _, _) = html_to_markdown(bare, "https://blog.example.com/post").expect("转换");
        assert_eq!(title, "blog.example.com");
    }

    #[test]
    fn aborts_oversized_markdown() {
        let long = "段落。".repeat(60_000);
        let html = format!("<html><body><article><p>{long}</p></article></body></html>");
        let (_, markdown, _) = html_to_markdown(&html, "https://example.com/big").expect("转换");
        assert!(markdown.chars().count() <= MAX_MARKDOWN_CHARS + 100);
        assert!(markdown.contains("已截断"));
    }
}
