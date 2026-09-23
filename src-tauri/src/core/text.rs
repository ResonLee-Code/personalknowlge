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
    let segments = split_query_segments(&trimmed);
    let mut clauses: Vec<String> = Vec::new();

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
