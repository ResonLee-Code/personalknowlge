//! 本地笔记模板（V1.1 M2-2 / F8）。
//!
//! 模板是工作区内的普通 Markdown 文件，放在 `.knowledge/templates/*.md`；
//! 新建笔记时选择一个模板，服务把占位符替换成实际值后返回内容，
//! 前端再走常规的「新建笔记」写入路径（保持单一写入路径）。
//!
//! 支持的占位符：`{{title}}`、`{{date}}`、`{{time}}`、`{{datetime}}`、`{{weekday}}`
//! （另接受中文 `{{标题}}`、`{{日期}}`、`{{时间}}`）。

use std::fs;
use std::sync::Arc;

use chrono::{Datelike, Local};
use serde::{Deserialize, Serialize};

use crate::core::error::{AppError, ErrorCode, Result};

use super::context::WorkspaceContext;

/// 单个模板的读取上限（1 MB），避免误把大文件当模板。
const MAX_TEMPLATE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSummary {
    /// 模板名（文件名去掉扩展名），用于后续 render
    pub name: String,
    pub file_name: String,
    /// 取正文第一个 `#` 标题，没有则回落到模板名
    pub title: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderTemplateInput {
    pub name: String,
    /// 用于替换 `{{title}}`
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedTemplate {
    pub name: String,
    pub content: String,
}

/// 把一篇笔记另存为模板（对标语雀的「另存为模板」）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveTemplateInput {
    /// 模板名（不含扩展名）；为空时回落到笔记标题
    #[serde(default)]
    pub name: Option<String>,
    /// 模板正文（Markdown，不含 frontmatter）
    pub content: String,
    /// 覆盖同名模板
    #[serde(default)]
    pub overwrite: bool,
}

pub struct TemplateService {
    ctx: Arc<WorkspaceContext>,
}

impl TemplateService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    /// 列出模板（按名称排序）。目录不存在时返回空列表，不隐式创建文件。
    pub fn list(&self) -> Result<Vec<TemplateSummary>> {
        let dir = self.ctx.paths.templates_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut templates = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || !crate::core::paths::WorkspacePaths::is_markdown_file(&path) {
                continue;
            }
            let Some(file_name) = path.file_name().map(|name| name.to_string_lossy().to_string())
            else {
                continue;
            };
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_else(|| file_name.clone());
            let bytes = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            // 标题只读取前几行，避免大文件全量读入
            let title = read_first_heading(&path).unwrap_or_else(|| name.clone());
            templates.push(TemplateSummary { name, file_name, title, bytes });
        }

        templates.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(templates)
    }

    /// 读取模板并替换占位符。
    pub fn render(&self, input: RenderTemplateInput) -> Result<RenderedTemplate> {
        let name = input.name.trim();
        let path = self.template_path(name)?;
        let metadata = fs::metadata(&path)?;
        if metadata.len() > MAX_TEMPLATE_BYTES {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "模板文件超过 1 MB，请精简后再使用。",
            ));
        }
        let raw = fs::read_to_string(&path)?;
        let title = input
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("未命名笔记");
        Ok(RenderedTemplate {
            name: name.to_string(),
            content: render_placeholders(&raw, title, Local::now()),
        })
    }

    /// 解析模板文件路径：只接受 `.knowledge/templates` 下的 Markdown 文件。
    fn template_path(&self, name: &str) -> Result<std::path::PathBuf> {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.contains(['/', '\\']) || trimmed.contains("..") {
            return Err(AppError::new(ErrorCode::InvalidInput, "模板名不合法。"));
        }
        let stem = trimmed.strip_suffix(".md").unwrap_or(trimmed);
        let path = self.ctx.paths.templates_dir().join(format!("{stem}.md"));
        if !path.is_file() {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!("模板「{stem}」不存在，请检查 .knowledge/templates 目录。"),
            ));
        }
        Ok(path)
    }

    /// 把当前笔记正文另存为模板：写入 `.knowledge/templates/<name>.md`。
    ///
    /// 与语雀「另存为模板」一致——模板是纯本地文件，用户也可以直接在文件夹里编辑；
    /// 正文里的 `{{title}}` 等占位符保持原样，下次用模板新建时再替换。
    pub fn save(&self, input: SaveTemplateInput) -> Result<TemplateSummary> {
        let name = normalize_template_name(input.name.as_deref())?;
        let dir = self.ctx.paths.templates_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{name}.md"));
        if path.exists() && !input.overwrite {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!("模板「{name}」已存在，请换一个名字或确认覆盖。"),
            ));
        }
        let body = input.content.trim_end();
        // 模板首行补一个 `{{title}}` 标题，新建笔记时会被替换成实际标题
        let content = if body.starts_with("# ") {
            format!("{body}\n")
        } else {
            format!("# {{{{title}}}}\n\n{body}\n")
        };
        // 复用笔记写入的原子写（先写临时文件再 rename），避免半截模板文件
        self.ctx.storage.write_atomic_absolute(&path, content.as_bytes())?;
        let file_name = format!("{name}.md");
        let title = read_first_heading(&path).unwrap_or_else(|| name.clone());
        Ok(TemplateSummary {
            name,
            file_name,
            title,
            bytes: content.len() as u64,
        })
    }
}

