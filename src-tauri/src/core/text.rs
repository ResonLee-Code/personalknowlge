//! 文本处理：文件名规范化、Markdown 转纯文本、中文搜索分词、摘要与高亮。

use std::collections::BTreeSet;
use std::sync::OnceLock;

use regex::Regex;

/// 文件名（不含扩展名）的最大字符数，避免超出常见文件系统限制。
pub const MAX_FILE_STEM_CHARS: usize = 120;
/// 单次关键词的最大长度，防止异常搜索表达式。
pub const MAX_KEYWORD_CHARS: usize = 200;

fn re(regex: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    regex.get_or_init(|| Regex::new(pattern).expect("内置正则表达式必须合法"))
}

/// 判断字符是否属于中日韩文字（含假名与谚文），这类文字需要特殊分词处理。
pub fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x3400..=0x4DBF     // CJK 扩展 A
        | 0x4E00..=0x9FFF   // CJK 基本区
        | 0xF900..=0xFAFF   // CJK 兼容表意
        | 0x3040..=0x30FF   // 平假名 / 片假名
        | 0xAC00..=0xD7AF   // 谚文
    )
}

/// 清理文件名：去掉非法字符、控制字符，避免路径穿越与保留名称冲突。
pub fn sanitize_file_name(input: &str) -> String {
    let mut cleaned = String::with_capacity(input.len());
    for ch in input.chars() {
        let illegal = matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\0');
        if illegal || ch.is_control() {
            cleaned.push('-');
        } else {
            cleaned.push(ch);
        }
    }

    // 折叠连续分隔符
    let mut collapsed = String::with_capacity(cleaned.len());
    let mut previous_dash = false;
    for ch in cleaned.chars() {
        let is_dash = ch == '-' || ch.is_whitespace();
        if is_dash {
            if !previous_dash {
                collapsed.push('-');
            }
            previous_dash = true;
        } else {
            collapsed.push(ch);
            previous_dash = false;
        }
    }

    let trimmed = collapsed.trim_matches(|c: char| c == '-' || c == '.' || c.is_whitespace());
    let mut result: String = trimmed.chars().take(MAX_FILE_STEM_CHARS).collect();
    result = result.trim_matches(|c: char| c == '-' || c == '.').to_string();

    if result.is_empty() {
        result.push_str("note");
    }

    // Windows 保留名称
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.iter().any(|name| result.eq_ignore_ascii_case(name)) {
        result = format!("note-{result}");
    }

    result
}

/// 生成 Markdown 文件名：`YYYYMMDD-HHmmss-标题.md`（设计文档 12.2）。
pub fn note_file_name(title: &str, timestamp: &str) -> String {
    let stem = sanitize_file_name(title);
    let stem = if stem == "note" && title.trim().is_empty() {
        "untitled".to_string()
    } else {
        stem
    };
    format!("{timestamp}-{stem}.md")
}

/// 去掉 Markdown 语法，得到用于索引与摘要的纯文本。
pub fn markdown_to_plain_text(markdown: &str) -> String {
    static IMAGE: OnceLock<Regex> = OnceLock::new();
    static LINK: OnceLock<Regex> = OnceLock::new();
    static HEADING: OnceLock<Regex> = OnceLock::new();
    static LIST: OnceLock<Regex> = OnceLock::new();
    static QUOTE: OnceLock<Regex> = OnceLock::new();
    static HTML: OnceLock<Regex> = OnceLock::new();
    static WIKILINK: OnceLock<Regex> = OnceLock::new();
    static BLANK: OnceLock<Regex> = OnceLock::new();

    let image = re(&IMAGE, r"!\[([^\]]*)\]\([^)]*\)");
    let link = re(&LINK, r"\[([^\]]*)\]\(([^)]*)\)");
    let heading = re(&HEADING, r"(?m)^\s{0,3}#{1,6}\s*");
    let list = re(&LIST, r"(?m)^\s*(?:[-*+]|\d+[.)])\s+");
    let quote = re(&QUOTE, r"(?m)^\s*>\s?");
    let html = re(&HTML, r"</?[A-Za-z][^>]*>");
    let wikilink = re(&WIKILINK, r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]");
    let blank = re(&BLANK, r"\n{3,}");

    let mut text = strip_frontmatter(markdown).to_string();
    text = image.replace_all(&text, "$1").into_owned();
    text = link.replace_all(&text, "$1").into_owned();
    text = wikilink
        .replace_all(&text, |caps: &regex::Captures<'_>| {
            caps.get(2).or_else(|| caps.get(1)).map(|m| m.as_str()).unwrap_or("").to_string()
        })
        .into_owned();
    text = heading.replace_all(&text, "").into_owned();
    text = list.replace_all(&text, "").into_owned();
    text = quote.replace_all(&text, "").into_owned();
    text = html.replace_all(&text, "").into_owned();

    // 去掉围栏代码块标记，但保留代码内容（便于搜索代码片段）
    let mut out = String::with_capacity(text.len());
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    let out = out
        .replace('`', "")
        .replace("**", "")
        .replace("__", "")
        .replace("~~", "");
    let out = blank.replace_all(&out, "\n\n").into_owned();
    out.trim().to_string()
}

