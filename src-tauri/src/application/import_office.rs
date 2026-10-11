//! Office 文档导入：`.docx`（Word）与 `.csv` → Markdown（V1.2.1 P2）。
//!
//! 对标语雀支持的 Word / Excel 导入，但只做**纯文本骨架**：
//! - `.docx`：解压 `word/document.xml`，按段落抽取文本，
//!   依据段落样式（`Heading1`~`Heading6`）映射成 Markdown 标题，列表项映射成 `- `；
//!   图片、表格、批注、页眉页脚不导入（保持零新增依赖与可预期的结果）；
//! - `.csv`：按 RFC 4180（引号转义）解析，生成 Markdown 表格；
//!   行数超过 `MAX_CSV_ROWS` 时截断并在文末标注，避免一次导入把界面拖死。
//!
//! 依赖只有已有的 `zip` 与本地缓存里的 `quick-xml`，不引入 calamine / docx-rs 之类的大包。

use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::core::error::{AppError, ErrorCode, Result};

/// CSV 单次导入的最大数据行数（不含表头）。
const MAX_CSV_ROWS: usize = 5000;
/// docx 段落数上限，避免超大文档把界面拖住。
const MAX_DOCX_PARAGRAPHS: usize = 20000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeFormat {
    Docx,
    Csv,
}

impl OfficeFormat {
    /// 按扩展名识别；不支持的返回 `None`。
    pub fn detect(path: &Path) -> Option<Self> {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("docx") => Some(Self::Docx),
            Some("csv") => Some(Self::Csv),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Csv => "csv",
        }
    }
}

/// 把 Office 文档转成 Markdown 正文；`path` 用于报错信息。
pub fn to_markdown(path: &Path, format: OfficeFormat) -> Result<String> {
    match format {
        OfficeFormat::Docx => docx_to_markdown(path),
        OfficeFormat::Csv => {
            let bytes = std::fs::read(path).map_err(|error| {
                AppError::new(ErrorCode::IoError, format!("读取 CSV 失败：{error}"))
            })?;
            csv_to_markdown(&crate::core::encoding::decode_text(&bytes).0)
        }
    }
}


/// `.docx` → Markdown：解压 `word/document.xml` 抽取段落。
fn docx_to_markdown(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path)
        .map_err(|error| AppError::new(ErrorCode::IoError, format!("打开 docx 失败：{error}")))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| {
        AppError::new(ErrorCode::InvalidInput, format!("不是有效的 docx（zip 解析失败）：{error}"))
    })?;
    let mut bytes = Vec::new();
    {
        let mut document = archive.by_name("word/document.xml").map_err(|_| {
            AppError::new(ErrorCode::InvalidInput, "docx 里没有 word/document.xml，可能是旧版 .doc 改名")
        })?;
        document.read_to_end(&mut bytes).map_err(|error| {
            AppError::new(ErrorCode::IoError, format!("读取 docx 正文失败：{error}"))
        })?;
    }
    extract_docx_paragraphs(&crate::core::encoding::decode_text(&bytes).0)
}

