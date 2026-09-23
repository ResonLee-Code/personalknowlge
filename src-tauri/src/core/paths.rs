//! 工作区路径与路径安全。
//!
//! 设计文档 8.2 / 12.1 / 19.1：所有文件操作必须经过路径安全检查，
//! 并且必须忽略内部目录、临时文件与隐藏文件。

use std::path::{Component, Path, PathBuf};

use crate::core::error::{AppError, ErrorCode, Result};

/// 工作区目录约定（设计文档 12.1）。
pub const KNOWLEDGE_DIR: &str = ".knowledge";
pub const TRASH_DIR: &str = ".trash";
pub const ATTACHMENTS_DIR: &str = "attachments";
pub const INBOX_DIR: &str = "00_Inbox";
pub const ARCHIVE_DIR: &str = "99_Archive";
pub const DEFAULT_CATEGORY_DIRS: [&str; 4] = ["01_技术", "02_工作", "03_项目", "04_生活"];
pub const BACKUP_DIR: &str = "backups";
pub const LOG_DIR: &str = "logs";
pub const INDEX_DIR: &str = "index";
pub const DATABASE_FILE: &str = "database.sqlite";
pub const CONFIG_FILE: &str = "config.json";
pub const TEMP_PREFIX: &str = ".pkos-tmp-";

/// 工作区路径集合。
#[derive(Debug, Clone)]
pub struct WorkspacePaths {
    root: PathBuf,
}

impl WorkspacePaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn knowledge_dir(&self) -> PathBuf {
        self.root.join(KNOWLEDGE_DIR)
    }

    pub fn database_path(&self) -> PathBuf {
        self.knowledge_dir().join(DATABASE_FILE)
    }

    pub fn config_path(&self) -> PathBuf {
        self.knowledge_dir().join(CONFIG_FILE)
    }

    pub fn index_dir(&self) -> PathBuf {
        self.knowledge_dir().join(INDEX_DIR)
    }

    pub fn log_dir(&self) -> PathBuf {
        self.knowledge_dir().join(LOG_DIR)
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.knowledge_dir().join(BACKUP_DIR)
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.root.join(TRASH_DIR)
    }

    pub fn attachments_dir(&self) -> PathBuf {
        self.root.join(ATTACHMENTS_DIR)
    }

    pub fn inbox_dir(&self) -> PathBuf {
        self.root.join(INBOX_DIR)
    }

    /// 创建缺失的目录结构。已存在的目录不会被破坏。
    pub fn ensure_layout(&self) -> Result<()> {
        let mut dirs = vec![
            self.knowledge_dir(),
            self.index_dir(),
            self.log_dir(),
            self.backups_dir(),
            self.trash_dir(),
            self.attachments_dir(),
            self.inbox_dir(),
            self.root.join(ARCHIVE_DIR),
        ];
        for name in DEFAULT_CATEGORY_DIRS {
            dirs.push(self.root.join(name));
        }
        for dir in dirs {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    /// 校验根目录：必须是绝对路径且可写。
    pub fn validate_root(root: &Path) -> Result<PathBuf> {
        if !root.is_absolute() {
            return Err(AppError::new(ErrorCode::PathInvalid, "工作区路径必须是绝对路径。"));
        }
        if root.exists() {
            if !root.is_dir() {
                return Err(AppError::new(ErrorCode::PathInvalid, "工作区路径已存在且不是目录。"));
            }
            return Ok(std::fs::canonicalize(root)?);
        }
        // 目录不存在时，校验最近的已存在父目录
        let mut parent = root.parent();
        while let Some(candidate) = parent {
            if candidate.exists() {
                if !candidate.is_dir() {
                    return Err(AppError::new(
                        ErrorCode::PathInvalid,
                        "工作区路径的父级不是目录。",
                    ));
                }
                return Ok(root.to_path_buf());
            }
            parent = candidate.parent();
        }
        Err(AppError::new(ErrorCode::PathInvalid, "无法解析工作区路径。"))
    }

    /// 把相对路径解析为工作区内的绝对路径，并阻止路径穿越。
    pub fn resolve_relative(&self, relative: &str) -> Result<PathBuf> {
        if relative.contains('\0') {
            return Err(AppError::new(ErrorCode::PathInvalid, "路径包含非法字符。"));
        }
        let candidate = Path::new(relative);
        if candidate.is_absolute() {
            return Err(AppError::new(ErrorCode::PathInvalid, "不允许使用绝对路径。"));
        }
        let mut clean = PathBuf::new();
        for component in candidate.components() {
            match component {
                Component::Normal(part) => clean.push(part),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(AppError::new(
                        ErrorCode::PathInvalid,
                        "路径不允许包含上级目录引用。",
                    ));
                }
            }
        }
        let joined = self.root.join(clean);
        if !joined.starts_with(&self.root) {
            return Err(AppError::new(ErrorCode::PathInvalid, "路径越出工作区范围。"));
        }
        // 已存在的路径再做一次真实路径校验，防止符号链接逃逸
        if joined.exists() {
            let canonical_root = std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
            let canonical = std::fs::canonicalize(&joined)?;
            if !canonical.starts_with(&canonical_root) {
                return Err(AppError::new(ErrorCode::PathInvalid, "路径越出工作区范围。"));
            }
        }
        Ok(joined)
    }

    /// 绝对路径 -> 工作区相对路径（统一使用 `/` 分隔）。
    pub fn to_relative(&self, absolute: &Path) -> Result<String> {
        let relative = absolute.strip_prefix(&self.root).map_err(|_| {
            AppError::new(ErrorCode::PathInvalid, "路径不属于当前工作区。")
        })?;
        Ok(normalize_relative(&relative.to_string_lossy()))
    }

    /// 判断相对路径是否属于需要忽略的内部路径。
    pub fn is_ignored_relative(relative: &str) -> bool {
        let normalized = normalize_relative(relative);
        if normalized.is_empty() {
            return false;
        }
        for component in normalized.split('/') {
            if component.is_empty() {
                continue;
            }
            if component.starts_with('.') {
                return true; // .knowledge / .trash / 其他隐藏文件
            }
            if component == "node_modules" {
                return true;
            }
        }
        let name = normalized.rsplit('/').next().unwrap_or(&normalized);
        is_temporary_file_name(name)
    }

    /// 判断是否为 Markdown 文件。
    pub fn is_markdown_file(path: &Path) -> bool {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) => {
                let ext = ext.to_ascii_lowercase();
                ext == "md" || ext == "markdown"
            }
            None => false,
        }
    }
}