/// 去掉 frontmatter，保留正文。
pub fn strip_frontmatter(markdown: &str) -> &str {
    let normalized = markdown.trim_start_matches('\u{feff}');
    if !normalized.starts_with("---") {
        return normalized;
    }
    let after_first = &normalized[3..];
    let after_first = after_first.strip_prefix('\r').unwrap_or(after_first);
    let after_first = after_first.strip_prefix('\n').unwrap_or(after_first);
    if let Some(end) = find_frontmatter_end(after_first) {
        return &after_first[end..];
    }
    normalized
}

fn find_frontmatter_end(text: &str) -> Option<usize> {
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.trim() == "---" || trimmed.trim() == "..." {
            return Some(offset + line.len());
        }
        offset += line.len();
    }
    None
}

/// 统计字数：中日韩文字按字计数，其他语言按词计数。
pub fn count_words(text: &str) -> i64 {
    static WORD: OnceLock<Regex> = OnceLock::new();
    let word = re(&WORD, r"[A-Za-z0-9_'\u{00C0}-\u{024F}]+");

    let mut cjk = 0i64;
    for ch in text.chars() {
        if is_cjk(ch) {
            cjk += 1;
        }
    }
    let latin = word.find_iter(text).count() as i64;
    cjk + latin
}

/// 为 FTS5 生成中日韩检索词：单字（支持单字查询）+ 双字（支持短语查询）。
///
/// SQLite 的 unicode61 分词器会把连续中文当成一个整词，
/// 因此「全文搜索」无法通过「全文」命中。这里额外写入单字与相邻双字，
/// 使中文查询可以按「双字短语」精确匹配（设计文档 11.5）。
pub fn cjk_search_tokens(text: &str) -> String {
    let mut unigrams: Vec<String> = Vec::new();
    let mut bigrams: Vec<String> = Vec::new();

    for token in text.split(|c: char| !is_cjk(c)) {
        if token.is_empty() {
            continue;
        }
        let chars: Vec<char> = token.chars().collect();
        for ch in &chars {
            unigrams.push(ch.to_string());
        }
        for window in chars.windows(2) {
            bigrams.push(format!("{}{}", window[0], window[1]));
        }
    }

    let mut out = String::new();
    for token in unigrams.iter().chain(bigrams.iter()) {
        out.push_str(token);
        out.push(' ');
    }
    out.trim_end().to_string()
}

/// 标签名规范化：去首尾空白并折叠内部空白。
pub fn normalize_tag_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 标签唯一性比较用的键（不区分大小写，设计文档 FR-T01）。
pub fn tag_key(name: &str) -> String {
    normalize_tag_name(name).to_lowercase()
}

/// 按字符安全截断。
pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push('…');
    out
}

/// 查询词的一段：ASCII 词或中日韩连续片段。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuerySegment {
    Ascii(String),
    Cjk(String),
}

/// 把关键词切分成 ASCII 片段与中日韩片段。
pub fn split_query_segments(keyword: &str) -> Vec<QuerySegment> {
    let mut segments = Vec::new();
    let mut buffer = String::new();
    let mut buffer_is_cjk = false;

    let flush = |buffer: &mut String, is_cjk: bool, segments: &mut Vec<QuerySegment>| {
        if buffer.is_empty() {
            return;
        }
        let value = std::mem::take(buffer);
        if is_cjk {
            segments.push(QuerySegment::Cjk(value));
        } else {
            segments.push(QuerySegment::Ascii(value));
        }
    };

    for ch in keyword.chars() {
        if ch.is_whitespace() {
            flush(&mut buffer, buffer_is_cjk, &mut segments);
            continue;
        }
        let ch_is_cjk = is_cjk(ch);
        if !buffer.is_empty() && ch_is_cjk != buffer_is_cjk {
            flush(&mut buffer, buffer_is_cjk, &mut segments);
        }
        buffer_is_cjk = ch_is_cjk;
        buffer.push(ch);
    }
    flush(&mut buffer, buffer_is_cjk, &mut segments);

    segments
}

