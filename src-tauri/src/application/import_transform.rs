//! 外部知识库导入的文本转换（V1.1.2）。
//!
//! 支持两类常见导出：
//! - **Obsidian**：`![[图片.png]]` 形式的嵌入、`[[双链]]` 已由链接解析器支持，附件放在库内；
//! - **Notion**：导出的目录/文件名带 ` <32 位十六进制>` 后缀，链接目标可能被 URL 编码。
//!
//! 这里只做纯文本转换，方便单测；文件复制与路径解析由 `ImportService` 负责。

use std::collections::HashMap;

/// 导入格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportMode {
    /// 通用 Markdown（不做任何改写）
    Plain,
    /// Obsidian 库：把 `![[文件]]` 嵌入改写为标准 Markdown 图片链接
    Obsidian,
    /// Notion 导出：去掉名称里的哈希后缀、解码链接目标
    Notion,
}

impl ImportMode {
    pub fn parse(value: Option<&str>) -> Self {
        match value.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
            "obsidian" => ImportMode::Obsidian,
            "notion" => ImportMode::Notion,
            "plain" | "markdown" => ImportMode::Plain,
            _ => ImportMode::Plain,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ImportMode::Plain => "plain",
            ImportMode::Obsidian => "obsidian",
            ImportMode::Notion => "notion",
        }
    }
}

/// 自动识别：目录里有 `.obsidian/` 视为 Obsidian 库；
/// 出现 `名称 <32 位十六进制>.md` 视为 Notion 导出。
pub fn detect_mode(hints: &[String]) -> ImportMode {
    let has_obsidian_dir = hints.iter().any(|hint| {
        let lower = hint.to_ascii_lowercase();
        lower == ".obsidian" || lower.ends_with("/.obsidian") || lower.contains(".obsidian/")
    });
    if has_obsidian_dir {
        return ImportMode::Obsidian;
    }
    let has_notion_suffix = hints.iter().any(|hint| looks_like_notion_name(hint));
    if has_notion_suffix {
        return ImportMode::Notion;
    }
    ImportMode::Plain
}

/// `页面 abcdef0123456789abcdef0123456789.md` → `页面.md`
pub fn strip_notion_suffix(file_name: &str) -> String {
    let (stem, extension) = match file_name.rsplit_once('.') {
        Some((stem, extension)) if !extension.is_empty() && extension.len() <= 8 => {
            (stem, Some(extension))
        }
        _ => (file_name, None),
    };
    let trimmed = match stem.rsplit_once(' ') {
        Some((head, tail)) if is_hex32(tail) => head.trim_end(),
        _ => stem,
    };
    match extension {
        Some(extension) => format!("{trimmed}.{extension}"),
        None => trimmed.to_string(),
    }
}

fn looks_like_notion_name(value: &str) -> bool {
    let name = value.rsplit('/').next().unwrap_or(value);
    let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name);
    stem.rsplit_once(' ').is_some_and(|(_, tail)| is_hex32(tail))
}

