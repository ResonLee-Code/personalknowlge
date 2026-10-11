//! 笔记导出：Markdown / HTML / Word(docx) / PDF(打印视图)。
//!
//! 说明：
//! - `Markdown`：直接复制原文件（含 frontmatter），保证数据无损；
//! - `Html`：Markdown → 自包含 HTML（内联样式；**工作区内的图片内联成 data URL**）；
//! - `Docx`：Markdown → OOXML（Word / WPS 可直接打开，也可另存为 PDF）；
//! - `Pdf`：生成打印视图 HTML 并返回其内容与路径（图片同样内联），由前端打开系统打印流程
//!   （Windows 走 WebView 打印，macOS 在浏览器里「打印 → 存储为 PDF」）。
//!   这样无需引入体积庞大的 PDF 排版引擎与字体嵌入。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use regex::Regex;

use pulldown_cmark::{html, Options, Parser};
use serde::{Deserialize, Serialize};

use crate::core::error::{AppError, ErrorCode, Result};
use crate::application::attachment_service::{AttachmentService, ReadAttachmentInput};
use crate::core::text::sanitize_file_name;
use crate::infrastructure::frontmatter;
use crate::repositories::NoteRepository;

use super::context::WorkspaceContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Markdown,
    Html,
    Docx,
    Pdf,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Markdown => "md",
            ExportFormat::Html | ExportFormat::Pdf => "html",
            ExportFormat::Docx => "docx",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportNoteInput {
    pub id: String,
    pub format: ExportFormat,
    /// 目标文件路径；PDF 打印视图可省略（写入系统临时目录）
    #[serde(default)]
    pub target_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportNoteResult {
    pub format: ExportFormat,
    pub path: String,
    pub file_name: String,
    pub bytes: u64,
    /// PDF：需要前端打开打印视图（内容一并返回，避免二次读盘）
    pub print_ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
}

pub struct ExportService {
    ctx: Arc<WorkspaceContext>,
}

impl ExportService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    pub fn export_note(&self, input: ExportNoteInput) -> Result<ExportNoteResult> {
        let stored = self.read_stored_note(&input.id)?;
        let default_name = format!(
            "{}.{}",
            sanitize_file_name(&stored.title),
            input.format.extension()
        );

        match input.format {
            ExportFormat::Markdown => {
                let path = self.resolve_target(&input.target_path, &default_name)?;
                let bytes = stored.raw.as_bytes();
                write_file(&path, bytes)?;
                Ok(self.result(input.format, &path, bytes.len() as u64, false, None))
            }
            ExportFormat::Html => {
                let body = self.inline_workspace_images(&input.id, &stored.body);
                let page = render_html(&stored.title, &body, &stored.category_path);
                let path = self.resolve_target(&input.target_path, &default_name)?;
                write_file(&path, page.as_bytes())?;
                Ok(self.result(input.format, &path, page.len() as u64, false, None))
            }
            ExportFormat::Docx => {
                let bytes = crate::infrastructure::docx::build_docx(
                    &stored.title,
                    &stored.body,
                    &stored.category_path,
                    stored.tags.as_slice(),
                    &stored.created_at,
                    &stored.updated_at,
                )?;
                let path = self.resolve_target(&input.target_path, &default_name)?;
                write_file(&path, &bytes)?;
                Ok(self.result(input.format, &path, bytes.len() as u64, false, None))
            }
            ExportFormat::Pdf => {
                // 生成打印视图：默认写到系统临时目录，前端打开后走「打印 → 存储为 PDF」
                let body = self.inline_workspace_images(&input.id, &stored.body);
                let page = render_html(&stored.title, &body, &stored.category_path);
                let path = match input.target_path {
                    Some(target) => PathBuf::from(target),
                    None => {
                        let dir = std::env::temp_dir().join("personal-knowledge-os-export");
                        fs::create_dir_all(&dir)?;
                        dir.join(&default_name)
                    }
                };
                write_file(&path, page.as_bytes())?;
                Ok(self.result(
                    input.format,
                    &path,
                    page.len() as u64,
                    true,
                    Some(page),
                ))
            }
        }
    }

    fn result(
        &self,
        format: ExportFormat,
        path: &Path,
        bytes: u64,
        print_ready: bool,
        html: Option<String>,
    ) -> ExportNoteResult {
        ExportNoteResult {
            format,
            path: path.to_string_lossy().to_string(),
            file_name: path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default(),
            bytes,
            print_ready,
            html,
        }
    }

    /// 把正文里指向**工作区内**的图片换成 data URL。
    ///
    /// 导出文件（HTML / 打印视图）宣称「自包含」，但如果正文里是
    /// `../attachments/2026-10/x.png` 这种相对路径，离开工作区就是破图——
    /// 打印成 PDF 也会缺图。这里复用 `AttachmentService::read` 的解析口径
    /// （相对**当前笔记目录**），读不到或超过大小上限时**原样保留**，不让导出失败。
    fn inline_workspace_images(&self, note_id: &str, markdown: &str) -> String {
        static IMAGE: OnceLock<Regex> = OnceLock::new();
        let pattern = IMAGE.get_or_init(|| {
            Regex::new(r#"!\[([^\]]*)\]\(([^)\s]+)(\s+"[^"]*")?\)"#)
                .expect("内置图片正则必须合法")
        });
        if !markdown.contains("![") {
            return markdown.to_string();
        }
        let attachments = AttachmentService::new(Arc::clone(&self.ctx));
        pattern
            .replace_all(markdown, |caps: &regex::Captures<'_>| {
                let alt = caps.get(1).map(|item| item.as_str()).unwrap_or("");
                let source = caps.get(2).map(|item| item.as_str()).unwrap_or("");
                let title = caps.get(3).map(|item| item.as_str()).unwrap_or("");
                let original = caps.get(0).map(|item| item.as_str()).unwrap_or("");
                match attachments.read(ReadAttachmentInput {
                    note_id: Some(note_id.to_string()),
                    source: source.to_string(),
                }) {
                    Ok(data) => format!("![{alt}]({}){title}", data.data_url),
                    Err(_) => original.to_string(),
                }
            })
            .to_string()
    }

    fn resolve_target(&self, target: &Option<String>, default_name: &str) -> Result<PathBuf> {
        match target.as_deref().filter(|value| !value.trim().is_empty()) {
            Some(value) => {
                let path = PathBuf::from(value);
                if path.is_dir() {
                    Ok(path.join(default_name))
                } else {
                    Ok(path)
                }
            }
            None => {
                let dir = std::env::temp_dir().join("personal-knowledge-os-export");
                fs::create_dir_all(&dir)?;
                Ok(dir.join(default_name))
            }
        }
    }

    /// 读取笔记正文（不含 frontmatter）、原始文件、分类路径与标签。
    fn read_stored_note(&self, id: &str) -> Result<StoredNote> {
        let note = self
            .ctx
            .db
            .with_conn(|conn| crate::repositories::SqliteNoteRepository.get(conn, id))?
            .ok_or_else(|| AppError::new(ErrorCode::NoteNotFound, "笔记不存在或已被删除。"))?;
        let stored = self.ctx.storage.read_note(&note.file_path)?;
        let lookup = self.ctx.category_lookup()?;
        let category_path = note
            .category_id
            .as_ref()
            .map(|category_id| lookup.path_string(category_id))
            .unwrap_or_default();
        let tags = self
            .ctx
            .db
            .with_conn(|conn| crate::repositories::tag_repository::tags_for_note(conn, id))?
            .into_iter()
            .map(|tag| tag.name)
            .collect::<Vec<_>>();

        Ok(StoredNote {
            title: note.title.clone(),
            body: stored.body.clone(),
            raw: frontmatter::serialize(&stored.frontmatter, &stored.body),
            category_path,
            tags,
            created_at: note.created_at.clone(),
            updated_at: note.updated_at.clone(),
        })
    }
}

