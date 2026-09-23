//! Markdown 文件存储（设计文档 12 / 13）。
//!
//! 关键保证：
//! - 原子写入：先写临时文件并 `fsync`，再替换目标文件
//! - 写入失败不会破坏原文件
//! - 所有路径操作都经过工作区边界检查
//! - 删除先进回收站，可恢复

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use walkdir::WalkDir;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::hash::sha256_hex;
use crate::core::paths::{is_temporary_file_name, normalize_relative, WorkspacePaths, TEMP_PREFIX};
use crate::core::text::sanitize_file_name;
use crate::infrastructure::frontmatter::{self, Frontmatter};

/// 应用自身写入记录，用于忽略文件监听器产生的回声事件（设计文档 19.2）。
#[derive(Default)]
pub struct RecentWrites {
    entries: Mutex<HashMap<String, (String, Instant)>>,
}

impl RecentWrites {
    pub fn mark(&self, relative: &str, hash: &str) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(relative.to_string(), (hash.to_string(), Instant::now()));
        }
    }

    /// 指定路径是否刚刚由应用写入，且内容未再变化。
    pub fn is_self_write(&self, relative: &str, current_hash: &str) -> bool {
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        entries.retain(|_, (_, at)| at.elapsed() < Duration::from_secs(15));
        match entries.get(relative) {
            Some((hash, at)) => *hash == current_hash && at.elapsed() < Duration::from_secs(15),
            None => false,
        }
    }

    pub fn forget(&self, relative: &str) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.remove(relative);
        }
    }
}

/// 一条笔记文件的读取结果。
pub struct StoredNote {
    pub relative_path: String,
    pub frontmatter: Frontmatter,
    pub body: String,
    pub content_hash: String,
    pub parse_error: Option<String>,
    pub modified_at: Option<String>,
}

/// Markdown 文件存储。
pub struct MarkdownStorage {
    paths: WorkspacePaths,
    recent_writes: Arc<RecentWrites>,
}

impl MarkdownStorage {
    pub fn new(paths: WorkspacePaths, recent_writes: Arc<RecentWrites>) -> Self {
        Self { paths, recent_writes }
    }

    pub fn paths(&self) -> &WorkspacePaths {
        &self.paths
    }

    pub fn recent_writes(&self) -> &Arc<RecentWrites> {
        &self.recent_writes
    }

    fn absolute(&self, relative: &str) -> Result<PathBuf> {
        self.paths.resolve_relative(relative)
    }

    pub fn exists(&self, relative: &str) -> bool {
        self.absolute(relative).map(|path| path.exists()).unwrap_or(false)
    }

    pub fn read_raw(&self, relative: &str) -> Result<String> {
        let path = self.absolute(relative)?;
        if !path.exists() {
            return Err(AppError::new(
                ErrorCode::NoteNotFound,
                format!("文件不存在：{relative}"),
            ));
        }
        Ok(fs::read_to_string(&path)?)
    }

    /// 读取并解析一篇笔记。
    pub fn read_note(&self, relative: &str) -> Result<StoredNote> {
        let path = self.absolute(relative)?;
        if !path.exists() {
            return Err(AppError::new(
                ErrorCode::NoteNotFound,
                format!("文件不存在：{relative}"),
            ));
        }
        let content = fs::read_to_string(&path)?;
        let parsed = frontmatter::parse(&content);
        let modified_at = fs::metadata(&path)
            .and_then(|meta| meta.modified())
            .ok()
            .map(crate::core::time::system_time_to_iso);

        Ok(StoredNote {
            relative_path: normalize_relative(relative),
            frontmatter: parsed.frontmatter,
            body: parsed.body,
            content_hash: sha256_hex(content.as_bytes()),
            parse_error: parsed.parse_error,
            modified_at,
        })
    }

    /// 原子写入文件内容，返回内容哈希。
    pub fn write_atomic(&self, relative: &str, content: &str) -> Result<String> {
        let path = self.absolute(relative)?;
        self.write_atomic_absolute(&path, content.as_bytes())?;
        let hash = sha256_hex(content.as_bytes());
        self.recent_writes.mark(&normalize_relative(relative), &hash);
        Ok(hash)
    }