/// 构建安全的 FTS5 MATCH 表达式。
///
/// 返回 `None` 表示关键词为空（调用方应回退到「最近编辑」列表）。
pub fn build_fts_match(keyword: &str) -> Option<String> {
    let trimmed: String = keyword.chars().take(MAX_KEYWORD_CHARS).collect();
    let mut clauses: Vec<String> = Vec::new();

    // 先按空白切块：**混合写法**（中日韩 + 拉丁紧邻，如 `术语A7F3`、`SQLite索引`）
    // 在索引侧是「一个整 token」（unicode61 不按文字种类再切），因此必须用整段前缀匹配；
    // 若照旧拆成「CJK 双字 AND 拉丁前缀」，两个条件都会落空（实测 recall@5 = 0%）。
    let mut plain_chunks: Vec<String> = Vec::new();
    for chunk in trimmed.split_whitespace() {
        let has_cjk = chunk.chars().any(is_cjk);
        let has_latin = chunk
            .chars()
            .any(|ch| ch.is_ascii_alphanumeric() && !ch.is_ascii_digit() || ch.is_ascii_digit());
        if has_cjk && has_latin {
            let cleaned: String = chunk
                .chars()
                .filter(|ch| !ch.is_whitespace() && *ch != '"')
                .collect();
            if !cleaned.is_empty() {
                // 引号包裹避免把用户输入当成 FTS5 语法，尾部 * 表示前缀匹配
                clauses.push(format!("\"{}\"*", cleaned.to_lowercase()));
            }
            continue;
        }
        plain_chunks.push(chunk.to_string());
    }

    let segments = split_query_segments(&plain_chunks.join(" "));
    for segment in segments {
        match segment {
            QuerySegment::Ascii(word) => {
                let cleaned: String = word
                    .chars()
                    .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '+'))
                    .collect();
                if cleaned.is_empty() {
                    continue;
                }
                // 去掉 FTS5 语法字符后再加前缀匹配，避免用户输入被当成语法
                let escaped = cleaned.replace('"', "");
                if escaped.chars().count() >= 2 {
                    clauses.push(format!("{escaped}*"));
                } else {
                    clauses.push(escaped);
                }
            }
            QuerySegment::Cjk(run) => {
                let chars: Vec<char> = run.chars().collect();
                match chars.len() {
                    0 => continue,
                    1 => clauses.push(chars[0].to_string()),
                    _ => {
                        let mut bigrams = Vec::with_capacity(chars.len() - 1);
                        for window in chars.windows(2) {
                            bigrams.push(format!("{}{}", window[0], window[1]));
                        }
                        clauses.push(format!("\"{}\"", bigrams.join(" ")));
                    }
                }
            }
        }
    }

    if clauses.is_empty() {
        None
    } else {
        Some(clauses.join(" AND "))
    }
}

/// 用于摘要与高亮的匹配词（按长度倒序，优先匹配更长的词）。
pub fn highlight_terms(keyword: &str) -> Vec<String> {
    let mut terms: BTreeSet<String> = BTreeSet::new();
    for segment in split_query_segments(keyword) {
        match segment {
            QuerySegment::Ascii(word) => {
                let cleaned: String = word
                    .chars()
                    .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
                    .collect();
                if !cleaned.is_empty() {
                    terms.insert(cleaned);
                }
            }
            QuerySegment::Cjk(run) => {
                if !run.is_empty() {
                    terms.insert(run);
                }
            }
        }
    }
    let mut list: Vec<String> = terms.into_iter().collect();
    list.sort_by_key(|term| std::cmp::Reverse(term.chars().count()));
    list
}

/// 在纯文本中查找所有匹配位置（字符下标，左闭右开）。
pub fn find_match_ranges(text: &str, keyword: &str, limit_per_term: usize) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let lower_chars: Vec<char> = text.to_lowercase().chars().collect();
    let mut ranges: Vec<(usize, usize)> = Vec::new();

    for term in highlight_terms(keyword) {
        if term.is_empty() {
            continue;
        }
        let term_chars: Vec<char> = term.chars().collect();
        let term_lower: Vec<char> = term.to_lowercase().chars().collect();
        if term_lower.is_empty() || term_lower.len() > lower_chars.len() {
            continue;
        }
        let mut found = 0usize;
        let mut start = 0usize;
        while start + term_lower.len() <= lower_chars.len() && found < limit_per_term {
            let mut matched = true;
            for offset in 0..term_lower.len() {
                let hay = lower_chars[start + offset];
                let needle = term_lower[offset];
                if hay != needle {
                    matched = false;
                    break;
                }
            }
            if matched {
                let end = (start + term_chars.len()).min(chars.len());
                ranges.push((start, end));
                found += 1;
                start = start + term_lower.len().max(1);
            } else {
                start += 1;
            }
        }
    }

    ranges.sort_unstable();
    // 合并重叠区间
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// 摘要片段：带高亮标记的文本段。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSegment {
    pub text: String,
    pub matched: bool,
}

