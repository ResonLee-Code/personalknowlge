//! Markdown → Word (.docx) 生成。
//!
//! 做法：先用 `pulldown-cmark` 把 Markdown 解析成块级结构，再序列化为 OOXML，
//! 最后按 OOXML 包规范打包成 ZIP。不依赖外部工具，中文直接以 UTF-8 写入。
//!
//! 支持的语法：标题（1–3 级）、段落、粗体/斜体/删除线、行内代码、代码块、
//! 引用、有序/无序列表、表格、分割线、链接（保留文字并在其后附上 URL）。
//! 已知取舍：图片以「[图片]」占位（不内嵌二进制），嵌套列表压平为缩进。

use std::io::Write;

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::core::error::{AppError, ErrorCode, Result};

#[derive(Debug, Clone, Default)]
struct Run {
    text: String,
    bold: bool,
    italic: bool,
    strike: bool,
    code: bool,
    link: Option<String>,
    break_before: bool,
}

#[derive(Debug, Clone)]
enum Block {
    Heading { level: u8, runs: Vec<Run> },
    Paragraph { runs: Vec<Run>, indent: u8, marker: Option<String> },
    Quote { runs: Vec<Run> },
    Code { lines: Vec<String> },
    Table { rows: Vec<Vec<String>>, has_header: bool },
    Rule,
}

/// 生成 .docx 文件内容。
pub fn build_docx(
    title: &str,
    markdown: &str,
    category_path: &str,
    tags: &[String],
    created_at: &str,
    updated_at: &str,
) -> Result<Vec<u8>> {
    let blocks = parse_blocks(markdown);
    let document = render_document(title, category_path, tags, &blocks);

    let mut buffer = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buffer);
        let mut zip = ZipWriter::new(cursor);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        let write_entry = |zip: &mut ZipWriter<_>, name: &str, content: &str| -> Result<()> {
            zip.start_file(name, options)
                .map_err(|error| AppError::new(ErrorCode::ExportFailed, error.to_string()))?;
            zip.write_all(content.as_bytes())?;
            Ok(())
        };

        write_entry(&mut zip, "[Content_Types].xml", CONTENT_TYPES)?;
        write_entry(&mut zip, "_rels/.rels", RELS)?;
        write_entry(&mut zip, "word/document.xml", &document)?;
        write_entry(&mut zip, "word/styles.xml", STYLES)?;
        write_entry(&mut zip, "word/_rels/document.xml.rels", DOCUMENT_RELS)?;
        write_entry(&mut zip, "docProps/core.xml", &core_properties(title, created_at, updated_at))?;
        write_entry(&mut zip, "docProps/app.xml", APP_PROPERTIES)?;

        zip.finish()
            .map_err(|error| AppError::new(ErrorCode::ExportFailed, error.to_string()))?;
    }
    Ok(buffer)
}

// ---------------------------------------------------------------- 解析

