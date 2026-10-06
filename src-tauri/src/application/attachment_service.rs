//! 附件落盘（V1.1 M2-1）。
//!
//! 编辑器粘贴 / 拖拽图片时，前端把内容以 Base64 传给本服务；
//! 服务把文件写入 `attachments/YYYY-MM/`，并返回可直接写进正文的 Markdown 链接路径。
//!
//! 设计要点：
//! - 附件是**普通文件**，不进入 `notes` 表，也不会被笔记扫描当作笔记；
//! - 文件名统一为 `时间戳-清洗后的原名-短 ID.扩展名`，避免重名与非法字符；
//! - 返回的 `linkPath` 已经相对当前笔记所在目录，正文里直接写 `![](linkPath)` 即可。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::core::base64::{self, split_data_url};
use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::paths::{ATTACHMENTS_DIR, join_and_normalize, relative_from};
use crate::core::text::sanitize_file_name;
use crate::core::time::now_file_timestamp;
use crate::infrastructure::markdown_storage::parent_relative;
use crate::domain::note::NoteStatus;
use crate::repositories::{NoteRepository, SqliteNoteRepository};

use super::context::WorkspaceContext;

/// 单个附件的大小上限（20 MB）。超过时提示用户改用「导入」或手动放置文件。
pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;

/// 预览时单张图片的内联上限（8 MB）：超过则不在预览中内联，避免 WebView 占用过高。
pub const MAX_PREVIEW_BYTES: usize = 8 * 1024 * 1024;