/// 生成搜索结果摘要，并返回可直接渲染的高亮片段。
pub fn build_snippet(text: &str, keyword: &str, max_chars: usize) -> Vec<TextSegment> {
    let chars: Vec<char> = text.chars().collect();
    let ranges = find_match_ranges(text, keyword, 20);

    let (window_start, window_end) = if ranges.is_empty() {
        (0usize, chars.len().min(max_chars))
    } else {
        let first = ranges[0].0;
        let start = first.saturating_sub(max_chars / 3);
        let end = (start + max_chars).min(chars.len());
        let start = end.saturating_sub(max_chars).min(start);
        (start, end)
    };

    let mut segments: Vec<TextSegment> = Vec::new();
    let mut cursor = window_start;
    let push_plain = |segments: &mut Vec<TextSegment>, from: usize, to: usize| {
        if to > from {
            let value: String = chars[from..to].iter().collect();
            if !value.is_empty() {
                segments.push(TextSegment { text: value, matched: false });
            }
        }
    };

    for (start, end) in ranges {
        if end <= window_start || start >= window_end {
            continue;
        }
        let start = start.max(window_start);
        let end = end.min(window_end);
        if start > cursor {
            push_plain(&mut segments, cursor, start);
        }
        let value: String = chars[start..end].iter().collect();
        segments.push(TextSegment { text: value, matched: true });
        cursor = end;
    }
    if cursor < window_end {
        push_plain(&mut segments, cursor, window_end);
    }

    if window_start > 0 {
        if let Some(first) = segments.first_mut() {
            first.text.insert(0, '…');
        }
    }
    if window_end < chars.len() {
        if let Some(last) = segments.last_mut() {
            last.text.push('…');
        }
    }
    if segments.is_empty() {
        segments.push(TextSegment { text: String::new(), matched: false });
    }
    segments
}

// ------------------------------------------------------------------ 双向链接

/// 从 Markdown 中解析出的一个 `[[…]]` 链接。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WikiLinkMatch {
    /// 目标标题（`[[标题|别名]]` 中的「标题」）
    pub target: String,
    /// 显示别名
    pub alias: Option<String>,
    /// `[[标题#小节]]` 中的小节
    pub anchor: Option<String>,
    /// 在正文中的出现顺序（从 0 开始）
    pub position: usize,
    /// 在正文中的字符下标（用于按出现顺序合并其它类型的链接）
    pub offset: usize,
    /// 命中所在行的上下文摘要（最多 120 字）
    pub context: String,
}

/// 从 Markdown 中解析出的一个标准链接 `[文字](目标)`。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownLinkMatch {
    pub target: String,
    pub text: Option<String>,
    pub offset: usize,
    pub context: String,
    /// http/https 外链
    pub is_external: bool,
}

/// 上下文摘要的最大长度。
const LINK_CONTEXT_CHARS: usize = 120;

/// 解析正文中的双向链接。
///
/// 规则（对应 `docs/PLAN_V1.1.md` 的 F1）：
/// - 支持 `[[标题]]`、`[[标题|别名]]`、`[[标题#小节]]`、`[[标题#小节|别名]]`；
/// - `\|` 与 `\#` 视为字面量，便于标题里出现这些字符；
/// - **忽略围栏代码块与行内代码**中的 `[[…]]`；
/// - 链接不跨行；目标为空则丢弃。
pub fn parse_wikilinks(markdown: &str) -> Vec<WikiLinkMatch> {
    let chars: Vec<char> = markdown.chars().collect();
    let parsable = mask_code_regions(&chars);
    let mut matches = Vec::new();
    let mut index = 0usize;

    while index + 1 < chars.len() {
        if parsable[index] && chars[index] == '[' && chars[index + 1] == '[' {
            if let Some(end) = find_wikilink_end(&chars, &parsable, index + 2) {
                let inner: String = chars[index + 2..end].iter().collect();
                if let Some((target, alias, anchor)) = parse_wikilink_inner(&inner) {
                    matches.push(WikiLinkMatch {
                        target,
                        alias,
                        anchor,
                        position: matches.len(),
                        offset: index,
                        context: line_context(&chars, index),
                    });
                }
                index = end + 2;
                continue;
            }
        }
        index += 1;
    }
    matches
}