/// 统一相对路径分隔符为 `/`。
pub fn normalize_relative(value: &str) -> String {
    value.replace('\\', "/").trim_start_matches("./").to_string()
}

/// 判断文件名是否是临时/编辑器中间文件。
pub fn is_temporary_file_name(name: &str) -> bool {
    if name.starts_with(TEMP_PREFIX) || name.starts_with("~$") {
        return true;
    }
    // 编辑器/办公软件常见的临时文件：~a.md、a.md~、#a.md#
    if name.starts_with('~') || name.ends_with('~') {
        return true;
    }
    if name.starts_with('#') && name.ends_with('#') && name.len() > 1 {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    [".tmp", ".temp", ".swp", ".swx", ".part", ".crdownload", ".bak"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
}

/// 目录名 -> 分类名（去掉 `00_` 之类的排序前缀）。
pub fn dir_name_to_category_name(dir_name: &str) -> String {
    let trimmed = dir_name.trim();
    let mut chars = trimmed.char_indices();
    let mut end = 0usize;
    for (index, ch) in chars.by_ref() {
        if ch.is_ascii_digit() || ch == '-' || ch == '_' {
            let next = trimmed[index + ch.len_utf8()..].chars().next();
            if next == Some('_') {
                end = index + ch.len_utf8() + 1;
                break;
            }
            continue;
        }
        break;
    }
    let name = trimmed[end..].trim();
    if name.is_empty() {
        trimmed.to_string()
    } else {
        name.to_string()
    }
}

/// 分类名 -> 目录名（附加排序前缀）。
pub fn category_name_to_dir_name(name: &str, sort_order: i64) -> String {
    let cleaned = crate::core::text::sanitize_file_name(name);
    format!("{:02}_{}", sort_order.rem_euclid(100), cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn workspace_paths() -> (tempfile::TempDir, WorkspacePaths) {
        let dir = tempfile::tempdir().expect("创建临时目录");
        let paths = WorkspacePaths::new(dir.path().to_path_buf());
        paths.ensure_layout().expect("初始化目录结构");
        (dir, paths)
    }

    #[test]
    fn resolve_rejects_traversal_and_absolute_paths() {
        let (_dir, paths) = workspace_paths();
        assert!(paths.resolve_relative("../secret.md").is_err());
        assert!(paths.resolve_relative("a/../../secret.md").is_err());
        assert!(paths.resolve_relative("/etc/passwd").is_err());
        assert!(paths.resolve_relative("notes/ok.md").is_ok());
        assert!(paths.resolve_relative("./notes/ok.md").is_ok());
    }

    #[test]
    fn resolve_normalizes_current_dir_components() {
        let (_dir, paths) = workspace_paths();
        let resolved = paths.resolve_relative("./a/./b.md").expect("路径应可解析");
        assert!(resolved.ends_with("a/b.md"));
    }

    #[test]
    fn ignored_paths_cover_internal_directories() {
        assert!(WorkspacePaths::is_ignored_relative(".knowledge/database.sqlite"));
        assert!(WorkspacePaths::is_ignored_relative(".trash/00_Inbox/a.md"));
        assert!(WorkspacePaths::is_ignored_relative("00_Inbox/.hidden.md"));
        assert!(WorkspacePaths::is_ignored_relative("00_Inbox/a.md.tmp"));
        assert!(WorkspacePaths::is_ignored_relative("00_Inbox/.pkos-tmp-1.tmp"));
        assert!(WorkspacePaths::is_ignored_relative("00_Inbox/~a.md"));
        assert!(!WorkspacePaths::is_ignored_relative("00_Inbox/a.md"));
        assert!(!WorkspacePaths::is_ignored_relative("01_技术/数据库/索引.md"));
    }

    #[test]
    fn markdown_detection_is_case_insensitive() {
        assert!(WorkspacePaths::is_markdown_file(Path::new("a.md")));
        assert!(WorkspacePaths::is_markdown_file(Path::new("a.MARKDOWN")));
        assert!(!WorkspacePaths::is_markdown_file(Path::new("a.txt")));
        assert!(!WorkspacePaths::is_markdown_file(Path::new("noext")));
    }

    #[test]
    fn layout_paths_follow_document_conventions() {
        let (_dir, paths) = workspace_paths();
        assert!(paths.knowledge_dir().join("database.sqlite").exists() == false);
        assert!(paths.knowledge_dir().is_dir());
        assert!(paths.trash_dir().is_dir());
        assert!(paths.attachments_dir().is_dir());
        assert!(paths.inbox_dir().is_dir());
        assert!(paths.log_dir().is_dir());
        assert!(paths.backups_dir().is_dir());
        for directory in DEFAULT_CATEGORY_DIRS {
            assert!(paths.root().join(directory).is_dir(), "{directory} 应存在");
        }
    }

    #[test]
    fn relative_path_round_trip_uses_forward_slashes() {
        let (_dir, paths) = workspace_paths();
        let absolute = paths.root().join("01_技术").join("a.md");
        assert_eq!(paths.to_relative(&absolute).unwrap(), "01_技术/a.md");
        let outside = PathBuf::from("/tmp/not-in-workspace.md");
        assert!(paths.to_relative(&outside).is_err());
    }

    #[test]
    fn directory_name_conversion_is_stable() {
        assert_eq!(dir_name_to_category_name("01_技术"), "技术");
        assert_eq!(dir_name_to_category_name("99_Archive"), "Archive");
        assert_eq!(dir_name_to_category_name("技术"), "技术");
        assert_eq!(category_name_to_dir_name("技术", 1), "01_技术");
        assert_eq!(category_name_to_dir_name("数据库 设计", 12), "12_数据库-设计");
    }

    #[test]
    fn temporary_file_detection() {
        assert!(is_temporary_file_name(".pkos-tmp-abc.tmp"));
        assert!(is_temporary_file_name("note.md~"));
        assert!(is_temporary_file_name("~$note.md"));
        assert!(is_temporary_file_name("note.swp"));
        assert!(!is_temporary_file_name("note.md"));
    }
}
