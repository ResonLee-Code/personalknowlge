//! 可选加密同步（V2.0.x P1）。
//!
//! 同步只面向用户自己的 WebDAV / S3 兼容存储。凭据、缓存与冲突版本留在应用数据目录，
//! 不进入工作区；远端对象名不可读，正文与 manifest 均加密。

use serde::{Deserialize, Serialize};

/// 同步配置返回给界面的部分：不含任何密码或密钥。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSettings {
    pub enabled: bool,
    /// `webdav` / `s3`
    pub provider: String,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub region: String,
    pub username_set: bool,
    pub access_key_set: bool,
    pub sync_notes: bool,
    pub sync_attachments: bool,
    pub sync_index: bool,
    pub device_id: String,
    /// 上次因网络/认证失败仍待重试的本地变更数。
    pub pending_queue: usize,
}

/// 更新同步配置。空秘密字段表示保留旧值，`clearCredentials` 才清除。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSyncSettingsInput {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub bucket: Option<String>,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub access_key: Option<String>,
    #[serde(default)]
    pub secret_key: Option<String>,
    #[serde(default)]
    pub clear_credentials: Option<bool>,
    #[serde(default)]
    pub sync_notes: Option<bool>,
    #[serde(default)]
    pub sync_attachments: Option<bool>,
    #[serde(default)]
    pub sync_index: Option<bool>,
}

/// 每次同步必须显式携带口令；口令只用于派生本轮 AES Key，不落盘。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRunInput {
    pub passphrase: String,
    /// 真正写入 / 下载前必须确认预览。
    #[serde(default)]
    pub confirm: bool,
}

/// 计划中的一项操作。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlanItem {
    /// `upload` / `download` / `deleteLocal` / `deleteRemote` / `conflict`
    pub kind: String,
    pub path: String,
    pub size_bytes: u64,
    pub reason: String,
}

/// dry-run 结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPreview {
    pub uploads: Vec<SyncPlanItem>,
    pub downloads: Vec<SyncPlanItem>,
    pub local_deletes: Vec<SyncPlanItem>,
    pub remote_deletes: Vec<SyncPlanItem>,
    pub conflicts: Vec<SyncPlanItem>,
    pub total_bytes: u64,
    pub pending_queue: usize,
    pub warnings: Vec<String>,
}

/// 真正同步后的计数。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRunReport {
    pub preview: SyncPreview,
    pub uploaded: usize,
    pub downloaded: usize,
    pub local_deleted: usize,
    pub remote_deleted: usize,
    pub conflicts_created: usize,
    pub queued: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictSummary {
    pub id: String,
    pub path: String,
    /// `bothModified` / `modifyDelete` / `bothAdded`
    pub kind: String,
    pub local_hash: Option<String>,
    pub remote_hash: Option<String>,
    pub base_hash: Option<String>,
    pub detected_at: String,
    pub can_merge_text: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncConflictDetail {
    pub summary: SyncConflictSummary,
    pub local_content: Option<String>,
    pub remote_content: Option<String>,
    pub base_content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMergePreview {
    pub merged_content: Option<String>,
    pub conflict_count: usize,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncHistoryEntry {
    pub relative_path: String,
    pub original_path: String,
    /// `local` / `remote` / `base` / `before-write` / `before-delete`
    pub category: String,
    pub saved_at: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreSyncHistoryInput {
    pub relative_path: String,
    pub confirm: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveSyncConflictInput {
    pub conflict_id: String,
    /// `local` / `remote` / `merged`
    pub resolution: String,
    #[serde(default)]
    pub merged_content: Option<String>,
}