/// 解析 `[文字](目标)` 形式的链接（与双向链接共用「忽略代码区域」规则）。
pub fn parse_markdown_links(markdown: &str) -> Vec<MarkdownLinkMatch> {
    let chars: Vec<char> = markdown.chars().collect();
    let parsable = mask_code_regions(&chars);
    let mut matches = Vec::new();
    let mut index = 0usize;

    while index + 2 < chars.len() {
        if parsable[index] && chars[index] == ']' && chars[index + 1] == '(' {
            // 向前找配对的 `[`
            let mut open = None;
            let mut probe = index;
            while probe > 0 {
                probe -= 1;
                if chars[probe] == '\n' {
                    break;
                }
                if chars[probe] == '[' {
                    open = Some(probe);
                    break;
                }
            }
            if let Some(open) = open {
                // 找 `)` 结束
                let mut end = None;
                let mut cursor = index + 2;
                while cursor < chars.len() {
                    if chars[cursor] == '\n' {
                        break;
                    }
                    if chars[cursor] == ')' {
                        end = Some(cursor);
                        break;
                    }
                    cursor += 1;
                }
                if let Some(end) = end {
                    let text: String = chars[open + 1..index].iter().collect();
                    let raw_target: String = chars[index + 2..end].iter().collect();
                    let target = raw_target.trim();
                    if !target.is_empty() && !target.starts_with('#') {
                        let is_external =
                            target.starts_with("http://") || target.starts_with("https://");
                        matches.push(MarkdownLinkMatch {
                            target: target.to_string(),
                            text: Some(text.trim().to_string()).filter(|value| !value.is_empty()),
                            offset: open,
                            context: line_context(&chars, open),
                            is_external,
                        });
                    }
                    index = end + 1;
                    continue;
                }
            }
        }
        index += 1;
    }
    matches
}

/// 把 `[[旧标题…]]` 改写为 `[[新标题…]]`（保留锚点与别名），返回新文本与改写处数。
///
/// 用于「重命名笔记后自动重连」：只改写双向链接的目标部分，
/// 代码块与行内代码中的内容保持不变。
pub fn rewrite_wikilink_target(body: &str, old_title: &str, new_title: &str) -> (String, usize) {
    let chars: Vec<char> = body.chars().collect();
    let parsable = mask_code_regions(&chars);
    let old_key = old_title.trim().to_lowercase();
    let mut out = String::with_capacity(body.len());
    let mut index = 0usize;
    let mut rewritten = 0usize;

    while index < chars.len() {
        if parsable[index]
            && chars[index] == '['
            && index + 1 < chars.len()
            && chars[index + 1] == '['
        {
            if let Some(end) = find_wikilink_end(&chars, &parsable, index + 2) {
                let inner: String = chars[index + 2..end].iter().collect();
                if let Some((target, alias, anchor)) = parse_wikilink_inner(&inner) {
                    if target.trim().to_lowercase() == old_key {
                        out.push_str("[[");
                        out.push_str(new_title);
                        if let Some(anchor) = &anchor {
                            out.push('#');
                            out.push_str(anchor);
                        }
                        if let Some(alias) = &alias {
                            out.push('|');
                            out.push_str(alias);
                        }
                        out.push_str("]]");
                        rewritten += 1;
                        index = end + 2;
                        continue;
                    }
                }
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    (out, rewritten)
}

/// 找出 `[[` 之后的结束位置（`]` 的下标）。
fn find_wikilink_end(chars: &[char], parsable: &[bool], start: usize) -> Option<usize> {
    let mut index = start;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\n' {
            return None;
        }
        if ch == ']' && index + 1 < chars.len() && chars[index + 1] == ']' && parsable[index] {
            return Some(index);
        }
        // 出现新的 `[[` 说明前一个未闭合
        if ch == '[' && index + 1 < chars.len() && chars[index + 1] == '[' {
            return None;
        }
        index += 1;
    }
    None
}

/// 解析 `[[…]]` 内部文本：返回（目标、别名、锚点）。
fn parse_wikilink_inner(inner: &str) -> Option<(String, Option<String>, Option<String>)> {
    let (target_part, alias_part) = split_unescaped(inner, '|');
    let (title_part, anchor_part) = split_unescaped(&target_part, '#');

    let target = unescape_link_text(title_part.trim());
    if target.is_empty() {
        return None;
    }
    let alias = alias_part
        .map(|value| unescape_link_text(value.trim()))
        .filter(|value| !value.is_empty());
    let anchor = anchor_part
        .map(|value| unescape_link_text(value.trim()))
        .filter(|value| !value.is_empty());

    Some((target, alias, anchor))
}

/// 按未转义的分隔符切成两段。
fn split_unescaped(value: &str, separator: char) -> (String, Option<String>) {
    let chars: Vec<char> = value.chars().collect();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == '\\' {
            index += 2;
            continue;
        }
        if chars[index] == separator {
            let left: String = chars[..index].iter().collect();
            let right: String = chars[index + 1..].iter().collect();
            return (left, Some(right));
        }
        index += 1;
    }
    (value.to_string(), None)
}

