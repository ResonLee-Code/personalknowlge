//! 外部文件导入。
//!
//! 支持把工作区外的 Markdown / 纯文本文件（或整个目录）复制进知识库：
//! - `.md` / `.markdown` 会解析 frontmatter，保留 `id`、`title`、`tags`、`created_at`；
//! - `.txt` 整体作为正文，标题取文件名；
//! - 目录会递归展开，扁平导入到目标分类（默认 Inbox）；
//! - 源文件不会被修改（导入是复制而非移动）。
//!
//! 单个文件失败只记录到 `skipped`，不影响整批导入。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::events as event_names;
use crate::core::hash::sha256_hex_str;
use crate::core::ids::note_id;
use crate::core::paths::INBOX_DIR;
use crate::core::text::{count_words, markdown_to_plain_text, note_file_name};
use crate::core::time::{now_file_timestamp, now_iso, system_time_to_iso};
use crate::domain::note::{Note, NoteStatus};
use crate::infrastructure::frontmatter::{self, Frontmatter};
use crate::repositories::{NoteRepository, SqliteNoteRepository};

use super::context::WorkspaceContext;
use super::import_transform::{
    detect_mode, lookup_attachment, rewrite_notion_links, rewrite_obsidian_embeds,
    strip_notion_suffix, AttachmentMap, ImportMode,
};
use super::index_service::{file_stem_title, IndexService};
use super::tag_service::TagService;

/// 单文件体积上限，避免误导入超大文件。
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// 单次导入文件数量上限。
const MAX_FILES: usize = 500;
/// 允许导入的扩展名。
const SUPPORTED_EXTENSIONS: [&str; 5] = ["md", "markdown", "txt", "docx", "csv"];
/// Obsidian / Notion 模式下一并导入的附件扩展名。
const ATTACHMENT_EXTENSIONS: [&str; 11] = [
    "png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif", "pdf", "mp4", "webm",
];

/// 导入输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportNotesInput {
    /// 绝对路径列表，可以是文件或目录
    pub paths: Vec<String>,
    /// 目标分类；为空时导入到 Inbox
    #[serde(default)]
    pub category_id: Option<String>,
    /// 是否保留源目录结构（目录导入时在目标分类下重建子目录，并保留原文件名）
    #[serde(default)]
    pub preserve_structure: Option<bool>,
    /// 导入格式：`plain` / `obsidian` / `notion` / `auto`（默认 auto 自动识别）
    #[serde(default)]
    pub mode: Option<String>,
}

/// 成功导入的一篇笔记。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportedNote {
    pub source_path: String,
    pub note_id: String,
    pub title: String,
    pub file_path: String,
}

/// 被跳过的文件及原因。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub source_path: String,
    pub reason: String,
}

/// 导入结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub imported: Vec<ImportedNote>,
    pub skipped: Vec<SkippedFile>,
    /// 展开后参与导入的文件总数
    pub total: usize,
    /// 其中有问题的文件数量
    pub failed: usize,
    /// 一并导入的附件数量（Obsidian / Notion 模式）
    #[serde(default)]
    pub attachments: usize,
    /// 实际采用的导入模式（auto 时由识别结果决定）
    #[serde(default)]
    pub mode: String,
}

pub struct ImportService {
    ctx: Arc<WorkspaceContext>,
}

impl ImportService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    pub fn import(&self, input: ImportNotesInput) -> Result<ImportReport> {
        if input.paths.is_empty() {
            return Err(AppError::invalid_input("请先选择要导入的文件或目录。"));
        }

        let lookup = self.ctx.category_lookup()?;
        let (category_id, directory, category_path) = match input.category_id.as_deref() {
            Some(id) => {
                let category = lookup
                    .get(id)
                    .ok_or_else(|| AppError::new(ErrorCode::CategoryNotFound, "指定的分类不存在。"))?;
                (Some(category.id.clone()), lookup.directory_of(id), lookup.path_string(id))
            }
            None => (None, INBOX_DIR.to_string(), String::new()),
        };