fn parse_blocks(markdown: &str) -> Vec<Block> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(markdown, options);
    let mut blocks: Vec<Block> = Vec::new();

    let mut runs: Vec<Run> = Vec::new();
    let mut bold = 0usize;
    let mut italic = 0usize;
    let mut strike = 0usize;
    let mut in_code_block = false;
    let mut code_lines: Vec<String> = Vec::new();
    let mut code_buffer = String::new();
    let mut heading_level: Option<u8> = None;
    let mut quote_depth = 0usize;
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut item_marker: Option<String> = None;
    let mut pending_break = false;
    let mut in_image = false;

    let mut table_rows: Vec<Vec<String>> = Vec::new();
    let mut table_row: Vec<String> = Vec::new();
    let mut table_cell = String::new();
    let mut in_table = false;
    let mut has_header = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading_level = Some(heading_level_to_u8(level));
                runs.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_runs(
                    &mut runs,
                    &mut blocks,
                    heading_level,
                    quote_depth,
                    lists.len(),
                    &item_marker,
                );
                heading_level = None;
            }
            Event::Start(Tag::Paragraph) => runs.clear(),
            Event::End(TagEnd::Paragraph) => flush_runs(
                &mut runs,
                &mut blocks,
                heading_level,
                quote_depth,
                lists.len(),
                &item_marker,
            ),
            Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Strong) => bold = bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => italic += 1,
            Event::End(TagEnd::Emphasis) => italic = italic.saturating_sub(1),
            Event::Start(Tag::Strikethrough) => strike += 1,
            Event::End(TagEnd::Strikethrough) => strike = strike.saturating_sub(1),
            Event::Start(Tag::BlockQuote(_)) => {
                quote_depth += 1;
                runs.clear();
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush_runs(
                    &mut runs,
                    &mut blocks,
                    heading_level,
                    quote_depth,
                    lists.len(),
                    &item_marker,
                );
                quote_depth = quote_depth.saturating_sub(1);
            }
            Event::Start(Tag::List(start)) => lists.push(start),
            Event::End(TagEnd::List(_)) => {
                lists.pop();
            }
            Event::Start(Tag::Item) => {
                let marker = match lists.last() {
                    Some(Some(number)) => format!("{number}. "),
                    _ => "• ".to_string(),
                };
                item_marker = Some(marker);
                runs.clear();
            }
            Event::End(TagEnd::Item) => {
                flush_runs(
                    &mut runs,
                    &mut blocks,
                    heading_level,
                    quote_depth,
                    lists.len(),
                    &item_marker,
                );
                item_marker = None;
            }
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                code_buffer.clear();
                code_lines.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                if !code_buffer.is_empty() {
                    code_lines.push(std::mem::take(&mut code_buffer));
                }
                blocks.push(Block::Code { lines: std::mem::take(&mut code_lines) });
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                // 链接 URL 记录到后续 run 上
                runs.push(Run { link: Some(dest_url.to_string()), ..Default::default() });
            }
            Event::End(TagEnd::Link) => {}
            Event::Start(Tag::Image { .. }) => in_image = true,
            Event::End(TagEnd::Image) => {
                in_image = false;
                runs.push(Run { text: "[图片]".into(), italic: true, ..Default::default() });
            }
            Event::Start(Tag::Table(_)) => {
                in_table = true;
                table_rows.clear();
                has_header = false;
            }
            Event::End(TagEnd::Table) => {
                in_table = false;
                blocks.push(Block::Table {
                    rows: std::mem::take(&mut table_rows),
                    has_header,
                });
            }
            Event::Start(Tag::TableHead) => has_header = true,
            Event::End(TagEnd::TableHead) => {
                // pulldown-cmark 的表头不经过 TableRow，这里直接收行
                if !table_row.is_empty() {
                    table_rows.push(std::mem::take(&mut table_row));
                }
            }
            Event::Start(Tag::TableRow) => table_row.clear(),
            Event::End(TagEnd::TableRow) => table_rows.push(std::mem::take(&mut table_row)),
            Event::Start(Tag::TableCell) => table_cell.clear(),
            Event::End(TagEnd::TableCell) => table_row.push(std::mem::take(&mut table_cell)),
            Event::Text(text) => {
                if in_table {
                    table_cell.push_str(&text);
                } else if in_code_block {
                    code_buffer.push_str(&text);
                } else if in_image {
                    // 忽略图片的 alt 文本，统一输出占位
                } else {
                    push_run(&mut runs, &text, bold, italic, strike, false, pending_break);
                    pending_break = false;
                }
            }
            // 行内/块级 HTML 不解析，按纯文本保留，避免内容丢失
            Event::Html(text) | Event::InlineHtml(text) => {
                if in_table {
                    table_cell.push_str(&text);
                } else if in_code_block {
                    code_buffer.push_str(&text);
                } else {
                    push_run(&mut runs, &text, bold, italic, strike, false, pending_break);
                    pending_break = false;
                }
            }
            Event::Code(text) => {
                if in_table {
                    table_cell.push_str(&text);
                } else {
                    push_run(&mut runs, &text, bold, italic, strike, true, pending_break);
                    pending_break = false;
                }
            }
            Event::SoftBreak => {
                if in_code_block {
                    code_buffer.push('\n');
                } else {
                    pending_break = false;
                    if !runs.is_empty() {
                        pending_break_after_space(&mut runs);
                    }
                }
            }
            Event::HardBreak => {
                if in_code_block {
                    code_lines.push(std::mem::take(&mut code_buffer));
                } else {
                    pending_break = true;
                }
            }
            Event::Rule => blocks.push(Block::Rule),
            _ => {}
        }
    }

    if !code_lines.is_empty() || !code_buffer.is_empty() {
        if !code_buffer.is_empty() {
            code_lines.push(code_buffer);
        }
        blocks.push(Block::Code { lines: code_lines });
    }
    flush_runs(&mut runs, &mut blocks, None, 0, 0, &None);
    blocks
}