fn unescape_link_text(value: &str) -> String {
    value.replace("\\|", "|").replace("\\#", "#").replace("\\]", "]")
}

/// 取某一字符位置所在行的上下文摘要。
///
/// 行太长时以**链接为中心**截断（而不是从行首截），
/// 否则反链面板里会看不到命中的位置。
fn line_context(chars: &[char], at: usize) -> String {
    let line_start = chars[..at]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let line_end = chars[at..]
        .iter()
        .position(|ch| *ch == '\n')
        .map(|offset| at + offset)
        .unwrap_or(chars.len());
    let line: Vec<char> = chars[line_start..line_end].to_vec();

    if line.len() <= LINK_CONTEXT_CHARS {
        return line.iter().collect::<String>().trim().to_string();
    }

    let offset = at - line_start;
    let half = LINK_CONTEXT_CHARS / 2;
    let mut from = offset.saturating_sub(half);
    let to = (from + LINK_CONTEXT_CHARS).min(line.len());
    if to - from < LINK_CONTEXT_CHARS {
        from = to.saturating_sub(LINK_CONTEXT_CHARS);
    }

    let mut out = String::new();
    if from > 0 {
        out.push('…');
    }
    out.push_str(&line[from..to].iter().collect::<String>().trim());
    if to < line.len() {
        out.push('…');
    }
    out
}

/// 标记可解析位置：围栏代码块与行内代码中的内容不可解析。
fn mask_code_regions(chars: &[char]) -> Vec<bool> {
    let mut parsable = vec![true; chars.len()];

    // 1) 围栏代码块（``` 或 ~~~，允许更长围栏）
    let mut line_start = 0usize;
    let mut in_fence = false;
    let mut fence_char = '`';
    let mut fence_len = 0usize;
    let mut index = 0usize;
    loop {
        let at_end = index == chars.len();
        if at_end || chars[index] == '\n' {
            let line: String = chars[line_start..index].iter().collect();
            let trimmed = line.trim();
            let fence_run = trimmed.chars().take_while(|ch| *ch == '`' || *ch == '~').count();
            let is_fence = fence_run >= 3
                && trimmed.chars().all(|ch| ch == '`' || ch == '~' || ch.is_whitespace() || !ch.is_alphanumeric() == false)
                && trimmed.chars().next().is_some_and(|ch| ch == '`' || ch == '~');
            let marker = trimmed.chars().next().unwrap_or('`');

            if is_fence {
                if !in_fence {
                    in_fence = true;
                    fence_char = marker;
                    fence_len = fence_run;
                } else if marker == fence_char && fence_run >= fence_len {
                    in_fence = false;
                }
                for offset in line_start..index {
                    parsable[offset] = false;
                }
            } else if in_fence {
                for offset in line_start..index {
                    parsable[offset] = false;
                }
            }
            line_start = index + 1;
        }
        if at_end {
            break;
        }
        index += 1;
    }

    // 2) 行内代码（成对的反引号，不跨行）
    let mut cursor = 0usize;
    while cursor < chars.len() {
        if chars[cursor] == '`' && parsable[cursor] {
            let mut run = 1usize;
            while cursor + run < chars.len() && chars[cursor + run] == '`' && parsable[cursor + run] {
                run += 1;
            }
            let mut probe = cursor + run;
            let mut closing: Option<usize> = None;
            while probe < chars.len() {
                if chars[probe] == '\n' {
                    break;
                }
                if chars[probe] == '`' && parsable[probe] {
                    let mut close_run = 1usize;
                    while probe + close_run < chars.len()
                        && chars[probe + close_run] == '`'
                        && parsable[probe + close_run]
                    {
                        close_run += 1;
                    }
                    if close_run == run {
                        closing = Some(probe);
                        break;
                    }
                    probe += close_run;
                    continue;
                }
                probe += 1;
            }
            match closing {
                Some(end) => {
                    for offset in cursor..(end + run) {
                        parsable[offset] = false;
                    }
                    cursor = end + run;
                }
                None => cursor += run,
            }
        } else {
            cursor += 1;
        }
    }

    parsable
}

#[cfg(test)]
mod wikilink_tests {
    use super::*;