/// 允许落盘的扩展名白名单（图片 + 常见文档）。未知类型统一按 `bin` 处理。
const ALLOWED_EXTENSIONS: [&str; 16] = [
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "svg", "avif", "ico", "tif", "tiff", "heic",
    "pdf", "mp4", "webm", "txt",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAttachmentInput {
    /// 当前笔记 id；用于把附件路径换算成相对该笔记的链接
    #[serde(default)]
    pub note_id: Option<String>,
    /// 原始文件名（可含扩展名）
    #[serde(default)]
    pub file_name: Option<String>,
    /// MIME 类型；也接受把 `data:image/png;base64,...` 整体放在 `dataBase64`
    #[serde(default)]
    pub mime_type: Option<String>,
    pub data_base64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAttachment {
    /// 工作区相对路径，例如 `attachments/2026-10/20261007-101500-shot-ab12cd.png`
    pub relative_path: String,
    /// 相对当前笔记所在目录，可直接写入 Markdown
    pub link_path: String,
    pub absolute_path: String,
    pub file_name: String,
    pub bytes: u64,
    pub is_image: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadAttachmentInput {
    /// 当前笔记 id；用于把正文里的相对路径解析到工作区
    #[serde(default)]
    pub note_id: Option<String>,
    /// 正文里的原始链接目标，例如 `../attachments/2026-10/a.png`
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentData {
    /// `data:image/png;base64,...`，可直接作为 `<img src>`
    pub data_url: String,
    pub mime_type: String,
    pub bytes: u64,
}

/// 附件清单条目（V1.1.2 附件管理）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentEntry {
    /// 工作区相对路径
    pub relative_path: String,
    pub file_name: String,
    /// `YYYY-MM`（来自目录名，缺失时按修改时间推断）
    pub month: String,
    pub bytes: u64,
    pub modified_at: String,
    /// 引用了该附件的笔记（用于删除前的风险提示）
    pub references: Vec<AttachmentReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentReference {
    pub note_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentListReport {
    pub items: Vec<AttachmentEntry>,
    pub total_bytes: u64,
    /// 有笔记引用的附件数量
    pub referenced_count: usize,
}

pub struct AttachmentService {
    ctx: Arc<WorkspaceContext>,
}

impl AttachmentService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    pub fn save(&self, input: SaveAttachmentInput) -> Result<SavedAttachment> {
        let (data_url_mime, payload) = split_data_url(&input.data_base64);
        if payload.trim().is_empty() {
            return Err(AppError::new(ErrorCode::InvalidInput, "附件内容为空。"));
        }
        // Base64 膨胀约 4/3，提前用字符串长度拦截超大输入，避免无谓的解码开销
        if payload.len() > MAX_ATTACHMENT_BYTES / 3 * 4 + 1024 {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!(
                    "附件超过 {} MB 上限，请改用「导入外部文件」。",
                    MAX_ATTACHMENT_BYTES / 1024 / 1024
                ),
            ));
        }

        let bytes = base64::decode(payload)?;
        if bytes.is_empty() {
            return Err(AppError::new(ErrorCode::InvalidInput, "附件内容为空。"));
        }
        if bytes.len() > MAX_ATTACHMENT_BYTES {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!(
                    "附件超过 {} MB 上限，请改用「导入外部文件」。",
                    MAX_ATTACHMENT_BYTES / 1024 / 1024
                ),
            ));
        }

        let mime = input
            .mime_type
            .clone()
            .or_else(|| data_url_mime.clone())
            .unwrap_or_default();
        let extension = resolve_extension(input.file_name.as_deref(), &mime);
        let is_image = mime.starts_with("image/") || is_image_extension(&extension);

        let month = crate::core::time::now_iso().get(0..7).unwrap_or("0000-00").to_string();
        let directory = format!("{ATTACHMENTS_DIR}/{month}");
        let raw_stem = input.file_name.as_deref().unwrap_or("attachment");
        let raw_stem = raw_stem
            .rsplit_once('.')
            .map(|(base, _)| base)
            .filter(|base| !base.trim().is_empty())
            .unwrap_or(raw_stem);
        let stem = sanitize_file_name(raw_stem);
        let stem: String = stem.chars().take(40).collect();
        let stem = if stem.is_empty() { "attachment".to_string() } else { stem };
        let short_id: String = crate::core::ids::ulid().chars().take(6).collect();
        let file_name = format!(
            "{}-{}-{}.{}",
            now_file_timestamp(),
            stem,
            short_id.to_lowercase(),
            extension
        );
        let relative_path = format!("{directory}/{file_name}");

        let absolute = self.ctx.paths.resolve_relative(&relative_path)?;
        self.ctx.storage.write_atomic_absolute(&absolute, &bytes)?;

        let note_directory = self
            .ctx
            .db
            .with_conn(|conn| match input.note_id.as_deref() {
                Some(id) => SqliteNoteRepository.get(conn, id),
                None => Ok(None),
            })?
            .map(|note| parent_relative(&note.file_path))
            .unwrap_or_default();
        let link_path = relative_from(&note_directory, &relative_path);

        Ok(SavedAttachment {
            relative_path,
            link_path,
            absolute_path: absolute.to_string_lossy().to_string(),
            file_name,
            bytes: bytes.len() as u64,
            is_image,
        })
    }

    /// 读取工作区内的相对图片，返回 data URL 供预览渲染。
    ///
    /// 只接受工作区相对路径：外部 URL（http/https/data/asset）由调用方原样使用，
    /// 越出工作区的路径一律拒绝。
    pub fn read(&self, input: ReadAttachmentInput) -> Result<AttachmentData> {
        let source = input.source.trim();
        let lowered = source.to_ascii_lowercase();
        if source.is_empty()
            || lowered.starts_with("http://")
            || lowered.starts_with("https://")
            || lowered.starts_with("data:")
            || lowered.starts_with("asset:")
        {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                "只支持读取工作区内的相对路径图片。",
            ));
        }

        let note_directory = self
            .ctx
            .db
            .with_conn(|conn| match input.note_id.as_deref() {
                Some(id) => SqliteNoteRepository.get(conn, id),
                None => Ok(None),
            })?
            .map(|note| parent_relative(&note.file_path))
            .unwrap_or_default();

        let relative = join_and_normalize(&note_directory, source).ok_or_else(|| {
            AppError::new(ErrorCode::PathInvalid, "图片路径越出了知识库目录。")
        })?;
        let absolute = self.ctx.paths.resolve_relative(&relative)?;
        let bytes = std::fs::read(&absolute).map_err(|error| {
            AppError::new(
                ErrorCode::IoError,
                format!("读取图片失败：{error}（{relative}）"),
            )
        })?;
        if bytes.len() > MAX_PREVIEW_BYTES {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!("图片超过 {} MB，预览不内联显示。", MAX_PREVIEW_BYTES / 1024 / 1024),
            ));
        }

        let extension = relative
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .unwrap_or_default();
        let mime_type = mime_for_extension(&extension).to_string();
        let data_url = format!("data:{mime_type};base64,{}", base64::encode(&bytes));
        Ok(AttachmentData {
            data_url,
            mime_type,
            bytes: bytes.len() as u64,
        })
    }

    /// 列出附件并标注引用它的笔记（一次遍历正文，避免 N×M 扫描）。
    pub fn list(&self) -> Result<AttachmentListReport> {
        let files = self.ctx.storage.list_attachments()?;
        if files.is_empty() {
            return Ok(AttachmentListReport {
                items: Vec::new(),
                total_bytes: 0,
                referenced_count: 0,
            });
        }

        // 1) 先用「路径 → 条目索引」建表：正文里出现的是结尾片段（可能带 ../）
        let mut by_suffix: HashMap<String, Vec<usize>> = HashMap::new();
        let mut items: Vec<AttachmentEntry> = Vec::with_capacity(files.len());
        for relative in &files {
            let absolute = self.ctx.paths.resolve_relative(relative)?;
            let metadata = std::fs::metadata(&absolute).ok();
            let bytes = metadata.as_ref().map(|meta| meta.len()).unwrap_or(0);
            let modified_at = metadata
                .as_ref()
                .and_then(|meta| meta.modified().ok())
                .map(crate::core::time::system_time_to_iso)
                .unwrap_or_else(crate::core::time::now_iso);
            let file_name = relative
                .rsplit('/')
                .next()
                .unwrap_or(relative)
                .to_string();
            let month = relative
                .split('/')
                .nth(1)
                .unwrap_or("未分月")
                .to_string();
            let index = items.len();
            by_suffix
                .entry(relative.clone())
                .or_default()
                .push(index);
            items.push(AttachmentEntry {
                relative_path: relative.clone(),
                file_name,
                month,
                bytes,
                modified_at,
                references: Vec::new(),
            });
        }

        // 2) 遍历活跃笔记正文，找出写了 `attachments/...` 的引用
        let notes = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list_by_status(conn, &self.ctx.workspace.id, NoteStatus::Active)
        })?;
        for note in notes {
            let Ok(stored) = self.ctx.storage.read_note(&note.file_path) else {
                continue;
            };
            let mut seen: HashSet<usize> = HashSet::new();
            for target in extract_attachment_targets(&stored.body) {
                let Some(indexes) = by_suffix.get(&target) else {
                    continue;
                };
                for index in indexes {
                    if !seen.insert(*index) {
                        continue;
                    }
                    items[*index].references.push(AttachmentReference {
                        note_id: note.id.clone(),
                        title: note.title.clone(),
                    });
                }
            }
        }

        let total_bytes = items.iter().map(|item| item.bytes).sum();
        let referenced_count = items
            .iter()
            .filter(|item| !item.references.is_empty())
            .count();
        Ok(AttachmentListReport { items, total_bytes, referenced_count })
    }

    /// 删除附件（只允许 `attachments/` 下的文件）。返回释放的字节数。
    ///
    /// 调用方负责确认风险：仍然被笔记引用时只给警告，不阻止删除
    /// （用户可能正在清理误粘贴的图片，引用由笔记自己承担）。
    pub fn remove(&self, relative_path: &str) -> Result<u64> {
        let relative = relative_path.trim().replace('\\', "/");
        if !relative.starts_with(&format!("{ATTACHMENTS_DIR}/")) {
            return Err(AppError::new(
                ErrorCode::PathInvalid,
                "只能删除附件目录中的文件。",
            ));
        }
        let absolute = self.ctx.paths.resolve_relative(&relative)?;
        let bytes = std::fs::metadata(&absolute).map(|meta| meta.len()).unwrap_or(0);
        std::fs::remove_file(&absolute).map_err(|error| {
            AppError::new(ErrorCode::IoError, format!("删除附件失败：{error}"))
        })?;
        Ok(bytes)
    }

    /// 供附件管理界面展示「引用了该附件的笔记」。
    pub fn references(&self, relative_path: &str) -> Result<Vec<AttachmentReference>> {
        let report = self.list()?;
        Ok(report
            .items
            .into_iter()
            .find(|item| item.relative_path == relative_path.trim().replace('\\', "/"))
            .map(|item| item.references)
            .unwrap_or_default())
    }
}