/// 从 `word/document.xml` 里按段落抽取 Markdown。
fn extract_docx_paragraphs(xml: &str) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut lines: Vec<String> = Vec::new();
    let mut paragraph = String::new();
    let mut heading_level: Option<usize> = None;
    let mut in_paragraph = false;
    let mut is_list_item = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => match event.name().into_inner() {
                "w:p" => {
                    in_paragraph = true;
                    paragraph.clear();
                    heading_level = None;
                    is_list_item = false;
                }
                "w:pStyle" => {
                    let style = attribute_value(&event, "w:val").unwrap_or_default();
                    heading_level = heading_level_of(&style);
                    if style.to_ascii_lowercase().contains("list") {
                        is_list_item = true;
                    }
                }
                "w:numPr" => is_list_item = true,
                "w:br" => paragraph.push('\n'),
                "w:tab" => paragraph.push('\t'),
                _ => {}
            },
            // 自闭合标签（<w:pStyle .../>、<w:numPr/>）走 Empty，不是 Start
            Ok(Event::Empty(event)) => match event.name().into_inner() {
                "w:pStyle" => {
                    let style = attribute_value(&event, "w:val").unwrap_or_default();
                    heading_level = heading_level_of(&style);
                    if style.to_ascii_lowercase().contains("list") {
                        is_list_item = true;
                    }
                }
                "w:numPr" => is_list_item = true,
                "w:br" => paragraph.push('\n'),
                "w:tab" => paragraph.push('\t'),
                _ => {}
            },
            Ok(Event::Text(event)) => {
                let raw = event.into_inner();
                // 实体（&amp; 等）需要显式反转义；失败时保留原文，不丢内容
                let text = quick_xml::escape::unescape(&raw)
                    .map(|value| value.into_owned())
                    .unwrap_or_else(|_| raw.into_owned());
                paragraph.push_str(&text);
            }
            // `&amp;` / `&#65;` 会作为独立事件出现，必须显式还原，否则会丢字符
            Ok(Event::GeneralRef(event)) => {
                let name = event.into_inner().into_owned();
                if let Some(resolved) = quick_xml::escape::resolve_predefined_entity(&name) {
                    paragraph.push_str(resolved);
                } else if let Some(ch) = resolve_numeric_ref(&name) {
                    paragraph.push(ch);
                } else {
                    paragraph.push('&');
                    paragraph.push_str(&name);
                    paragraph.push(';');
                }
            }
            Ok(Event::End(event)) => {
                if event.name().into_inner() == "w:p" && in_paragraph {
                    let content = paragraph.trim().to_string();
                    if !content.is_empty() {
                        let rendered = match heading_level {
                            Some(level) => format!("{} {}", "#".repeat(level), content),
                            None if is_list_item => format!("- {content}"),
                            None => content,
                        };
                        lines.push(rendered);
                        if lines.len() >= MAX_DOCX_PARAGRAPHS {
                            lines.push(format!(
                                "\n> 文档段落数超过 {MAX_DOCX_PARAGRAPHS}，其余内容已省略。"
                            ));
                            break;
                        }
                    }
                    in_paragraph = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(AppError::new(
                    ErrorCode::InvalidInput,
                    format!("docx 正文 XML 解析失败：{error}"),
                ))
            }
            _ => {}
        }
    }

    if lines.is_empty() {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "docx 里没有可导入的文字（可能是纯图片文档）",
        ));
    }
    Ok(lines.join("\n\n"))
}

/// `w:pStyle w:val="Heading2"` / `标题 2` → 2。
fn heading_level_of(style: &str) -> Option<usize> {
    let normalized = style.trim();
    if normalized.is_empty() {
        return None;
    }
    let lower = normalized.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("heading") {
        return rest.trim().parse::<usize>().ok().filter(|level| (1..=6).contains(level));
    }
    // 中文 Word 的样式名：标题 1 / 标题1
    if let Some(rest) = normalized.strip_prefix("标题") {
        return rest.trim().parse::<usize>().ok().filter(|level| (1..=6).contains(level));
    }
    None
}

fn attribute_value(event: &quick_xml::events::BytesStart<'_>, key: &str) -> Option<String> {
    event
        .attributes()
        .flatten()
        .find(|attribute| attribute.key.into_inner() == key)
        .map(|attribute| attribute.value.to_string())
}

/// 还原 `&#65;` / `&#x1F600;` 形式的数字实体。
fn resolve_numeric_ref(name: &str) -> Option<char> {
    let digits = name.strip_prefix('#')?;
    let code = if let Some(hex) = digits.strip_prefix(['x', 'X']) {
        u32::from_str_radix(hex, 16).ok()?
    } else {
        digits.parse::<u32>().ok()?
    };
    char::from_u32(code)
}