    #[test]
    fn parses_basic_alias_and_anchor_forms() {
        let matches = parse_wikilinks(
            "参考 [[SQLite 全文搜索]]，别名写法 [[全文搜索|搜索笔记]]，锚点 [[设计文档#索引策略]]，组合 [[设计文档#索引策略|策略]]。",
        );
        assert_eq!(matches.len(), 4);

        assert_eq!(matches[0].target, "SQLite 全文搜索");
        assert_eq!(matches[0].alias, None);
        assert_eq!(matches[0].anchor, None);
        assert_eq!(matches[0].position, 0);

        assert_eq!(matches[1].target, "全文搜索");
        assert_eq!(matches[1].alias.as_deref(), Some("搜索笔记"));

        assert_eq!(matches[2].target, "设计文档");
        assert_eq!(matches[2].anchor.as_deref(), Some("索引策略"));

        assert_eq!(matches[3].target, "设计文档");
        assert_eq!(matches[3].anchor.as_deref(), Some("索引策略"));
        assert_eq!(matches[3].alias.as_deref(), Some("策略"));
        assert_eq!(matches[3].position, 3);
    }

    #[test]
    fn ignores_links_inside_code() {
        let markdown = "正文 [[真链接]]\n\n```\n[[代码块里的链接]]\n```\n\n行内 `[[行内代码]]` 结束。\n\n~~~md\n[[另一个代码块]]\n~~~\n";
        let matches = parse_wikilinks(markdown);
        assert_eq!(matches.len(), 1, "只应解析正文中的链接：{matches:?}");
        assert_eq!(matches[0].target, "真链接");
    }