/// 从正文里提取 `attachments/...` 形式的附件路径（去掉 `../` 前缀与结尾标点）。
fn extract_attachment_targets(body: &str) -> Vec<String> {
    const MARKER: &str = "attachments/";
    let mut targets = Vec::new();
    let bytes = body.as_bytes();
    let mut index = 0usize;
    while let Some(found) = body[index..].find(MARKER) {
        let start = index + found;
        let mut end = start;
        while end < bytes.len() {
            let ch = bytes[end] as char;
            if ch.is_whitespace() || matches!(ch, ')' | '"' | '\'' | '<' | '>' | ']' | '}') {
                break;
            }
            end += 1;
        }
        let candidate = body[start..end]
            .trim_end_matches(['.', ',', ';', ':'])
            .to_string();
        if candidate.len() > MARKER.len() {
            targets.push(candidate);
        }
        index = end.max(start + MARKER.len());
        if index >= body.len() {
            break;
        }
    }
    targets
}

fn mime_for_extension(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "tif" | "tiff" => "image/tiff",
        _ => "application/octet-stream",
    }
}

/// 扩展名优先取原始文件名，其次由 MIME 推断，最后回落到 `bin`。
fn resolve_extension(file_name: Option<&str>, mime: &str) -> String {
    if let Some(name) = file_name {
        if let Some((_, extension)) = name.rsplit_once('.') {
            let lower = extension.trim().to_ascii_lowercase();
            if ALLOWED_EXTENSIONS.contains(&lower.as_str()) {
                return lower;
            }
        }
    }
    let guessed = match mime.trim().to_ascii_lowercase().as_str() {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/svg+xml" => "svg",
        "image/avif" => "avif",
        "image/tiff" => "tiff",
        "application/pdf" => "pdf",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "text/plain" => "txt",
        _ => "bin",
    };
    guessed.to_string()
}

fn is_image_extension(extension: &str) -> bool {
    matches!(
        extension,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "avif" | "ico" | "tif"
            | "tiff" | "heic"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_prefers_file_name_then_mime() {
        assert_eq!(resolve_extension(Some("截图.PNG"), ""), "png");
        assert_eq!(resolve_extension(Some("资料.pdf"), "image/png"), "pdf");
        assert_eq!(resolve_extension(None, "image/jpeg"), "jpg");
        assert_eq!(resolve_extension(Some("noext"), "image/webp"), "webp");
        assert_eq!(resolve_extension(Some("a.unknownext"), ""), "bin");
    }

    #[test]
    fn image_detection() {
        assert!(is_image_extension("png"));
        assert!(is_image_extension("webp"));
        assert!(!is_image_extension("pdf"));
    }
}