/// `.csv` → Markdown 表格。
pub fn csv_to_markdown(text: &str) -> Result<String> {
    let mut rows = parse_csv(text);
    if rows.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidInput, "CSV 是空的"));
    }
    let truncated = rows.len() > MAX_CSV_ROWS + 1;
    if truncated {
        rows.truncate(MAX_CSV_ROWS + 1);
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);

    let mut out = String::new();
    for (index, row) in rows.iter().enumerate() {
        let mut cells: Vec<String> = row.iter().map(|cell| escape_cell(cell)).collect();
        cells.resize(columns, String::new());
        out.push_str("| ");
        out.push_str(&cells.join(" | "));
        out.push_str(" |\n");
        if index == 0 {
            out.push('|');
            for _ in 0..columns {
                out.push_str(" --- |");
            }
            out.push('\n');
        }
    }
    if truncated {
        out.push_str(&format!(
            "\n> CSV 行数超过 {MAX_CSV_ROWS}，其余内容已省略；可用电子表格分片后再导入。\n"
        ));
    }
    Ok(out.trim_end().to_string())
}

/// 表格单元里的 `|` 与换行会破坏 Markdown 表格，需要转义 / 替换。
fn escape_cell(cell: &str) -> String {
    cell.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\r', '\n'], "<br>")
        .trim()
        .to_string()
}

/// RFC 4180 解析：支持引号包裹、字段内逗号 / 换行、`""` 转义。
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(ch);
            }
            continue;
        }
        match ch {
            '"' => in_quotes = true,
            ',' => {
                row.push(std::mem::take(&mut field));
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            _ => field.push(ch),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    // 丢掉完全空白的行（Excel 导出的尾部空行）
    rows.retain(|cells| cells.iter().any(|cell| !cell.trim().is_empty()));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_csv_fields() {
        let rows = parse_csv("a,b\n\"x,1\",\"he said \"\"hi\"\"\"\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][0], "x,1");
        assert_eq!(rows[1][1], "he said \"hi\"");
    }

    #[test]
    fn csv_becomes_markdown_table_with_escaped_pipes() {
        let table = csv_to_markdown("名称,数量\nA|B,2\n").expect("转换");
        assert!(table.contains("| 名称 | 数量 |"));
        assert!(table.contains("| --- | --- |"));
        assert!(table.contains("A\\|B"), "竖线要转义：{table}");
    }

    #[test]
    fn heading_styles_map_to_markdown_levels() {
        assert_eq!(heading_level_of("Heading1"), Some(1));
        assert_eq!(heading_level_of("heading 3"), Some(3));
        assert_eq!(heading_level_of("标题2"), Some(2));
        assert_eq!(heading_level_of("Heading9"), None);
        assert_eq!(heading_level_of("Normal"), None);
    }

    #[test]
    fn extracts_docx_paragraphs_from_document_xml() {
        let xml = r#"<?xml version="1.0"?>
<w:document xmlns:w="x"><w:body>
  <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>标题一</w:t></w:r></w:p>
  <w:p><w:r><w:t>第一段</w:t></w:r><w:r><w:t>续写</w:t></w:r></w:p>
  <w:p><w:pPr><w:numPr/></w:pPr><w:r><w:t>列表项</w:t></w:r></w:p>
  <w:p/>
</w:body></w:document>"#;
        let markdown = extract_docx_paragraphs(xml).expect("解析");
        assert!(markdown.contains("# 标题一"), "{markdown}");
        assert!(markdown.contains("第一段续写"), "同段多个 run 要拼接：{markdown}");
        assert!(markdown.contains("- 列表项"), "{markdown}");
    }

    #[test]
    fn rejects_documents_without_text() {
        let xml = r#"<w:document xmlns:w="x"><w:body><w:p/></w:body></w:document>"#;
        assert!(extract_docx_paragraphs(xml).is_err());
    }
}