fn is_hex32(value: &str) -> bool {
    value.len() == 32 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

/// 解码链接目标里的 URL 编码（`%20` → 空格）并去掉首尾的 `<` `>`。
pub fn decode_link_target(target: &str) -> String {
    let trimmed = target.trim().trim_start_matches('<').trim_end_matches('>');
    let bytes = trimmed.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            // % 转义一定是 ASCII，切片安全；其余字节原样保留（不能按字节转 char，
            // 否则中日韩字符会被拆成乱码）
            if let Ok(value) = u8::from_str_radix(&trimmed[index + 1..index + 3], 16) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 把 Obsidian 嵌入改写为标准 Markdown 图片链接。
///
/// `resolver(嵌入目标)` 返回「相对当前笔记目录」的附件路径；返回 `None` 表示附件没被导入，
/// 此时保留原文，避免丢信息。`[[双链]]`（非嵌入）不做处理，交由链接解析器。
pub fn rewrite_obsidian_embeds(
    body: &str,
    resolver: &dyn Fn(&str) -> Option<String>,
) -> String {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len());
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == '!' && index + 1 < chars.len() && chars[index + 1] == '[' {

            if let Some(end) = find_embed_end(&chars, index + 3) {
                let inner: String = chars[index + 3..end].iter().collect();
                let (target, alias) = match inner.split_once('|') {
                    Some((target, alias)) => (target.trim(), Some(alias.trim())),
                    None => (inner.trim(), None),
                };
                if let Some(link) = resolver(target) {
                    let alt = alias.filter(|value| !value.is_empty()).unwrap_or(target);
                    out.push_str(&format!("![{}]({})", alt, link));
                } else {
                    out.push_str(&chars[index..end + 2].iter().collect::<String>());
                }
                index = end + 2;
                continue;
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    out
}

fn find_embed_end(chars: &[char], start: usize) -> Option<usize> {
    let mut index = start;
    while index < chars.len() {
        if chars[index] == '\n' {
            return None;
        }
        if chars[index] == ']' && chars.get(index + 1) == Some(&']') {
            return Some(index);
        }
        if chars[index] == '[' && chars.get(index + 1) == Some(&'[') {
            return None;
        }
        index += 1;
    }
    None
}

/// Notion：把正文里的相对链接目标做 URL 解码（`%20` → 空格）。
pub fn rewrite_notion_links(body: &str) -> String {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len());
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == ']' && chars.get(index + 1) == Some(&'(') {
            if let Some(end) = chars[index + 2..].iter().position(|ch| *ch == ')') {
                let end = index + 2 + end;
                let target: String = chars[index + 2..end].iter().collect();
                out.push_str("](");
                out.push_str(&decode_link_target(&target));
                out.push(')');
                index = end + 1;
                continue;
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    out
}

/// 附件映射：用「文件名」与「相对路径」双重索引，便于两种写法都能命中。
pub type AttachmentMap = HashMap<String, String>;

pub fn lookup_attachment(map: &AttachmentMap, target: &str) -> Option<String> {
    let normalized = target.trim().trim_start_matches("./").replace('\\', "/");
    let decoded = decode_link_target(&normalized);
    if let Some(hit) = map.get(&decoded) {
        return Some(hit.clone());
    }
    let file_name = decoded.rsplit('/').next().unwrap_or(&decoded);
    map.get(file_name).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_mode_from_hints() {
        assert_eq!(
            detect_mode(&["我的库/.obsidian".to_string()]),
            ImportMode::Obsidian
        );
        assert_eq!(
            detect_mode(&["页面 abcdef0123456789abcdef0123456789.md".to_string()]),
            ImportMode::Notion
        );
        assert_eq!(detect_mode(&["普通笔记.md".to_string()]), ImportMode::Plain);
        assert_eq!(ImportMode::parse(Some("OBSIDIAN")), ImportMode::Obsidian);
        assert_eq!(ImportMode::parse(None), ImportMode::Plain);
    }

    #[test]
    fn strips_notion_hash_suffix_only_when_present() {
        assert_eq!(
            strip_notion_suffix("会议记录 abcdef0123456789abcdef0123456789.md"),
            "会议记录.md"
        );
        assert_eq!(
            strip_notion_suffix("会议记录 abcdef0123456789abcdef0123456789"),
            "会议记录"
        );
        // 不是 32 位十六进制时保持原样
        assert_eq!(strip_notion_suffix("会议记录 abc123.md"), "会议记录 abc123.md");
        assert_eq!(strip_notion_suffix("普通笔记.md"), "普通笔记.md");
    }

    #[test]
    fn decodes_url_encoded_targets() {
        assert_eq!(decode_link_target("图片%20一.png"), "图片 一.png");
        assert_eq!(decode_link_target("<a%2Fb.png>"), "a/b.png");
        assert_eq!(decode_link_target("原样.png"), "原样.png");
    }

    #[test]
    fn rewrites_obsidian_embeds_and_keeps_unknown() {
        let mut map = AttachmentMap::new();
        map.insert("图片 一.png".to_string(), "../attachments/2026-10/图片 一.png".to_string());
        let resolver = |target: &str| lookup_attachment(&map, target);

        let body = "正文\n\n![[图片 一.png]]\n![[图片 一.png|示意图]]\n![[缺失.png]]\n[[双链笔记]]\n";
        let output = rewrite_obsidian_embeds(body, &resolver);
        println!("[debug-output] {output:?}");
        assert!(output.contains("![图片 一.png](../attachments/2026-10/图片 一.png)"));
        assert!(output.contains("![示意图](../attachments/2026-10/图片 一.png)"));
        assert!(output.contains("![[缺失.png]]"), "未导入的附件保留原文");
        assert!(output.contains("[[双链笔记]]"), "双链不受影响");
    }

    #[test]
    fn rewrites_notion_relative_links() {
        let body = "见 [设计稿](设计稿%20v2.png) 与 [其它](其它.md)";
        let output = rewrite_notion_links(body);
        assert!(output.contains("[设计稿](设计稿 v2.png)"));
        assert!(output.contains("[其它](其它.md)"));
    }
}