    #[test]
    fn supports_escaped_separators_and_skips_empty_targets() {
        let matches = parse_wikilinks("[[A\\|B]] 与 [[ ]]，还有 [[#只有锚点]]");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].target, "A|B");
        assert_eq!(matches[0].alias, None);
    }

    #[test]
    fn does_not_span_lines() {
        let matches = parse_wikilinks("[[未闭合\n换行]]");
        assert!(matches.is_empty());
    }

    #[test]
    fn captures_line_context_and_truncates() {
        let long = "前缀".repeat(80);
        let markdown = format!("{long} [[目标笔记]] 后缀");
        let matches = parse_wikilinks(&markdown);
        assert_eq!(matches.len(), 1);
        assert!(matches[0].context.contains("[[目标笔记]]"));
        assert!(matches[0].context.chars().count() <= LINK_CONTEXT_CHARS + 2);
        assert!(matches[0].context.starts_with('…'));
    }

    #[test]
    fn parses_markdown_links_and_flags_external() {
        let matches = parse_markdown_links(
            "内部 [设计稿](01_技术/设计稿.md)，外部 [官网](https://example.com)，锚点 [跳转](#小节)，代码 `[不该解析](a.md)`。",
        );
        assert_eq!(matches.len(), 2, "{matches:?}");
        assert_eq!(matches[0].target, "01_技术/设计稿.md");
        assert_eq!(matches[0].text.as_deref(), Some("设计稿"));
        assert!(!matches[0].is_external);
        assert_eq!(matches[1].target, "https://example.com");
        assert!(matches[1].is_external);
    }

    #[test]
    fn rewrites_wikilink_targets_keeping_alias_and_anchor() {
        let body = "见 [[旧标题]]、[[旧标题|别名]]、[[旧标题#小节]]，以及 [[别的笔记]] 与 `[[旧标题]]`。";
        let (updated, count) = rewrite_wikilink_target(body, "旧标题", "新标题");
        assert_eq!(count, 3, "{updated}");
        assert!(updated.contains("[[新标题]]"));
        assert!(updated.contains("[[新标题|别名]]"));
        assert!(updated.contains("[[新标题#小节]]"));
        assert!(updated.contains("[[别的笔记]]"), "其它链接不受影响");
        assert!(updated.contains("`[[旧标题]]`"), "行内代码内的内容不改写");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_removes_illegal_characters() {
        assert_eq!(sanitize_file_name("a/b:c*d?e"), "a-b-c-d-e");
        assert_eq!(sanitize_file_name("  前后空格  "), "前后空格");
        assert_eq!(sanitize_file_name("CON"), "note-CON");
        assert_eq!(sanitize_file_name(""), "note");
        assert_eq!(sanitize_file_name("..."), "note");
    }

    #[test]
    fn sanitize_limits_length() {
        let long = "字".repeat(400);
        let cleaned = sanitize_file_name(&long);
        assert!(cleaned.chars().count() <= MAX_FILE_STEM_CHARS);
    }

    #[test]
    fn note_file_name_uses_documented_style() {
        let name = note_file_name("SQLite 全文搜索", "20260923-120000");
        assert_eq!(name, "20260923-120000-SQLite-全文搜索.md");
        assert!(note_file_name("", "20260923-120000").ends_with("untitled.md"));
    }

    #[test]
    fn markdown_plain_text_strips_syntax_and_frontmatter() {
        let markdown = "---\nid: note_1\ntitle: 标题\n---\n\n# 大标题\n\n正文 **加粗** 与 `代码`。\n\n- 列表项\n\n> 引用\n\n![图](a.png)\n\n[链接](https://example.com)\n\n```rust\nlet a = 1;\n```\n";
        let plain = markdown_to_plain_text(markdown);
        assert!(!plain.contains("id: note_1"), "frontmatter 应被去掉：{plain}");
        assert!(!plain.contains('#'), "标题标记应被去掉：{plain}");
        assert!(plain.contains("大标题"));
        assert!(plain.contains("正文 加粗 与 代码。"));
        assert!(plain.contains("列表项"));
        assert!(plain.contains("引用"));
        assert!(plain.contains("图"));
        assert!(plain.contains("链接"));
        assert!(plain.contains("let a = 1;"), "代码内容应保留：{plain}");
        assert!(!plain.contains("```"));
    }

    #[test]
    fn count_words_handles_cjk_and_latin() {
        assert_eq!(count_words("中文四个字"), 5);
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("中文 hello 世界"), 4 + 1);
    }

    #[test]
    fn fts_match_handles_mixed_cjk_latin_as_single_prefix() {
        // 混合写法：整段前缀（索引侧是一个 token）
        assert_eq!(
            build_fts_match("术语A7F3").as_deref(),
            Some("\"术语a7f3\"*")
        );
        assert_eq!(
            build_fts_match("SQLite索引").as_deref(),
            Some("\"sqlite索引\"*")
        );
        // 纯中文仍是双字短语，纯英文仍是前缀
        assert_eq!(build_fts_match("查询优化").as_deref(), Some("\"查询 询优 优化\""));
        assert_eq!(build_fts_match("sqlite").as_deref(), Some("sqlite*"));
        // 带空格的混合查询：两段分别处理（拉丁前缀 + 中文双字）
        assert_eq!(
            build_fts_match("索引 A7F3").as_deref(),
            Some("\"索引\" AND A7F3*")
        );
    }

    #[test]
    fn cjk_tokens_include_unigrams_and_bigrams() {
        let tokens = cjk_search_tokens("全文搜索");
        for expected in ["全", "文", "搜", "索", "全文", "文搜", "搜索"] {
            assert!(tokens.split(' ').any(|token| token == expected), "缺少 {expected}：{tokens}");
        }
        assert!(!tokens.contains("全文搜"), "不应生成三字词");
    }

    #[test]
    fn fts_match_builds_safe_expressions() {
        let expression = build_fts_match("SQLite 全文搜索").unwrap();
        assert!(expression.contains("SQLite*"), "{expression}");
        assert!(expression.contains("\"全文 文搜 搜索\""), "{expression}");
        assert!(expression.contains(" AND "), "{expression}");

        // 语法字符必须被清理，避免注入 FTS 语法
        let dangerous = build_fts_match("\" OR (x").unwrap();
        assert!(!dangerous.contains('"'), "{dangerous}");
        assert!(!dangerous.contains('('), "{dangerous}");

        assert!(build_fts_match("   ").is_none());
    }

    #[test]
    fn fts_match_supports_single_cjk_character() {
        let expression = build_fts_match("库").unwrap();
        assert_eq!(expression, "库");
    }

    #[test]
    fn snippet_marks_matched_segments() {
        let text = "前文内容。这里讨论 SQLite 全文搜索的实现方式，后面还有很多内容。";
        let segments = build_snippet(text, "全文搜索", 60);
        let matched: String = segments
            .iter()
            .filter(|segment| segment.matched)
            .map(|segment| segment.text.clone())
            .collect();
        assert_eq!(matched, "全文搜索");
        let joined: String = segments.iter().map(|segment| segment.text.clone()).collect();
        assert!(joined.contains("SQLite"));
    }

    #[test]
    fn snippet_without_match_returns_prefix() {
        let segments = build_snippet("只有一段普通文本", "不存在", 6);
        let joined: String = segments.iter().map(|segment| segment.text.clone()).collect();
        assert!(joined.contains("只有一段普通"));
        assert!(segments.iter().all(|segment| !segment.matched));
    }

    #[test]
    fn highlight_terms_are_deduplicated_and_ordered() {
        let terms = highlight_terms("search search 全文搜索");
        // 按字符数倒序排列，较长的 ASCII 词排在前面
        assert_eq!(terms[0], "search");
        assert!(terms.contains(&"全文搜索".to_string()));
        assert_eq!(terms.iter().filter(|term| term.as_str() == "search").count(), 1);
    }
}