        let preserve_structure = input.preserve_structure.unwrap_or(false);
        let requested_mode = ImportMode::parse(input.mode.as_deref());
        let mode = if requested_mode == ImportMode::Plain && input.mode.as_deref().is_none() {
            ImportMode::Plain
        } else {
            requested_mode
        };
        let mut report = ImportReport {
            imported: Vec::new(),
            skipped: Vec::new(),
            total: 0,
            failed: 0,
            attachments: 0,
            mode: String::new(),
        };
        // (文件, 该文件的导入根目录) —— 保留目录结构时用它算子路径
        let mut files: Vec<(PathBuf, PathBuf)> = Vec::new();
        for raw in &input.paths {
            let path = PathBuf::from(raw);
            if path.is_dir() {
                let mut collected: Vec<PathBuf> = Vec::new();
                self.collect_directory(&path, &mut collected, &mut report);
                files.extend(collected.into_iter().map(|file| (file, path.clone())));
            } else if path.is_file() {
                let root = path.parent().map(Path::to_path_buf).unwrap_or_default();
                files.push((path, root));
            } else {
                report.failed += 1;
                report.skipped.push(SkippedFile {
                    source_path: raw.clone(),
                    reason: "路径不存在或不可读".to_string(),
                });
            }
        }

        files.sort();
        files.dedup();

        // 自动识别：目录里有 .obsidian/ → Obsidian；出现 `名称 <32 位十六进制>.md` → Notion
        let mode = if input.mode.as_deref().unwrap_or("auto").eq_ignore_ascii_case("auto") {
            let hints: Vec<String> = input
                .paths
                .iter()
                .cloned()
                .chain(files.iter().map(|(file, _)| file.to_string_lossy().to_string()))
                .chain(
                    input
                        .paths
                        .iter()
                        .filter(|raw| Path::new(raw).join(".obsidian").is_dir())
                        .map(|raw| format!("{raw}/.obsidian")),
                )
                .collect();
            detect_mode(&hints)
        } else {
            mode
        };
        report.mode = mode.as_str().to_string();

        // Obsidian / Notion：把附件（图片、PDF 等）一并导入到 attachments/YYYY-MM/
        let attachment_map: AttachmentMap = if mode == ImportMode::Plain {
            AttachmentMap::new()
        } else {
            self.import_attachments(&input.paths, &mut report)?
        };
        if files.len() > MAX_FILES {
            report.failed += files.len() - MAX_FILES;
            for extra in files.drain(MAX_FILES..) {
                report.skipped.push(SkippedFile {
                    source_path: extra.0.to_string_lossy().to_string(),
                    reason: format!("超过单次导入上限（{MAX_FILES} 个文件）"),
                });
            }
        }
        report.total = files.len() + report.skipped.len();

        let mut seen_ids: HashSet<String> = HashSet::new();
        for (file, root) in files {
            let source_path = file.to_string_lossy().to_string();
            // 保留目录结构：把「相对于导入根目录」的子路径原样搬到目标分类下
            let sub_directory = if preserve_structure {
                file.parent()
                    .and_then(|parent| parent.strip_prefix(&root).ok())
                    .map(|sub| sub.to_string_lossy().replace('\\', "/"))
                    .filter(|sub| !sub.is_empty())
            } else {
                None
            };
            match self.import_file(
                &file,
                category_id.as_deref(),
                &directory,
                &category_path,
                &mut seen_ids,
                sub_directory.as_deref(),
                preserve_structure,
                mode,
                &attachment_map,
            ) {
                Ok(imported) => report.imported.push(imported),
                Err(error) => {
                    report.failed += 1;
                    report.skipped.push(SkippedFile { source_path, reason: error.message });
                }
            }
        }

