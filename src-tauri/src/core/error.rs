//! 统一错误类型。
//!
//! 设计文档 14.9 要求错误使用统一结构：
//! `{ code, message, details }`，前端据此展示可理解的提示。

use std::fmt;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// 统一结果类型。
pub type Result<T> = std::result::Result<T, AppError>;

/// 错误码。前端可按错误码做分支处理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidInput,
    NotInitialized,
    WorkspaceNotFound,
    WorkspaceAlreadyExists,
    WorkspaceInvalid,
    NoteNotFound,
    NoteSaveFailed,
    NoteConflict,
    CategoryNotFound,
    CategoryNameConflict,
    CategoryMoveInvalid,
    TagNotFound,
    FileAccessDenied,
    PathInvalid,
    DatabaseError,
    SearchIndexError,
    BackupFailed,
    ExportFailed,
    IoError,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidInput => "INVALID_INPUT",
            ErrorCode::NotInitialized => "NOT_INITIALIZED",
            ErrorCode::WorkspaceNotFound => "WORKSPACE_NOT_FOUND",
            ErrorCode::WorkspaceAlreadyExists => "WORKSPACE_ALREADY_EXISTS",
            ErrorCode::WorkspaceInvalid => "WORKSPACE_INVALID",
            ErrorCode::NoteNotFound => "NOTE_NOT_FOUND",
            ErrorCode::NoteSaveFailed => "NOTE_SAVE_FAILED",
            ErrorCode::NoteConflict => "NOTE_CONFLICT",
            ErrorCode::CategoryNotFound => "CATEGORY_NOT_FOUND",
            ErrorCode::CategoryNameConflict => "CATEGORY_NAME_CONFLICT",
            ErrorCode::CategoryMoveInvalid => "CATEGORY_MOVE_INVALID",
            ErrorCode::TagNotFound => "TAG_NOT_FOUND",
            ErrorCode::FileAccessDenied => "FILE_ACCESS_DENIED",
            ErrorCode::PathInvalid => "PATH_INVALID",
            ErrorCode::DatabaseError => "DATABASE_ERROR",
            ErrorCode::SearchIndexError => "SEARCH_INDEX_ERROR",
            ErrorCode::BackupFailed => "BACKUP_FAILED",
            ErrorCode::ExportFailed => "EXPORT_FAILED",
            ErrorCode::IoError => "IO_ERROR",
            ErrorCode::InternalError => "INTERNAL_ERROR",
        }
    }
}

impl Serialize for ErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// 应用错误。
#[derive(Debug, Clone)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into(), details: None }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    pub fn not_initialized() -> Self {
        Self::new(ErrorCode::NotInitialized, "尚未打开任何工作区，请先创建或打开知识库。")
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for AppError {}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AppError", 3)?;
        state.serialize_field("code", &self.code)?;
        state.serialize_field("message", &self.message)?;
        match &self.details {
            Some(details) => state.serialize_field("details", details)?,
            None => state.serialize_field("details", &serde_json::Value::Null)?,
        }
        state.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        Self::new(ErrorCode::DatabaseError, format!("数据库操作失败：{err}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        let code = match err.kind() {
            std::io::ErrorKind::PermissionDenied => ErrorCode::FileAccessDenied,
            std::io::ErrorKind::NotFound => ErrorCode::IoError,
            _ => ErrorCode::IoError,
        };
        Self::new(code, format!("文件操作失败：{err}"))
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::new(ErrorCode::InternalError, format!("JSON 序列化失败：{err}"))
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::new(ErrorCode::InvalidInput, format!("Frontmatter 解析失败：{err}"))
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(err: zip::result::ZipError) -> Self {
        Self::new(ErrorCode::BackupFailed, format!("备份文件处理失败：{err}"))
    }
}

impl From<tauri::Error> for AppError {
    fn from(err: tauri::Error) -> Self {
        Self::new(ErrorCode::InternalError, format!("应用内核错误：{err}"))
    }
}