/// 把当前累积的行内内容收尾成一个块。
#[allow(clippy::too_many_arguments)]
fn flush_runs(
    runs: &mut Vec<Run>,
    blocks: &mut Vec<Block>,
    heading_level: Option<u8>,
    quote_depth: usize,
    list_depth: usize,
    item_marker: &Option<String>,
) {
    if runs.is_empty() {
        return;
    }
    let collected = std::mem::take(runs);
    if let Some(level) = heading_level {
        blocks.push(Block::Heading { level, runs: collected });
    } else if quote_depth > 0 {
        blocks.push(Block::Quote { runs: collected });
    } else {
        blocks.push(Block::Paragraph {
            runs: collected,
            indent: list_depth.saturating_sub(1) as u8,
            marker: item_marker.clone(),
        });
    }
}

fn heading_level_to_u8(level: pulldown_cmark::HeadingLevel) -> u8 {
    use pulldown_cmark::HeadingLevel::*;
    match level {
        H1 => 1,
        H2 => 2,
        H3 => 3,
        H4 => 4,
        H5 => 5,
        H6 => 6,
    }
}

fn push_run(
    runs: &mut Vec<Run>,
    text: &str,
    bold: usize,
    italic: usize,
    strike: usize,
    code: bool,
    break_before: bool,
) {
    if text.is_empty() {
        return;
    }
    // 链接 URL 已作为占位 run 入队，这里把样式合并到同一个 run 上
    if let Some(last) = runs.last_mut() {
        if last.text.is_empty() && last.link.is_some() {
            last.text = text.to_string();
            last.bold = bold > 0;
            last.italic = italic > 0;
            last.strike = strike > 0;
            last.code = code;
            last.break_before = break_before;
            return;
        }
    }
    runs.push(Run {
        text: text.to_string(),
        bold: bold > 0,
        italic: italic > 0,
        strike: strike > 0,
        code,
        link: None,
        break_before,
    });
}

/// SoftBreak 在 Word 中等价于空格
fn pending_break_after_space(runs: &mut [Run]) {
    if let Some(last) = runs.last_mut() {
        if !last.text.ends_with(' ') {
            last.text.push(' ');
        }
    }
}

// ---------------------------------------------------------------- 序列化