        if !report.imported.is_empty() {
            self.ctx.events.emit(
                event_names::NOTES_CHANGED,
                serde_json::json!({
                    "kind": "imported",
                    "count": report.imported.len(),
                }),
            );
        }
        Ok(report)
    }

    /// 把目录/文件里的附件复制到 `attachments/YYYY-MM/`，返回「原文件名/原相对路径 → 新路径」映射。
    fn import_attachments(
        &self,
        paths: &[String],
        report: &mut ImportReport,
    ) -> Result<AttachmentMap> {
        let mut map = AttachmentMap::new();
        let month = now_iso().get(0..7).unwrap_or("0000-00").to_string();
        let directory = format!("{}/{}", crate::core::paths::ATTACHMENTS_DIR, month);
        let mut candidates: Vec<PathBuf> = Vec::new();
        for raw in paths {
            let path = PathBuf::from(raw);
            if path.is_dir() {
                for entry in WalkDir::new(&path).follow_links(false).into_iter().flatten() {
                    if entry.file_type().is_file() && is_attachment(entry.path()) {
                        candidates.push(entry.path().to_path_buf());
                    }
                }
            } else if path.is_file() && is_attachment(&path) {
                candidates.push(path);
            }
        }
        candidates.sort();
        for path in candidates {
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            if bytes.len() as u64 > MAX_FILE_BYTES {
                continue;
            }
            let original = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "attachment".to_string());
            let (stem, extension) = match original.rsplit_once('.') {
                Some((stem, extension)) => (stem.to_string(), extension.to_lowercase()),
                None => (original.clone(), "bin".to_string()),
            };
            let mut file_name = format!("{stem}.{extension}");
            let mut counter = 1;
            let mut relative = format!("{directory}/{file_name}");
            while self.ctx.storage.exists(&relative) {
                file_name = format!("{stem}-{counter}.{extension}");
                relative = format!("{directory}/{file_name}");
                counter += 1;
                if counter > 500 {
                    break;
                }
            }
            let absolute = self.ctx.paths.resolve_relative(&relative)?;
            self.ctx.storage.write_atomic_absolute(&absolute, &bytes)?;
            map.insert(original.clone(), relative.clone());
            map.insert(path.to_string_lossy().replace('\\', "/"), relative.clone());
            report.attachments += 1;
        }
        Ok(map)
    }

    /// 递归收集目录下可导入的文件。
    fn collect_directory(&self, dir: &Path, files: &mut Vec<PathBuf>, report: &mut ImportReport) {
        let mut cursor = 0usize;
        for entry in WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                // 根目录本身也要允许（临时目录名可能以 . 开头）
                entry.depth() == 0
                    || (!name.starts_with('.') && name != "node_modules")
            })
            .flatten()
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if !is_supported(path) {
                continue;
            }
            cursor += 1;
            if cursor > MAX_FILES {
                report.skipped.push(SkippedFile {
                    source_path: path.to_string_lossy().to_string(),
                    reason: format!("超过单次导入上限（{MAX_FILES} 个文件）"),
                });
                continue;
            }
            files.push(path.to_path_buf());
        }
    }

    /// 导入单个文件。
    fn import_file(
        &self,
        path: &Path,
        category_id: Option<&str>,
        directory: &str,
        category_path: &str,
        seen_ids: &mut HashSet<String>,
        sub_directory: Option<&str>,
        preserve_file_name: bool,
        mode: ImportMode,
        attachments: &AttachmentMap,
    ) -> Result<ImportedNote> {
        if !is_supported(path) {
            return Err(AppError::invalid_input(format!(
                "不支持的文件类型（仅支持 {}）",
                SUPPORTED_EXTENSIONS.join(" / ")
            )));
        }
        let metadata = std::fs::metadata(path)?;
        if metadata.len() > MAX_FILE_BYTES {
            return Err(AppError::invalid_input(format!(
                "文件超过 {} MB 上限",
                MAX_FILE_BYTES / 1024 / 1024
            )));
        }

        // Office 文档（.docx / .csv）先转成 Markdown，再走既有的导入管线
        let office_format = super::import_office::OfficeFormat::detect(path);

        let bytes = std::fs::read(path)?;
        let raw = String::from_utf8_lossy(&bytes).into_owned();
        let is_markdown = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "md" | "markdown"))
            .unwrap_or(false);

        // Markdown 解析 frontmatter；Office 转成 Markdown；纯文本整体作为正文
        let (source, body) = match (office_format, is_markdown) {
            (Some(format), _) => (
                Frontmatter::default(),
                super::import_office::to_markdown(path, format)?,
            ),
            (None, true) => {
                let parsed = frontmatter::parse(&raw);
                (parsed.frontmatter, parsed.body)
            }
            (None, false) => (Frontmatter::default(), raw),
        };

        let source_path = path.to_string_lossy().to_string();
        let raw_title = source
            .title
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| file_stem_title(&source_path));
        // Notion 导出：标题/文件名都带 ` <32 位十六进制>`，导入时去掉
        let title = if mode == ImportMode::Notion {
            strip_notion_suffix(&raw_title)
        } else {
            raw_title
        };
        let title = if title.trim().is_empty() {
            "导入笔记".to_string()
        } else {
            title
        };

        let modified_at = metadata
            .modified()
            .ok()
            .map(system_time_to_iso)
            .unwrap_or_else(now_iso);
        let created_at = source.created_at.clone().unwrap_or_else(|| modified_at.clone());
        let updated_at = source.updated_at.clone().unwrap_or_else(|| modified_at.clone());
        let tags = TagService::new(Arc::clone(&self.ctx)).normalize_names(source.tags.clone());

        // 复用源文件 id（便于重复导入时识别），但要避开冲突
        let id = match source.id.clone().filter(|value| !value.trim().is_empty()) {
            Some(candidate) if !seen_ids.contains(&candidate) && !self.id_exists(&candidate)? => {
                candidate
            }
            _ => note_id(),
        };
        seen_ids.insert(id.clone());

        let frontmatter = Frontmatter {
            id: Some(id.clone()),
            title: Some(title.clone()),
            note_type: source.note_type.clone().or_else(|| Some("note".to_string())),
            status: Some("active".to_string()),
            category: Some(category_path.to_string()).filter(|value| !value.is_empty()),
            tags: tags.clone(),
            created_at: Some(created_at.clone()),
            updated_at: Some(updated_at.clone()),
            source_type: source.source_type.clone(),
            source_ref: source.source_ref.clone(),
            source_reliability: source.source_reliability,
            valid_until: source.valid_until.clone(),
            relations: source.relations.clone(),
            parent: None,
            order: None,
            pinned: false,
            extra: source.extra.clone(),
        };
        // 正文转换：Obsidian 嵌入 → 标准图片链接；Notion 相对链接 → URL 解码
        let body = match mode {
            ImportMode::Obsidian => {
                let note_directory = match sub_directory {
                    Some(sub) => crate::infrastructure::markdown_storage::join_relative(directory, sub),
                    None => directory.to_string(),
                };
                let resolver = |target: &str| {
                    lookup_attachment(attachments, target).map(|relative| {
                        crate::core::paths::relative_from(&note_directory, &relative)
                    })
                };
                rewrite_obsidian_embeds(&body, &resolver)
            }
            ImportMode::Notion => rewrite_notion_links(&body),
            ImportMode::Plain => body,
        };
        let markdown = frontmatter::serialize(&frontmatter, &body);
        // 目标目录：保留目录结构时在目标分类下重建子目录
        let target_directory = match sub_directory {
            Some(sub) => crate::infrastructure::markdown_storage::join_relative(directory, sub),
            None => directory.to_string(),
        };
        let file_name = if preserve_file_name || mode == ImportMode::Notion {
            // 保留原文件名（去扩展名后交给 create_note_file 去重与清洗）；Notion 模式已去掉哈希后缀
            let stem = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_else(|| note_file_name(&title, &now_file_timestamp()));
            if mode == ImportMode::Notion {
                strip_notion_suffix(&stem)
            } else {
                stem
            }
        } else {
            note_file_name(&title, &now_file_timestamp())
        };
        let relative = self.ctx.storage.create_note_file(&target_directory, &file_name, &markdown)?;

        let note = Note {
            id: id.clone(),
            workspace_id: self.ctx.workspace.id.clone(),
            title: title.clone(),
            file_path: relative.clone(),
            category_id: category_id.map(|value| value.to_string()),
            status: NoteStatus::Active,
            content_hash: Some(sha256_hex_str(&markdown)),
            word_count: count_words(&markdown_to_plain_text(&body)),
            favorite: false,
            created_at,
            updated_at,
            last_indexed_at: None,
            file_size: None,
            file_mtime: None,
            parent_id: None,
            sort_order: 0,
            pinned: false,
            source_type: source.source_type.clone(),
            source_ref: source.source_ref.clone(),
            source_reliability: source.source_reliability,
            valid_until: source.valid_until.clone(),
        };
        self.ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.insert(conn, &note))?;

        if !tags.is_empty() {
            TagService::new(Arc::clone(&self.ctx)).set_note_tags(&id, &tags)?;
        }
        let index = IndexService::new(Arc::clone(&self.ctx));
        match index.index_document(&id, &title, &body, &tags, category_path) {
            Ok(()) => {
                let _ = self.ctx.db.with_conn(|conn| {
                    conn.execute(
                        "UPDATE notes SET last_indexed_at = ?2 WHERE id = ?1",
                        rusqlite::params![id, now_iso()],
                    )?;
                    Ok(())
                });
            }
            Err(error) => {
                // 正文已落盘，索引失败只标记过期
                let _ = index.mark_stale(Some(error.message.clone()));
            }
        }
        // 导入的笔记同样参与双向链接（也会把指向它的未解析链接绑定过来）
        let _ = super::relation_service::RelationService::new(Arc::clone(&self.ctx)).rebuild();
        if let Err(error) = super::link_service::LinkService::new(Arc::clone(&self.ctx))
            .on_note_available(&note, &body)
        {
            tracing::warn!(target: "links", "导入后更新链接失败：{}", error.message);
        }

        Ok(ImportedNote { source_path, note_id: id, title, file_path: relative })
    }

    fn id_exists(&self, note_id: &str) -> Result<bool> {
        Ok(self
            .ctx
            .db
            .with_conn(|conn| SqliteNoteRepository.get(conn, note_id))?
            .is_some())
    }
}

/// 是否属于支持导入的文件类型。
/// 是否是「附件」类文件（Obsidian / Notion 模式下一并导入）。
pub fn is_attachment(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ATTACHMENT_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}
