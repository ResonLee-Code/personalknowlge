//! 备份与导出模型（设计文档 20）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub file_name: String,
    pub path: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub note_count: i64,
    pub attachment_count: i64,
    pub includes_database: bool,
}

/// 单项校验结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupValidation {
    pub valid: bool,
    pub checks: Vec<BackupCheck>,
    pub note_count: i64,
    pub attachment_count: i64,
    pub created_at: Option<String>,
    pub workspace_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCreateInput {
    /// 备份输出目录；缺省为 `.knowledge/backups`
    #[serde(default)]
    pub target_dir: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupInput {
    pub archive_path: String,
    /// 恢复到该目录（必须不存在或为空目录）
    pub target_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportMarkdownInput {
    pub target_dir: String,
    /// 是否包含附件目录
    #[serde(default)]
    pub include_attachments: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub target_dir: String,
    pub note_count: i64,
    pub attachment_count: i64,
}