fn render_document(title: &str, category_path: &str, tags: &[String], blocks: &[Block]) -> String {
    let mut body = String::new();
    body.push_str(&paragraph(&[Run {
        text: title.to_string(),
        bold: true,
        ..Default::default()
    }], Some("Title"), None));

    let mut meta_parts: Vec<String> = Vec::new();
    if !category_path.is_empty() {
        meta_parts.push(format!("分类：{category_path}"));
    }
    if !tags.is_empty() {
        meta_parts.push(format!("标签：{}", tags.join("、")));
    }
    if !meta_parts.is_empty() {
        body.push_str(&paragraph(
            &[Run { text: meta_parts.join("　"), italic: true, ..Default::default() }],
            Some("Meta"),
            None,
        ));
    }
    body.push_str(&paragraph(
        &[Run {
            text: "由 Personal Knowledge OS 导出".to_string(),
            italic: true,
            ..Default::default()
        }],
        Some("Meta"),
        None,
    ));

    for block in blocks {
        match block {
            Block::Heading { level, runs } => {
                let style = format!("Heading{}", (*level).clamp(1, 6));
                body.push_str(&paragraph(runs, Some(&style), None));
            }
            Block::Paragraph { runs, indent, marker } => {
                let mut all = Vec::new();
                if let Some(marker) = marker {
                    all.push(Run { text: marker.clone(), ..Default::default() });
                }
                all.extend(runs.iter().cloned());
                body.push_str(&paragraph(&all, None, (*indent > 0).then_some(*indent)));
            }
            Block::Quote { runs } => body.push_str(&paragraph(runs, Some("Quote"), Some(1))),
            Block::Code { lines } => {
                for line in lines {
                    body.push_str(&paragraph(
                        &[Run { text: line.clone(), code: true, ..Default::default() }],
                        Some("Code"),
                        None,
                    ));
                }
            }
            Block::Table { rows, has_header } => body.push_str(&render_table(rows, *has_header)),
            Block::Rule => body.push_str(
                r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="1" w:color="CCCCCC"/></w:pBdr></w:pPr></w:p>"#,
            ),
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body>
</w:document>"#
    )
}

fn paragraph(runs: &[Run], style: Option<&str>, indent: Option<u8>) -> String {
    let mut properties = String::new();
    if let Some(style) = style {
        properties.push_str(&format!(r#"<w:pStyle w:val="{style}"/>"#));
    }
    if let Some(indent) = indent {
        let left = 720 * (indent as i32 + 1);
        properties.push_str(&format!(
            r#"<w:ind w:left="{left}" w:hanging="360"/>"#
        ));
    }
    let properties = if properties.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{properties}</w:pPr>")
    };
    let runs: String = runs.iter().map(render_run).collect();
    format!("<w:p>{properties}{runs}</w:p>")
}

fn render_run(run: &Run) -> String {
    let mut properties = String::new();
    if run.code {
        properties.push_str(
            r#"<w:rFonts w:ascii="Consolas" w:hAnsi="Consolas" w:eastAsia="Consolas"/><w:shd w:val="clear" w:fill="F2F2F5"/>"#,
        );
    }
    if run.bold {
        properties.push_str("<w:b/>");
    }
    if run.italic {
        properties.push_str("<w:i/>");
    }
    if run.strike {
        properties.push_str("<w:strike/>");
    }
    if run.link.is_some() {
        properties.push_str(r#"<w:color w:val="4F46E5"/><w:u w:val="single"/>"#);
    }
    let properties = if properties.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{properties}</w:rPr>")
    };
    let br = if run.break_before { "<w:br/>" } else { "" };
    let mut text = escape_xml(&run.text);
    if let Some(link) = &run.link {
        if !run.text.trim().is_empty() && link.trim() != run.text.trim() {
            text.push_str(&escape_xml(&format!("（{link}）")));
        }
    }
    format!(
        r#"<w:r>{properties}{br}<w:t xml:space="preserve">{text}</w:t></w:r>"#
    )
}

fn render_table(rows: &[Vec<String>], has_header: bool) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let mut xml = String::from(
        r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/><w:tblBorders>"#,
    );
    for edge in ["top", "left", "bottom", "right", "insideH", "insideV"] {
        xml.push_str(&format!(
            r#"<w:{edge} w:val="single" w:sz="4" w:space="0" w:color="D7D7DE"/>"#
        ));
    }
    xml.push_str("</w:tblBorders></w:tblPr>");
    for (index, row) in rows.iter().enumerate() {
        xml.push_str("<w:tr>");
        for cell in row {
            let bold = has_header && index == 0;
            xml.push_str("<w:tc><w:tcPr><w:tcW w:w=\"0\" w:type=\"auto\"/></w:tcPr>");
            xml.push_str(&paragraph(
                &[Run { text: cell.clone(), bold, ..Default::default() }],
                None,
                None,
            ));
            xml.push_str("</w:tc>");
        }
        xml.push_str("</w:tr>");
    }
    xml.push_str("</w:tbl>");
    xml
}

fn escape_xml(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(ch),
            ch if (ch as u32) < 0x20 => {}
            ch => out.push(ch),
        }
    }
    out
}

fn core_properties(title: &str, created_at: &str, updated_at: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<dc:title>{title}</dc:title>
<dc:creator>Personal Knowledge OS</dc:creator>
<cp:lastModifiedBy>Personal Knowledge OS</cp:lastModifiedBy>
<dcterms:created xsi:type="dcterms:W3CDTF">{created}</dcterms:created>
<dcterms:modified xsi:type="dcterms:W3CDTF">{modified}</dcterms:modified>
</cp:coreProperties>"#,
        title = escape_xml(title),
        created = escape_xml(created_at),
        modified = escape_xml(updated_at),
    )
}

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>"#;