/// 规范化模板名：去掉扩展名、禁止路径分隔符与 `..`，长度限制 60 字符。
fn normalize_template_name(raw: Option<&str>) -> Result<String> {
    let trimmed = raw.unwrap_or("").trim();
    let stem = trimmed.strip_suffix(".md").unwrap_or(trimmed).trim();
    if stem.is_empty() {
        return Err(AppError::new(ErrorCode::InvalidInput, "请填写模板名。"));
    }
    if stem.contains(['/', '\\']) || stem.contains("..") {
        return Err(AppError::new(ErrorCode::InvalidInput, "模板名不能包含路径分隔符。"));
    }
    let cleaned: String = stem
        .chars()
        .filter(|ch| !matches!(ch, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let limited: String = cleaned.chars().take(60).collect();
    if limited.trim().is_empty() {
        return Err(AppError::new(ErrorCode::InvalidInput, "模板名不合法。"));
    }
    Ok(limited.trim().to_string())
}

/// 读取文件首个 `#` 标题（最多读取 200 行）。
/// 含占位符（`{{…}}`）的标题会被跳过，避免模板列表里显示 `{{title}}`。
fn read_first_heading(path: &std::path::Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    content.lines().take(200).find_map(|line| {
        let trimmed = line.trim();
        let heading = trimmed.strip_prefix("# ")?.trim();
        if heading.is_empty() || heading.contains("{{") {
            None
        } else {
            Some(heading.to_string())
        }
    })
}

/// 替换模板占位符。未知占位符保持原样，便于用户自定义。
fn render_placeholders(content: &str, title: &str, now: chrono::DateTime<Local>) -> String {
    let weekday = match now.weekday() {
        chrono::Weekday::Mon => "周一",
        chrono::Weekday::Tue => "周二",
        chrono::Weekday::Wed => "周三",
        chrono::Weekday::Thu => "周四",
        chrono::Weekday::Fri => "周五",
        chrono::Weekday::Sat => "周六",
        chrono::Weekday::Sun => "周日",
    };

    content
        .replace("{{title}}", title)
        .replace("{{标题}}", title)
        .replace("{{date}}", &now.format("%Y-%m-%d").to_string())
        .replace("{{日期}}", &now.format("%Y-%m-%d").to_string())
        .replace("{{time}}", &now.format("%H:%M").to_string())
        .replace("{{时间}}", &now.format("%H:%M").to_string())
        .replace("{{datetime}}", &now.format("%Y-%m-%d %H:%M").to_string())
        .replace("{{weekday}}", weekday)
}

/// 供其他模块复用的「模板占位符说明」（新建对话框展示）。
pub fn placeholder_hint() -> &'static str {
    "{{title}} {{date}} {{time}} {{datetime}} {{weekday}}"
}

/// 供命令层返回的元信息（模板目录位置）。
pub fn templates_relative_dir() -> String {
    format!(
        "{}/{}",
        crate::core::paths::KNOWLEDGE_DIR,
        crate::core::paths::TEMPLATES_DIR
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn replaces_known_placeholders_and_keeps_unknown() {
        let now = Local.with_ymd_and_hms(2026, 10, 7, 9, 5, 0).unwrap();
        let content = "# {{title}} {{未定义}}\n\n日期：{{date}} {{time}} {{weekday}}";
        let rendered = render_placeholders(content, "周会记录", now);

        assert!(rendered.starts_with("# 周会记录 {{未定义}}"));
        assert!(rendered.contains("日期：2026-10-07 09:05 周三"));
    }

    #[test]
    fn supports_chinese_placeholders() {
        let now = Local.with_ymd_and_hms(2026, 1, 2, 8, 0, 0).unwrap();
        let rendered = render_placeholders("{{标题}} {{日期}} {{时间}}", "测试", now);
        assert_eq!(rendered, "测试 2026-01-02 08:00");
    }

    #[test]
    fn exposes_documented_hint_and_directory() {
        assert!(placeholder_hint().contains("{{title}}"));
        assert_eq!(templates_relative_dir(), ".knowledge/templates");
    }
}