    /// 原子写入绝对路径（用于回收站与恢复）。
    pub fn write_atomic_absolute(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| AppError::new(ErrorCode::PathInvalid, "目标文件缺少父目录。"))?;
        fs::create_dir_all(parent)?;

        let temp_path = parent.join(format!("{TEMP_PREFIX}{}.tmp", crate::core::ids::ulid()));
        let write_result = (|| -> std::io::Result<()> {
            let mut file = fs::File::create(&temp_path)?;
            file.write_all(bytes)?;
            file.flush()?;
            file.sync_all()?;
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = fs::remove_file(&temp_path);
            return Err(AppError::new(
                ErrorCode::NoteSaveFailed,
                format!("写入临时文件失败：{error}"),
            ));
        }

        if let Err(error) = replace_file(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(AppError::new(
                ErrorCode::NoteSaveFailed,
                format!("替换目标文件失败：{error}"),
            ));
        }
        Ok(())
    }

    /// 在工作区目录内创建笔记文件，自动避免重名。
    pub fn create_note_file(
        &self,
        dir_relative: &str,
        file_name: &str,
        content: &str,
    ) -> Result<String> {
        let relative = self.unique_relative_path(dir_relative, file_name)?;
        self.write_atomic(&relative, content)?;
        Ok(relative)
    }

    /// 生成不冲突的相对路径。
    pub fn unique_relative_path(&self, dir_relative: &str, file_name: &str) -> Result<String> {
        let dir_relative = normalize_relative(dir_relative);
        let stem = sanitize_file_name(file_name.trim_end_matches(".md"));
        let file_name = format!("{stem}.md");
        let first = join_relative(&dir_relative, &file_name);
        if !self.exists(&first) {
            return Ok(first);
        }
        for index in 1..1000 {
            let candidate = join_relative(&dir_relative, &format!("{stem}-{index}.md"));
            if !self.exists(&candidate) {
                return Ok(candidate);
            }
        }
        Err(AppError::new(
            ErrorCode::PathInvalid,
            "同名文件过多，请调整标题。",
        ))
    }