const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#;

const DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#;

const APP_PROPERTIES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
<Application>Personal Knowledge OS</Application>
</Properties>"#;

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:docDefaults><w:rPrDefault><w:rPr>
<w:rFonts w:ascii="Segoe UI" w:hAnsi="Segoe UI" w:eastAsia="Microsoft YaHei"/>
<w:sz w:val="22"/><w:szCs w:val="22"/>
</w:rPr></w:rPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:after="120"/></w:pPr><w:rPr><w:sz w:val="36"/><w:szCs w:val="36"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="360" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="280" w:after="100"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="240" w:after="80"/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="24"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Code"><w:name w:val="Code"/><w:basedOn w:val="Normal"/><w:pPr><w:shd w:val="clear" w:fill="F7F7F9"/><w:spacing w:after="0"/></w:pPr><w:rPr><w:rFonts w:ascii="Consolas" w:hAnsi="Consolas" w:eastAsia="Consolas"/><w:sz w:val="20"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:pPr><w:ind w:left="480"/></w:pPr><w:rPr><w:i/><w:color w:val="55565E"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Meta"><w:name w:val="Meta"/><w:basedOn w:val="Normal"/><w:rPr><w:i/><w:color w:val="8B8B96"/><w:sz w:val="18"/></w:rPr></w:style>
</w:styles>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn document_xml(bytes: &[u8]) -> String {
        let mut archive =
            zip::ZipArchive::new(std::io::Cursor::new(bytes.to_vec())).expect("docx 应为合法 zip");
        let mut file = archive.by_name("word/document.xml").expect("应包含 document.xml");
        let mut xml = String::new();
        file.read_to_string(&mut xml).expect("读取 document.xml");
        xml
    }

    #[test]
    fn builds_valid_docx_with_chinese_content() {
        let markdown = "# 一级标题\n\n这是一段**加粗**文字，包含 `代码`。\n\n- 列表项 A\n- 列表项 B\n\n```rust\nlet a = 1;\n```\n\n> 引用内容\n\n| 列1 | 列2 |\n| --- | --- |\n| a | b |\n";
        let bytes = build_docx("测试笔记", markdown, "技术/数据库", &["sqlite".into()], "2026-09-23T10:00:00+08:00", "2026-09-23T12:00:00+08:00")
            .expect("生成 docx");

        assert!(bytes.starts_with(b"PK\x03\x04"), "docx 应为 zip 格式");
        let xml = document_xml(&bytes);
        assert!(xml.contains("测试笔记"), "标题应写入文档");
        assert!(xml.contains("一级标题"));
        assert!(xml.contains("加粗"));
        assert!(xml.contains("<w:b/>"), "粗体应有对应属性");
        assert!(xml.contains("列表项 A"));
        assert!(xml.contains("let a = 1;"), "代码块内容应保留");
        assert!(xml.contains("引用内容"));
        assert!(xml.contains("列1"), "表格应写入");
        assert!(xml.contains(r#"w:val="Heading1""#), "标题应使用 Heading 样式");
        assert!(xml.contains("分类：技术/数据库"));
    }

    #[test]
    fn escapes_xml_special_characters() {
        let bytes = build_docx("A & B", "文本包含 <标签> 与 & 符号", "", &[], "", "").expect("生成");
        let xml = document_xml(&bytes);
        assert!(xml.contains("A &amp; B"));
        // pulldown-cmark 会把 `<标签>` 拆成 InlineHtml("<") + 文本，因此按片段断言
        assert!(xml.contains("&lt;"), "尖括号应被转义");
        assert!(xml.contains("标签"), "内容不应丢失");
        assert!(xml.contains("&amp;"), "& 应被转义");
        assert!(!xml.contains("<标签>"), "不应输出未转义的标签");
    }

    #[test]
    fn contains_all_required_parts() {
        let bytes = build_docx("标题", "正文", "", &[], "", "").expect("生成");
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("zip");
        for part in [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/document.xml",
            "word/styles.xml",
            "word/_rels/document.xml.rels",
            "docProps/core.xml",
            "docProps/app.xml",
        ] {
            assert!(archive.by_name(part).is_ok(), "缺少部件 {part}");
        }
    }
}
