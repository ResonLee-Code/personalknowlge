//! 同步配置、凭据与本地基线。
//!
//! 它们全部位于应用数据目录（工作区之外）：
//! - 凭据不会进入备份 / Markdown / 导出；
//! - state 保存上次同步基线与待重试队列，关闭同步不删除本地数据；
//! - 冲突历史也在这里，远端不可用时仍可查看本地与共同祖先版本。

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::hash::sha256_hex_str;
use crate::domain::sync::{SyncHistoryEntry, SyncSettings, UpdateSyncSettingsInput};

use crate::application::context::WorkspaceContext;

const STORE_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StoredSyncConfig {
    pub enabled: bool,
    pub provider: String,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub region: String,
    pub sync_notes: bool,
    pub sync_attachments: bool,
    pub sync_index: bool,
    pub device_id: String,
}

impl Default for StoredSyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "webdav".to_string(),
            endpoint: None,
            bucket: None,
            region: "us-east-1".to_string(),
            sync_notes: true,
            sync_attachments: true,
            sync_index: false,
            device_id: Uuid::new_v4().to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StoredSyncSecrets {
    pub username: Option<String>,
    pub password: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct BaseEntry {
    pub content_hash: Option<String>,
    pub object_id: Option<String>,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: String,
    pub path: String,
    /// `upload` / `delete`
    pub action: String,
    pub queued_at: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredConflict {
    pub id: String,
    pub path: String,
    pub kind: String,
    pub local_hash: Option<String>,
    pub remote_hash: Option<String>,
    pub base_hash: Option<String>,
    pub remote_object_id: Option<String>,
    pub base_object_id: Option<String>,
    pub detected_at: String,
    /// 相对本地同步历史目录的版本文件。
    pub local_archive: Option<String>,
    pub remote_archive: Option<String>,
    pub base_archive: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StoredSyncState {
    pub version: u8,
    pub workspace_id: String,
    pub base: BTreeMap<String, BaseEntry>,
    pub queue: Vec<QueueItem>,
    pub conflicts: Vec<StoredConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveMetadata {
    original_path: String,
    category: String,
    saved_at: String,
    size_bytes: u64,
}

pub struct SyncStore {
    ctx: Arc<WorkspaceContext>,
}

impl SyncStore {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    pub fn workspace_dir(&self) -> PathBuf {
        let suffix = sha256_hex_str(&self.ctx.workspace.id);
        self.ctx
            .data_dir
            .join("sync")
            .join(&suffix[..16.min(suffix.len())])
    }

    pub fn history_dir(&self) -> PathBuf {
        self.workspace_dir().join("history")
    }

    pub fn prefix(&self) -> String {
        // 远端目录由用户提供的 endpoint 决定，不能带本地 workspace id：
        // 两台设备会各自生成不同的 workspace id，否则永远无法看到同一份知识库。
        "pkos-sync/v2/library".to_string()
    }

    pub fn load_config(&self) -> StoredSyncConfig {
        read_json(&self.workspace_dir().join("config.json"))
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    pub fn save_config(&self, config: &StoredSyncConfig) -> Result<()> {
        write_json_private(&self.workspace_dir().join("config.json"), config)
    }

    pub fn load_secrets(&self) -> StoredSyncSecrets {
        read_json(&self.workspace_dir().join("credentials.json"))
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    pub fn save_secrets(&self, secrets: &StoredSyncSecrets) -> Result<()> {
        write_json_private(&self.workspace_dir().join("credentials.json"), secrets)
    }

    pub fn settings(&self) -> SyncSettings {
        let config = self.load_config();
        let secrets = self.load_secrets();
        SyncSettings {
            enabled: config.enabled,
            provider: config.provider,
            endpoint: config.endpoint,
            bucket: config.bucket,
            region: config.region,
            username_set: secret_present(&secrets.username),
            access_key_set: secret_present(&secrets.access_key),
            sync_notes: config.sync_notes,
            sync_attachments: config.sync_attachments,
            sync_index: config.sync_index,
            device_id: config.device_id,
            pending_queue: self
                .load_state()
                .map(|state| state.queue.len())
                .unwrap_or(0),
        }
    }

    pub fn update_settings(&self, input: UpdateSyncSettingsInput) -> Result<SyncSettings> {
        let mut config = self.load_config();
        let mut secrets = self.load_secrets();
        if let Some(enabled) = input.enabled {
            config.enabled = enabled;
        }
        if let Some(provider) = input.provider {
            let normalized = provider.trim().to_ascii_lowercase();
            if !matches!(normalized.as_str(), "webdav" | "s3") {
                return Err(AppError::invalid_input(
                    "同步类型只支持 WebDAV 或 S3 兼容存储。",
                ));
            }
            config.provider = normalized;
        }
        if let Some(endpoint) = input.endpoint {
            let trimmed = endpoint.trim().trim_end_matches('/').to_string();
            if !trimmed.is_empty() {
                validate_sync_endpoint(&trimmed)?;
            }
            config.endpoint = (!trimmed.is_empty()).then_some(trimmed);
        }
        if let Some(bucket) = input.bucket {
            let trimmed = bucket.trim().to_string();
            config.bucket = (!trimmed.is_empty()).then_some(trimmed);
        }
        if let Some(region) = input.region {
            let trimmed = region.trim().to_string();
            config.region = if trimmed.is_empty() {
                "us-east-1".to_string()
            } else {
                trimmed
            };
        }
        if let Some(notes) = input.sync_notes {
            config.sync_notes = notes;
        }
        if let Some(attachments) = input.sync_attachments {
            config.sync_attachments = attachments;
        }
        if let Some(index) = input.sync_index {
            config.sync_index = index;
        }
        if input.clear_credentials == Some(true) {
            secrets = StoredSyncSecrets::default();
        } else {
            replace_secret(&mut secrets.username, input.username);
            replace_secret(&mut secrets.password, input.password);
            replace_secret(&mut secrets.access_key, input.access_key);
            replace_secret(&mut secrets.secret_key, input.secret_key);
        }
        if !config.sync_notes && !config.sync_attachments && !config.sync_index {
            return Err(AppError::invalid_input(
                "至少选择笔记、附件或索引中的一项进行同步。",
            ));
        }
        self.save_config(&config)?;
        self.save_secrets(&secrets)?;
        Ok(self.settings())
    }

    pub fn load_state(&self) -> Result<StoredSyncState> {
        let path = self.workspace_dir().join("state.json");
        let mut state: StoredSyncState = read_json(&path)?.unwrap_or_else(|| StoredSyncState {
            version: STORE_VERSION,
            workspace_id: self.ctx.workspace.id.clone(),
            ..Default::default()
        });
        if state.workspace_id.is_empty() {
            state.workspace_id = self.ctx.workspace.id.clone();
        }
        if state.workspace_id != self.ctx.workspace.id {
            return Err(AppError::new(
                ErrorCode::SyncFailed,
                "同步状态属于另一个工作区；已拒绝载入，避免覆盖错误知识库。",
            ));
        }
        Ok(state)
    }

    pub fn save_state(&self, state: &StoredSyncState) -> Result<()> {
        write_json_private(&self.workspace_dir().join("state.json"), state)
    }

    pub fn archive_bytes(&self, category: &str, path: &str, bytes: &[u8]) -> Result<String> {
        let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%3f").to_string();
        let safe: String = path
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '-' | '_') {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let relative = format!(
            "{category}/{stamp}-{}-{}",
            &Uuid::new_v4().simple().to_string()[..8],
            safe.trim_start_matches('/')
        );
        let target = self.history_dir().join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        write_private(&target, bytes)?;
        write_json_private(
            &self.history_dir().join(format!("{relative}.meta.json")),
            &ArchiveMetadata {
                original_path: path.to_string(),
                category: category.to_string(),
                saved_at: stamp,
                size_bytes: bytes.len() as u64,
            },
        )?;
        Ok(relative)
    }

    pub fn read_archive(&self, relative: &str) -> Result<Vec<u8>> {
        let path = safe_join(&self.history_dir(), relative)?;
        fs::read(path).map_err(AppError::from)
    }

    pub fn history(&self) -> Result<Vec<SyncHistoryEntry>> {
        let root = self.history_dir();
        if !root.is_dir() {
            return Ok(Vec::new());
        }
        let mut entries = Vec::new();
        for item in WalkDir::new(&root).follow_links(false) {
            let Ok(item) = item else { continue };
            if !item.file_type().is_file() || item.path().to_string_lossy().ends_with(".meta.json")
            {
                continue;
            }
            let relative = item
                .path()
                .strip_prefix(&root)
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if relative.is_empty() || relative.contains("..") {
                continue;
            }
            let metadata =
                read_json::<ArchiveMetadata>(&root.join(format!("{relative}.meta.json")))
                    .ok()
                    .flatten();
            let fallback_category = relative.split('/').next().unwrap_or("history").to_string();
            entries.push(SyncHistoryEntry {
                original_path: metadata
                    .as_ref()
                    .map(|meta| meta.original_path.clone())
                    .unwrap_or_else(|| {
                        relative.rsplit('/').next().unwrap_or(&relative).to_string()
                    }),
                category: metadata
                    .as_ref()
                    .map(|meta| meta.category.clone())
                    .unwrap_or(fallback_category),
                saved_at: metadata
                    .as_ref()
                    .map(|meta| meta.saved_at.clone())
                    .or_else(|| {
                        item.metadata()
                            .ok()
                            .and_then(|meta| meta.modified().ok())
                            .map(crate::core::time::system_time_to_iso)
                    })
                    .unwrap_or_default(),
                size_bytes: metadata
                    .as_ref()
                    .map(|meta| meta.size_bytes)
                    .or_else(|| item.metadata().ok().map(|meta| meta.len()))
                    .unwrap_or(0),
                relative_path: relative,
            });
        }
        entries.sort_by(|left, right| {
            right
                .saved_at
                .cmp(&left.saved_at)
                .then_with(|| right.relative_path.cmp(&left.relative_path))
        });
        Ok(entries)
    }

    pub fn restore_history(&self, relative: &str) -> Result<String> {
        let metadata_path = self.history_dir().join(format!("{relative}.meta.json"));
        let metadata = read_json::<ArchiveMetadata>(&metadata_path)?
            .ok_or_else(|| AppError::invalid_input("同步历史缺少原始路径元数据，无法安全还原。"))?;
        let bytes = self.read_archive(relative)?;
        let target = self.ctx.paths.resolve_relative(&metadata.original_path)?;
        if target.exists() {
            let current = fs::read(&target)?;
            self.archive_bytes("before-restore", &metadata.original_path, &current)?;
        }
        self.ctx.storage.write_atomic_absolute(&target, &bytes)?;
        Ok(metadata.original_path)
    }
}

fn replace_secret(target: &mut Option<String>, input: Option<String>) {
    let Some(value) = input else { return };
    let trimmed = value.trim().to_string();
    *target = (!trimmed.is_empty()).then_some(trimmed);
}

fn secret_present(value: &Option<String>) -> bool {
    value
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
}

pub(super) fn validate_sync_endpoint(endpoint: &str) -> Result<()> {
    if !(endpoint.starts_with("https://") || endpoint.starts_with("http://")) {
        return Err(AppError::invalid_input(
            "同步 endpoint 必须以 http:// 或 https:// 开头。",
        ));
    }
    let rest = endpoint
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    let authority = rest.split('/').next().unwrap_or_default();
    if authority.is_empty() {
        return Err(AppError::invalid_input("同步 endpoint 缺少主机名。"));
    }
    if authority.contains('@') {
        return Err(AppError::invalid_input(
            "同步 endpoint 不能包含 userinfo；请使用独立的用户名 / 密码或 Access Key 字段。",
        ));
    }
    if endpoint
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(AppError::invalid_input(
            "同步 endpoint 不能包含空白字符或控制字符。",
        ));
    }
    if endpoint.contains('?') || endpoint.contains('#') {
        return Err(AppError::invalid_input(
            "同步 endpoint 不能包含查询串或片段；请只填写存储根地址。",
        ));
    }
    if endpoint.starts_with("http://") && !crate::core::net::is_loopback_target(endpoint) {
        return Err(AppError::invalid_input(
            "远程同步必须使用 HTTPS；明文 HTTP 只允许 localhost / 127.0.0.1 / ::1，\
             避免 WebDAV 密码或 S3 Access Key 在网络中裸传。",
        ));
    }
    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| AppError::new(ErrorCode::SyncFailed, format!("同步本地状态损坏：{error}")))
}

fn write_json_private<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    write_private(path, &bytes)
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("tmp-{}", Uuid::new_v4()));
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        Ok(())
    })();
    if let Err(error) = result {
        let _ = fs::remove_file(&temp);
        return Err(AppError::new(
            ErrorCode::SyncFailed,
            format!("写入同步本地状态失败：{error}"),
        ));
    }
    if let Err(error) = fs::rename(&temp, path) {
        // Windows 下 rename 不覆盖已有文件；先删旧文件再重命名。
        if path.exists() {
            if let Err(remove_error) = fs::remove_file(path) {
                let _ = fs::remove_file(&temp);
                return Err(AppError::new(
                    ErrorCode::SyncFailed,
                    format!("替换同步本地状态失败：{remove_error}"),
                ));
            }
            if let Err(retry_error) = fs::rename(&temp, path) {
                let _ = fs::remove_file(&temp);
                return Err(AppError::new(
                    ErrorCode::SyncFailed,
                    format!("替换同步本地状态失败：{retry_error}"),
                ));
            }
        } else {
            let _ = fs::remove_file(&temp);
            return Err(AppError::new(
                ErrorCode::SyncFailed,
                format!("替换同步本地状态失败：{error}"),
            ));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.contains("..") || relative.starts_with('/') || relative.contains('\\') {
        return Err(AppError::invalid_input("同步历史路径无效。"));
    }
    Ok(root.join(relative))
}