struct StoredNote {
    title: String,
    body: String,
    raw: String,
    category_path: String,
    tags: Vec<String>,
    created_at: String,
    updated_at: String,
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(path, bytes).map_err(|error| {
        AppError::new(ErrorCode::ExportFailed, format!("写入导出文件失败：{error}"))
    })?;
    Ok(())
}

/// Markdown → HTML 片段（GFM 表格 / 任务列表 / 删除线 / 脚注）。
pub fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(markdown, options);
    let mut output = String::new();
    html::push_html(&mut output, parser);
    output
}

/// 自包含 HTML 页面：样式内联，适合直接打印或分享。
pub fn render_html(title: &str, markdown: &str, category_path: &str) -> String {
    let body = markdown_to_html(markdown);
    let meta = if category_path.is_empty() {
        escape_html(title)
    } else {
        format!("{} · {}", escape_html(title), escape_html(category_path))
    };
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>{title}</title>
<style>
:root {{ color-scheme: light; }}
body {{ margin: 0; background: #f6f6f8; color: #1f2024;
  font-family: "PingFang SC", "Microsoft YaHei", "Segoe UI", system-ui, sans-serif; }}
main {{ max-width: 820px; margin: 0 auto; padding: 48px 28px 80px; background: #fff;
  box-shadow: 0 1px 3px rgba(0,0,0,.08); }}
h1, h2, h3, h4 {{ line-height: 1.35; margin: 1.5em 0 .6em; }}
h1 {{ font-size: 1.7em; border-bottom: 1px solid #e6e6ec; padding-bottom: .3em; }}
h2 {{ font-size: 1.35em; }}
code {{ font-family: "SF Mono", Menlo, Consolas, monospace; font-size: .9em;
  background: #f2f2f5; border: 1px solid #e6e6ec; border-radius: 4px; padding: .1em .35em; }}
pre {{ background: #f2f2f5; border: 1px solid #e6e6ec; border-radius: 8px; padding: 14px 16px; overflow: auto; }}
pre code {{ background: none; border: none; padding: 0; }}
blockquote {{ margin: 1em 0; padding: .2em 1em; border-left: 3px solid #d7d7de; color: #55565e; }}
table {{ border-collapse: collapse; width: 100%; margin: 1em 0; }}
th, td {{ border: 1px solid #e0e0e6; padding: .5em .7em; text-align: left; }}
th {{ background: #f6f6f8; }}
img {{ max-width: 100%; }}
a {{ color: #4f46e5; }}
.meta {{ color: #8b8b96; font-size: .85em; margin-bottom: 2em; }}
@media print {{ body {{ background: #fff; }} main {{ box-shadow: none; padding: 0; max-width: none; }} }}
</style>
</head>
<body>
<main>
<h1>{title}</h1>
<p class="meta">{meta}</p>
{body}
</main>
</body>
</html>
"#,
        title = escape_html(title),
        meta = meta,
        body = body,
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