    /// 移动文件到指定目录（可同时改名）。
    pub fn move_note(
        &self,
        from: &str,
        to_dir_relative: &str,
        new_file_name: Option<&str>,
    ) -> Result<String> {
        let source = self.absolute(from)?;
        if !source.exists() {
            return Err(AppError::new(ErrorCode::NoteNotFound, format!("文件不存在：{from}")));
        }
        let file_name = match new_file_name {
            Some(name) => format!("{}.md", sanitize_file_name(name)),
            None => source
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "note.md".to_string()),
        };
        let target_relative = self.unique_relative_path(to_dir_relative, &file_name)?;
        let target = self.absolute(&target_relative)?;
        if source == target {
            return Ok(target_relative);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&source, &target).map_err(|error| {
            AppError::new(ErrorCode::NoteSaveFailed, format!("移动文件失败：{error}"))
        })?;
        self.recent_writes.forget(from);
        Ok(target_relative)
    }

    /// 把笔记移动到回收站，返回回收站相对路径。
    pub fn move_to_trash(&self, relative: &str) -> Result<String> {
        let source = self.absolute(relative)?;
        if !source.exists() {
            return Err(AppError::new(ErrorCode::NoteNotFound, format!("文件不存在：{relative}")));
        }
        let normalized = normalize_relative(relative);
        let mut target = self.paths.trash_dir().join(&normalized);
        if target.exists() {
            let stamp = crate::core::time::now_file_timestamp();
            let file_name = target
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "note.md".to_string());
            target = target.with_file_name(format!("{stamp}-{file_name}"));
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&source, &target).map_err(|error| {
            AppError::new(ErrorCode::IoError, format!("移动到回收站失败：{error}"))
        })?;
        self.recent_writes.forget(&normalized);
        self.paths.to_relative(&target)
    }

    /// 从回收站恢复到指定相对路径。若目标已存在则自动改名。
    pub fn restore_from_trash(&self, trash_relative: &str, target_relative: &str) -> Result<String> {
        let source = self.absolute(trash_relative)?;
        if !source.exists() {
            return Err(AppError::new(
                ErrorCode::NoteNotFound,
                format!("回收站文件不存在：{trash_relative}"),
            ));
        }
        let final_relative = if self.exists(target_relative) {
            let dir = parent_relative(target_relative);
            let name = Path::new(target_relative)
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
                .unwrap_or_else(|| "note".to_string());
            let stamp = crate::core::time::now_file_timestamp();
            self.unique_relative_path(&dir, &format!("{name}-{stamp}"))?
        } else {
            normalize_relative(target_relative)
        };
        let target = self.absolute(&final_relative)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&source, &target).map_err(|error| {
            AppError::new(ErrorCode::IoError, format!("恢复文件失败：{error}"))
        })?;
        Ok(final_relative)
    }

    /// 永久删除回收站中的文件。
    pub fn delete_from_trash(&self, trash_relative: &str) -> Result<()> {
        let path = self.absolute(trash_relative)?;
        if !path.exists() {
            return Ok(());
        }
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else {
            fs::remove_file(&path)?;
        }
        Ok(())
    }

    /// 列出工作区内所有 Markdown 文件（相对路径，已排序）。
    pub fn list_markdown_files(&self) -> Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in WalkDir::new(self.paths.root())
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                !(entry.file_type().is_dir()
                    && (name == crate::core::paths::KNOWLEDGE_DIR
                        || name == crate::core::paths::TRASH_DIR))
                    && !(entry.file_type().is_file() && is_temporary_file_name(&name))
            })
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    tracing::warn!(target: "storage", "扫描目录时跳过条目：{error}");
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            if !WorkspacePaths::is_markdown_file(entry.path()) {
                continue;
            }
            let relative = self.paths.to_relative(entry.path())?;
            if WorkspacePaths::is_ignored_relative(&relative) {
                continue;
            }
            files.push(relative);
        }
        files.sort();
        Ok(files)
    }

    /// 列出附件目录中的文件。
    pub fn list_attachments(&self) -> Result<Vec<String>> {
        let dir = self.paths.attachments_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        for entry in WalkDir::new(&dir).follow_links(false).into_iter().flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            if let Ok(relative) = self.paths.to_relative(entry.path()) {
                if !is_temporary_file_name(&relative) {
                    files.push(relative);
                }
            }
        }
        files.sort();
        Ok(files)
    }

    /// 复制一个外部文件到工作区（用于附件）。
    pub fn copy_file(&self, source: &Path, target_relative: &str) -> Result<String> {
        let target = self.absolute(target_relative)?;
        let bytes = fs::read(source)?;
        self.write_atomic_absolute(&target, &bytes)?;
        Ok(normalize_relative(target_relative))
    }

    /// 文件大小与修改时间，用于文件监听轮询（设计文档 19）。
    pub fn file_stamp(&self, relative: &str) -> Result<(u64, Option<String>)> {
        let path = self.absolute(relative)?;
        let metadata = fs::metadata(&path)?;
        let modified = metadata
            .modified()
            .ok()
            .map(crate::core::time::system_time_to_iso);
        Ok((metadata.len(), modified))
    }
}

/// 跨平台替换文件（Windows 下目标存在时 rename 会失败）。
fn replace_file(temp: &Path, target: &Path) -> std::io::Result<()> {
    match fs::rename(temp, target) {
        Ok(()) => Ok(()),
        Err(error) => {
            if target.exists() {
                fs::remove_file(target)?;
                fs::rename(temp, target)?;
                return Ok(());
            }
            Err(error)
        }
    }
}

/// 拼接工作区相对路径。
pub fn join_relative(dir: &str, name: &str) -> String {
    let dir = normalize_relative(dir);
    if dir.is_empty() {
        normalize_relative(name)
    } else {
        format!("{}/{}", dir.trim_end_matches('/'), normalize_relative(name))
    }
}

/// 取相对路径的父目录。
pub fn parent_relative(relative: &str) -> String {
    let normalized = normalize_relative(relative);
    match normalized.rsplit_once('/') {
        Some((parent, _)) => parent.to_string(),
        None => String::new(),
    }
}

/// 取相对路径的文件名。
pub fn file_name_of(relative: &str) -> String {
    let normalized = normalize_relative(relative);
    normalized.rsplit('/').next().unwrap_or("").to_string()
}

/// 备份文件名（设计文档 20.3）。
pub fn backup_file_name(prefix: &str) -> String {
    let stamp = crate::core::time::now_file_timestamp();
    format!("{prefix}_{stamp}.zip")
}
