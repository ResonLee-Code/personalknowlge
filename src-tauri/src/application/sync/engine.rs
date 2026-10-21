//! 加密同步引擎：文件扫描、dry-run、对象传输、manifest 并发保护与冲突留档。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::application::context::WorkspaceContext;
use crate::application::index_service::IndexService;
use crate::core::base64;
use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::hash::{sha256_hex, sha256_hex_str};
use crate::core::time::now_iso;
use crate::domain::note::NoteStatus;
use crate::domain::sync::{
    ResolveSyncConflictInput, RestoreSyncHistoryInput, SyncConflictDetail, SyncConflictSummary,
    SyncHistoryEntry, SyncMergePreview, SyncPlanItem, SyncPreview, SyncRunInput, SyncRunReport,
    SyncSettings, UpdateSyncSettingsInput,
};
use crate::repositories::{NoteRepository, SqliteNoteRepository};

use super::crypto;
use super::store::{
    BaseEntry, QueueItem, StoredConflict, StoredSyncConfig, StoredSyncSecrets, StoredSyncState,
    SyncStore,
};
use super::transport::{S3Transport, SyncTransport, WebDavTransport};

const MANIFEST_VERSION: u8 = 1;
const PROFILE_VERSION: u8 = 1;
const INDEX_SYNC_PATH: &str = ".knowledge/sync/index-cache.json";
static SYNC_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteProfile {
    version: u8,
    kdf: String,
    salt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteEntry {
    path: String,
    object_id: Option<String>,
    content_hash: Option<String>,
    size: u64,
    deleted: bool,
    updated_at: String,
    device_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteManifest {
    version: u8,
    revision: u64,
    generated_at: String,
    device_id: String,
    entries: BTreeMap<String, RemoteEntry>,
}

impl RemoteManifest {
    fn empty(config: &StoredSyncConfig) -> Self {
        Self {
            version: MANIFEST_VERSION,
            revision: 0,
            generated_at: now_iso(),
            device_id: config.device_id.clone(),
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct LocalMeta {
    hash: String,
    size: u64,
}

struct RemoteSession {
    transport: Box<dyn SyncTransport>,
    key: [u8; 32],
    manifest: RemoteManifest,
    manifest_existed: bool,
    manifest_etag: Option<String>,
}

#[derive(Debug, Clone)]
enum PlannedAction {
    Noop,
    Upload,
    Download,
    DeleteRemote,
    DeleteLocal,
    Conflict { kind: String },
}

#[derive(Debug, Clone)]
struct PlannedChange {
    path: String,
    action: PlannedAction,
    local: Option<LocalMeta>,
    remote: Option<RemoteEntry>,
    base: Option<BaseEntry>,
    size: u64,
    reason: String,
}

pub struct SyncService {
    ctx: Arc<WorkspaceContext>,
    store: SyncStore,
}

impl SyncService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self {
            store: SyncStore::new(Arc::clone(&ctx)),
            ctx,
        }
    }

    pub fn preview(&self, passphrase: &str) -> Result<SyncPreview> {
        let _guard = SYNC_LOCK
            .lock()
            .map_err(|_| AppError::internal("同步任务锁异常。"))?;
        self.preview_inner(passphrase)
    }

    pub fn settings(&self) -> SyncSettings {
        self.store.settings()
    }

    pub fn update_settings(&self, input: UpdateSyncSettingsInput) -> Result<SyncSettings> {
        self.store.update_settings(input)
    }

    pub fn run(&self, input: SyncRunInput) -> Result<SyncRunReport> {
        if !input.confirm {
            return Err(AppError::invalid_input(
                "同步写入前必须确认 dry-run；本次没有上传或下载任何文件。",
            ));
        }
        let started = Instant::now();
        let _guard = SYNC_LOCK
            .lock()
            .map_err(|_| AppError::internal("同步任务锁异常。"))?;
        let config = self.ready_config()?;
        let secrets = self.ready_secrets(&config)?;
        let mut state = self.store.load_state()?;
        let local = self.scan_local(&config)?;
        let session = match self.open_remote(&config, &secrets, &input.passphrase) {
            Ok(session) => session,
            Err(error) => {
                if error.code == ErrorCode::SyncFailed {
                    queue_offline_changes(&mut state, &config, &local);
                    let _ = self.store.save_state(&state);
                }
                return Err(error);
            }
        };
        let changes = plan_changes(&config, &state, &local, &session.manifest);
        let preview = summarize(&changes, state.queue.len());
        if !changes.iter().any(|change| {
            matches!(
                change.action,
                PlannedAction::Upload
                    | PlannedAction::Download
                    | PlannedAction::DeleteRemote
                    | PlannedAction::DeleteLocal
                    | PlannedAction::Conflict { .. }
            )
        }) {
            state.queue.clear();
            self.store.save_state(&state)?;
            return Ok(SyncRunReport {
                preview,
                uploaded: 0,
                downloaded: 0,
                local_deleted: 0,
                remote_deleted: 0,
                conflicts_created: 0,
                queued: 0,
                duration_ms: started.elapsed().as_millis() as u64,
            });
        }

        queue_changes(&mut state, &changes);
        self.store.save_state(&state)?;
        let result = self.apply_changes(
            &config,
            &secrets,
            &input.passphrase,
            &mut state,
            session,
            &local,
            &changes,
        );
        if let Err(error) = &result {
            mark_queue_error(&mut state, &changes, &error.message);
            let _ = self.store.save_state(&state);
        }
        let (uploaded, downloaded, local_deleted, remote_deleted, conflicts_created) = result?;
        clear_queue(&mut state, &changes);
        self.store.save_state(&state)?;
        Ok(SyncRunReport {
            preview,
            uploaded,
            downloaded,
            local_deleted,
            remote_deleted,
            conflicts_created,
            queued: state.queue.len(),
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    pub fn conflicts(&self) -> Result<Vec<SyncConflictSummary>> {
        Ok(self
            .store
            .load_state()?
            .conflicts
            .into_iter()
            .map(summary_of)
            .collect())
    }

    pub fn history(&self) -> Result<Vec<SyncHistoryEntry>> {
        self.store.history()
    }

    pub fn restore_history(&self, input: RestoreSyncHistoryInput) -> Result<String> {
        if !input.confirm {
            return Err(AppError::invalid_input(
                "还原同步历史前必须确认；本次没有覆盖任何文件。",
            ));
        }
        let _guard = SYNC_LOCK
            .lock()
            .map_err(|_| AppError::internal("同步任务锁异常。"))?;
        let path = self.store.restore_history(&input.relative_path)?;
        if path.ends_with(".md") {
            self.refresh_index();
        }
        Ok(path)
    }

    pub fn conflict_detail(&self, id: &str) -> Result<SyncConflictDetail> {
        let state = self.store.load_state()?;
        let conflict = state
            .conflicts
            .iter()
            .find(|conflict| conflict.id == id)
            .ok_or_else(|| AppError::invalid_input("同步冲突不存在或已经解决。"))?;
        Ok(SyncConflictDetail {
            summary: summary_of(conflict.clone()),
            local_content: self.read_archive_text(conflict.local_archive.as_deref()),
            remote_content: self.read_archive_text(conflict.remote_archive.as_deref()),
            base_content: self.read_archive_text(conflict.base_archive.as_deref()),
        })
    }

    pub fn merge_preview(&self, id: &str) -> Result<SyncMergePreview> {
        let detail = self.conflict_detail(id)?;
        if !detail.summary.can_merge_text {
            return Ok(SyncMergePreview {
                merged_content: None,
                conflict_count: 1,
                note: "附件是二进制文件，不执行逐行合并；请选择本地或远端版本。".to_string(),
            });
        }
        let (Some(base), Some(local), Some(remote)) = (
            detail.base_content.as_deref(),
            detail.local_content.as_deref(),
            detail.remote_content.as_deref(),
        ) else {
            return Ok(SyncMergePreview {
                merged_content: None,
                conflict_count: 1,
                note: "缺少本地、远端或共同祖本文本，无法自动三方合并。".to_string(),
            });
        };
        // 三路合并只比较正文；frontmatter 的 updatedAt 等元信息天然不同，
        // 把它混进 diff 会把所有修改误判为同一处冲突。合并后采用本地元信息。
        let local_parsed = crate::infrastructure::frontmatter::parse(local);
        let base_parsed = crate::infrastructure::frontmatter::parse(base);
        let remote_parsed = crate::infrastructure::frontmatter::parse(remote);
        let (merged_body, conflicts) =
            super::merge::merge_lines(&base_parsed.body, &local_parsed.body, &remote_parsed.body);
        let merged = merged_body.map(|body| {
            crate::infrastructure::frontmatter::serialize(&local_parsed.frontmatter, &body)
        });
        Ok(SyncMergePreview {
            note: if conflicts == 0 {
                "自动合并只拼接了互不重叠的行修改；保存前仍请人工核对。".to_string()
            } else {
                format!("有 {conflicts} 处修改重叠，不能安全自动合并。")
            },
            merged_content: merged,
            conflict_count: conflicts,
        })
    }

    pub fn resolve_conflict(&self, input: ResolveSyncConflictInput) -> Result<()> {
        let _guard = SYNC_LOCK
            .lock()
            .map_err(|_| AppError::internal("同步任务锁异常。"))?;
        let mut state = self.store.load_state()?;
        let conflict = state
            .conflicts
            .iter()
            .find(|conflict| conflict.id == input.conflict_id)
            .cloned()
            .ok_or_else(|| AppError::invalid_input("同步冲突不存在或已经解决。"))?;
        let path = self.ctx.paths.resolve_relative(&conflict.path)?;
        let current_hash = fs::read(&path).ok().map(|bytes| sha256_hex(&bytes));
        if current_hash != conflict.local_hash {
            return Err(AppError::new(
                ErrorCode::SyncConflict,
                "本地文件在冲突生成后又被修改；请重新同步并生成新的冲突。",
            ));
        }

        match input.resolution.as_str() {
            "local" => {
                if conflict.local_hash.is_none() {
                    self.archive_and_remove(&conflict.path)?;
                }
            }
            "remote" => {
                if let Some(relative) = conflict.remote_archive.as_deref() {
                    let bytes = self.store.read_archive(relative)?;
                    self.write_local(&conflict.path, &bytes)?;
                } else {
                    self.archive_and_remove(&conflict.path)?;
                }
            }
            "merged" => {
                if !conflict.path.ends_with(".md") {
                    return Err(AppError::invalid_input(
                        "附件冲突不能使用文本合并，请选择本地或远端版本。",
                    ));
                }
                let merged = input.merged_content.ok_or_else(|| {
                    AppError::invalid_input("选择三方合并时必须提供合并后的正文。")
                })?;
                if merged.len() > 8 * 1024 * 1024 {
                    return Err(AppError::invalid_input(
                        "合并后的正文过大，请手动拆分后再同步。",
                    ));
                }
                self.write_local(&conflict.path, merged.as_bytes())?;
            }
            _ => {
                return Err(AppError::invalid_input(
                    "冲突处理方式只能是 local、remote 或 merged。",
                ))
            }
        }
        state.conflicts.retain(|item| item.id != conflict.id);
        self.store.save_state(&state)?;
        if conflict.path.ends_with(".md") {
            self.refresh_index();
        }
        Ok(())
    }

    fn preview_inner(&self, passphrase: &str) -> Result<SyncPreview> {
        let config = self.ready_config()?;
        let secrets = self.ready_secrets(&config)?;
        let state = self.store.load_state()?;
        let session = self.open_remote(&config, &secrets, passphrase)?;
        let local = self.scan_local(&config)?;
        let changes = plan_changes(&config, &state, &local, &session.manifest);
        Ok(summarize(&changes, state.queue.len()))
    }

    fn ready_config(&self) -> Result<StoredSyncConfig> {
        let config = self.store.load_config();
        if !config.enabled {
            return Err(AppError::invalid_input("同步未开启；本地功能不受影响。"));
        }
        if config.endpoint.as_deref().is_none_or(str::is_empty) {
            return Err(AppError::invalid_input("请先填写 WebDAV / S3 endpoint。"));
        }
        super::store::validate_sync_endpoint(config.endpoint.as_deref().unwrap_or_default())?;
        if config.provider == "s3" && config.bucket.as_deref().is_none_or(str::is_empty) {
            return Err(AppError::invalid_input("S3 兼容存储必须填写 bucket。"));
        }
        Ok(config)
    }

    fn ready_secrets(&self, config: &StoredSyncConfig) -> Result<StoredSyncSecrets> {
        let secrets = self.store.load_secrets();
        if config.provider == "webdav" {
            if !present(&secrets.username) || !present(&secrets.password) {
                return Err(AppError::invalid_input(
                    "WebDAV 需要用户名与密码；凭据只保存在应用数据目录。",
                ));
            }
        } else if !present(&secrets.access_key) || !present(&secrets.secret_key) {
            return Err(AppError::invalid_input(
                "S3 兼容存储需要 Access Key 与 Secret Key；凭据只保存在应用数据目录。",
            ));
        }
        Ok(secrets)
    }

    fn transport(
        &self,
        config: &StoredSyncConfig,
        secrets: &StoredSyncSecrets,
    ) -> Result<Box<dyn SyncTransport>> {
        let endpoint = config
            .endpoint
            .as_deref()
            .ok_or_else(|| AppError::invalid_input("同步 endpoint 为空。"))?;
        if config.provider == "webdav" {
            Ok(Box::new(WebDavTransport::new(
                endpoint,
                secrets.username.as_deref().unwrap_or_default(),
                secrets.password.as_deref().unwrap_or_default(),
            )))
        } else {
            Ok(Box::new(S3Transport::new(
                endpoint,
                config.bucket.as_deref().unwrap_or_default(),
                &config.region,
                secrets.access_key.as_deref().unwrap_or_default(),
                secrets.secret_key.as_deref().unwrap_or_default(),
            )))
        }
    }

    fn open_remote(
        &self,
        config: &StoredSyncConfig,
        secrets: &StoredSyncSecrets,
        passphrase: &str,
    ) -> Result<RemoteSession> {
        let transport = self.transport(config, secrets)?;
        let prefix = self.store.prefix();
        let profile_key = format!("{prefix}/profile.json");
        let profile = match transport.get(&profile_key)? {
            Some(object) => parse_profile(&object.bytes)?,
            None => {
                let generated = RemoteProfile {
                    version: PROFILE_VERSION,
                    kdf: "argon2id".to_string(),
                    salt: base64::encode(Uuid::new_v4().as_bytes()),
                };
                let bytes = serde_json::to_vec_pretty(&generated)?;
                match transport.put(&profile_key, &bytes, None, true) {
                    Ok(_) => {}
                    // 另一台设备先创建了 profile：随后读取它的 salt，双方会收敛到同一密钥。
                    Err(error) if error.code == ErrorCode::SyncConflict => {}
                    Err(error) => return Err(error),
                }
                // 首次两台设备同时创建时，以随后读到的 profile 为准。
                let object = transport.get(&profile_key)?.ok_or_else(|| {
                    AppError::new(ErrorCode::SyncFailed, "远端同步 profile 创建后无法读取。")
                })?;
                parse_profile(&object.bytes)?
            }
        };
        if profile.version != PROFILE_VERSION || profile.kdf != "argon2id" {
            return Err(AppError::new(
                ErrorCode::SyncFailed,
                "远端同步 profile 使用了不支持的协议或 KDF。",
            ));
        }
        let salt = base64::decode(&profile.salt)?;
        let key = crypto::derive_key(passphrase, &salt)?;
        let manifest_key = format!("{prefix}/manifest.enc");
        let (manifest, manifest_existed, manifest_etag) = match transport.get(&manifest_key)? {
            Some(object) => {
                let plain = crypto::decrypt(&key, manifest_aad(&prefix).as_bytes(), &object.bytes)?;
                let manifest: RemoteManifest = serde_json::from_slice(&plain).map_err(|_| {
                    AppError::new(ErrorCode::SyncFailed, "远端同步 manifest 不是合法 JSON。")
                })?;
                if manifest.version != MANIFEST_VERSION {
                    return Err(AppError::new(
                        ErrorCode::SyncFailed,
                        format!("不支持的远端 manifest 版本：{}", manifest.version),
                    ));
                }
                (manifest, true, object.etag)
            }
            None => (RemoteManifest::empty(config), false, None),
        };
        Ok(RemoteSession {
            transport,
            key,
            manifest,
            manifest_existed,
            manifest_etag,
        })
    }

    fn scan_local(&self, config: &StoredSyncConfig) -> Result<BTreeMap<String, LocalMeta>> {
        let mut local = BTreeMap::new();
        if config.sync_notes {
            let notes = self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.list_by_status(
                    conn,
                    &self.ctx.workspace.id,
                    NoteStatus::Active,
                )
            })?;
            for note in notes {
                let path = self.ctx.paths.resolve_relative(&note.file_path)?;
                let Ok(bytes) = fs::read(path) else {
                    continue;
                };
                local.insert(
                    note.file_path,
                    LocalMeta {
                        hash: sha256_hex(&bytes),
                        size: bytes.len() as u64,
                    },
                );
            }
        }
        if config.sync_attachments {
            let root = self.ctx.paths.attachments_dir();
            if root.is_dir() {
                for entry in WalkDir::new(&root).follow_links(false) {
                    let Ok(entry) = entry else { continue };
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    let Ok(relative) = entry.path().strip_prefix(self.ctx.paths.root()) else {
                        continue;
                    };
                    let relative =
                        crate::core::paths::normalize_relative(&relative.to_string_lossy());
                    let bytes = fs::read(entry.path())?;
                    local.insert(
                        relative,
                        LocalMeta {
                            hash: sha256_hex(&bytes),
                            size: bytes.len() as u64,
                        },
                    );
                }
            }
        }
        if config.sync_index {
            let bytes = self.index_cache_bytes()?;
            local.insert(
                INDEX_SYNC_PATH.to_string(),
                LocalMeta {
                    hash: sha256_hex(&bytes),
                    size: bytes.len() as u64,
                },
            );
        }
        Ok(local)
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_changes(
        &self,
        config: &StoredSyncConfig,
        _secrets: &StoredSyncSecrets,
        _passphrase: &str,
        state: &mut StoredSyncState,
        session: RemoteSession,
        local: &BTreeMap<String, LocalMeta>,
        changes: &[PlannedChange],
    ) -> Result<(usize, usize, usize, usize, usize)> {
        let prefix = self.store.prefix();
        let mut entries = session.manifest.entries.clone();
        let mut downloaded_payloads: BTreeMap<String, Option<Vec<u8>>> = BTreeMap::new();
        let mut uploaded = 0usize;
        let mut downloaded = 0usize;
        let mut local_deleted = 0usize;
        let mut remote_deleted = 0usize;
        let mut conflicts_created = 0usize;

        for change in changes {
            match &change.action {
                PlannedAction::Upload => {
                    let bytes = self.read_local(&change.path, config)?;
                    let expected = local.get(&change.path).ok_or_else(|| {
                        AppError::new(
                            ErrorCode::SyncConflict,
                            format!("本地文件在同步开始后消失：{}", change.path),
                        )
                    })?;
                    if sha256_hex(&bytes) != expected.hash {
                        return Err(AppError::new(
                            ErrorCode::SyncConflict,
                            format!("本地文件在同步开始后发生变化：{}", change.path),
                        ));
                    }
                    let object_id = object_id(&change.path, &expected.hash);
                    let encrypted = crypto::encrypt(
                        &session.key,
                        object_aad(&prefix, &object_id).as_bytes(),
                        &bytes,
                    )?;
                    session.transport.put(
                        &format!("{prefix}/{object_id}"),
                        &encrypted,
                        None,
                        false,
                    )?;
                    entries.insert(
                        change.path.clone(),
                        RemoteEntry {
                            path: change.path.clone(),
                            object_id: Some(object_id),
                            content_hash: Some(expected.hash.clone()),
                            size: expected.size,
                            deleted: false,
                            updated_at: now_iso(),
                            device_id: config.device_id.clone(),
                        },
                    );
                    uploaded += 1;
                }
                PlannedAction::DeleteRemote => {
                    let old_hash = change
                        .base
                        .as_ref()
                        .and_then(|base| base.content_hash.clone());
                    entries.insert(
                        change.path.clone(),
                        RemoteEntry {
                            path: change.path.clone(),
                            object_id: None,
                            content_hash: old_hash,
                            size: 0,
                            deleted: true,
                            updated_at: now_iso(),
                            device_id: config.device_id.clone(),
                        },
                    );
                    remote_deleted += 1;
                }
                PlannedAction::Download => {
                    let Some(remote) = change.remote.as_ref() else {
                        continue;
                    };
                    let bytes = self.fetch_remote_object(&session, remote, &prefix)?;
                    downloaded_payloads.insert(change.path.clone(), Some(bytes));
                }
                PlannedAction::DeleteLocal => {
                    downloaded_payloads.insert(change.path.clone(), None);
                }
                PlannedAction::Conflict { kind } => {
                    let remote_bytes = match change.remote.as_ref() {
                        Some(remote) if !remote.deleted => {
                            Some(self.fetch_remote_object(&session, remote, &prefix)?)
                        }
                        _ => None,
                    };
                    let base_bytes = match change.base.as_ref().and_then(|base| {
                        base.object_id
                            .as_deref()
                            .map(|object_id| (object_id, base.content_hash.as_deref()))
                    }) {
                        Some((object_id, hash)) => self
                            .fetch_object_by_id(&session, object_id, hash, &prefix)
                            .ok(),
                        None => None,
                    };
                    let local_bytes = self.read_local_optional(&change.path)?;
                    let local_archive = local_bytes
                        .as_deref()
                        .map(|bytes| self.store.archive_bytes("local", &change.path, bytes))
                        .transpose()?;
                    let remote_archive = remote_bytes
                        .as_deref()
                        .map(|bytes| self.store.archive_bytes("remote", &change.path, bytes))
                        .transpose()?;
                    let base_archive = base_bytes
                        .as_deref()
                        .map(|bytes| self.store.archive_bytes("base", &change.path, bytes))
                        .transpose()?;
                    state.conflicts.retain(|item| item.path != change.path);
                    state.conflicts.push(StoredConflict {
                        id: format!("conflict_{}", Uuid::new_v4()),
                        path: change.path.clone(),
                        kind: kind.clone(),
                        local_hash: change.local.as_ref().map(|item| item.hash.clone()),
                        remote_hash: change
                            .remote
                            .as_ref()
                            .and_then(|item| item.content_hash.clone()),
                        base_hash: change
                            .base
                            .as_ref()
                            .and_then(|item| item.content_hash.clone()),
                        remote_object_id: change
                            .remote
                            .as_ref()
                            .and_then(|item| item.object_id.clone()),
                        base_object_id: change
                            .base
                            .as_ref()
                            .and_then(|item| item.object_id.clone()),
                        detected_at: now_iso(),
                        local_archive,
                        remote_archive,
                        base_archive,
                    });
                    conflicts_created += 1;
                }
                PlannedAction::Noop => {}
            }
        }

        let mut manifest = session.manifest.clone();
        manifest.revision = manifest.revision.saturating_add(1);
        manifest.generated_at = now_iso();
        manifest.device_id = config.device_id.clone();
        manifest.entries = entries;
        let plain = serde_json::to_vec(&manifest)?;
        let encrypted = crypto::encrypt(&session.key, manifest_aad(&prefix).as_bytes(), &plain)?;
        if session.manifest_existed && session.manifest_etag.is_none() {
            return Err(AppError::new(
                ErrorCode::SyncFailed,
                "远端服务没有返回 manifest ETag，无法安全检测并发更新；本次未覆盖 manifest。",
            ));
        }
        session.transport.put(
            &format!("{prefix}/manifest.enc"),
            &encrypted,
            session.manifest_etag.as_deref(),
            !session.manifest_existed,
        )?;

        for (path, bytes) in downloaded_payloads {
            if let Some(bytes) = bytes {
                self.write_local(&path, &bytes)?;
                downloaded += 1;
            } else {
                if self.archive_and_remove(&path)? {
                    local_deleted += 1;
                }
            }
        }

        state.base = manifest
            .entries
            .iter()
            .filter(|(path, _)| selected_path(path, config))
            .map(|(path, entry)| {
                (
                    path.clone(),
                    BaseEntry {
                        content_hash: entry.content_hash.clone(),
                        object_id: entry.object_id.clone(),
                        deleted: entry.deleted,
                    },
                )
            })
            .collect();
        let notes_changed = changes.iter().any(|change| {
            change.path.ends_with(".md")
                && matches!(
                    change.action,
                    PlannedAction::Download | PlannedAction::DeleteLocal
                )
        });
        if notes_changed {
            self.refresh_index();
        }
        Ok((
            uploaded,
            downloaded,
            local_deleted,
            remote_deleted,
            conflicts_created,
        ))
    }

    fn fetch_remote_object(
        &self,
        session: &RemoteSession,
        entry: &RemoteEntry,
        prefix: &str,
    ) -> Result<Vec<u8>> {
        let object_id = entry
            .object_id
            .as_deref()
            .ok_or_else(|| AppError::new(ErrorCode::SyncFailed, "远端文件记录缺少 object id。"))?;
        self.fetch_object_by_id(session, object_id, entry.content_hash.as_deref(), prefix)
    }

    fn fetch_object_by_id(
        &self,
        session: &RemoteSession,
        object_id: &str,
        expected_hash: Option<&str>,
        prefix: &str,
    ) -> Result<Vec<u8>> {
        let object = session
            .transport
            .get(&format!("{prefix}/{object_id}"))?
            .ok_or_else(|| AppError::new(ErrorCode::SyncFailed, "远端文件对象缺失。"))?;
        let plain = crypto::decrypt(
            &session.key,
            object_aad(prefix, object_id).as_bytes(),
            &object.bytes,
        )?;
        if expected_hash.is_some_and(|hash| sha256_hex(&plain) != hash) {
            return Err(AppError::new(
                ErrorCode::SyncFailed,
                "远端文件解密后内容哈希与 manifest 不一致。",
            ));
        }
        Ok(plain)
    }

    fn read_local(&self, path: &str, config: &StoredSyncConfig) -> Result<Vec<u8>> {
        if path == INDEX_SYNC_PATH && config.sync_index {
            return self.index_cache_bytes();
        }
        let absolute = self.ctx.paths.resolve_relative(path)?;
        fs::read(absolute).map_err(AppError::from)
    }

    fn read_local_optional(&self, path: &str) -> Result<Option<Vec<u8>>> {
        let absolute = self.ctx.paths.resolve_relative(path)?;
        if !absolute.exists() {
            return Ok(None);
        }
        fs::read(absolute).map(Some).map_err(AppError::from)
    }

    fn write_local(&self, path: &str, bytes: &[u8]) -> Result<()> {
        let absolute = self.ctx.paths.resolve_relative(path)?;
        if absolute.exists() {
            let old = fs::read(&absolute)?;
            self.store.archive_bytes("before-write", path, &old)?;
        }
        self.ctx.storage.write_atomic_absolute(&absolute, bytes)
    }

    fn archive_and_remove(&self, path: &str) -> Result<bool> {
        let absolute = self.ctx.paths.resolve_relative(path)?;
        if !absolute.exists() {
            return Ok(false);
        }
        let bytes = fs::read(&absolute)?;
        self.store.archive_bytes("before-delete", path, &bytes)?;
        fs::remove_file(absolute)?;
        Ok(true)
    }

    fn index_cache_bytes(&self) -> Result<Vec<u8>> {
        let notes = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list_by_status(conn, &self.ctx.workspace.id, NoteStatus::Active)
        })?;
        let generated_at = notes
            .iter()
            .map(|note| note.updated_at.as_str())
            .max()
            .unwrap_or_default()
            .to_string();
        let entries: Vec<serde_json::Value> = notes
            .into_iter()
            .map(|note| {
                serde_json::json!({
                    "id": note.id,
                    "path": note.file_path,
                    "contentHash": note.content_hash,
                    "updatedAt": note.updated_at,
                })
            })
            .collect();
        Ok(serde_json::to_vec_pretty(&serde_json::json!({
            "version": 1,
            "generatedAt": generated_at,
            "notes": entries,
        }))?)
    }

    fn refresh_index(&self) {
        if let Err(error) = IndexService::new(Arc::clone(&self.ctx)).scan_workspace() {
            tracing::warn!(target: "sync", "同步完成后刷新索引失败：{}", error.message);
        }
    }

    fn read_archive_text(&self, relative: Option<&str>) -> Option<String> {
        let relative = relative?;
        let bytes = self.store.read_archive(relative).ok()?;
        String::from_utf8(bytes).ok()
    }
}

fn parse_profile(bytes: &[u8]) -> Result<RemoteProfile> {
    serde_json::from_slice(bytes)
        .map_err(|_| AppError::new(ErrorCode::SyncFailed, "远端 sync profile 不是合法 JSON。"))
}

fn plan_changes(
    config: &StoredSyncConfig,
    state: &StoredSyncState,
    local: &BTreeMap<String, LocalMeta>,
    manifest: &RemoteManifest,
) -> Vec<PlannedChange> {
    let paths: BTreeSet<String> = state
        .base
        .keys()
        .chain(local.keys())
        .chain(manifest.entries.keys())
        .filter(|path| selected_path(path, config))
        .cloned()
        .collect();
    paths
        .into_iter()
        .filter_map(|path| {
            let base = state.base.get(&path).cloned();
            let current = local.get(&path).cloned();
            let remote = manifest.entries.get(&path).cloned();
            let (action, reason) = if path == INDEX_SYNC_PATH {
                classify_index_cache(current.as_ref(), remote.as_ref())
            } else {
                classify(base.as_ref(), current.as_ref(), remote.as_ref())
            };
            (!matches!(action, PlannedAction::Noop)).then_some(PlannedChange {
                path,
                size: current
                    .as_ref()
                    .map(|item| item.size)
                    .or_else(|| {
                        remote
                            .as_ref()
                            .filter(|item| !item.deleted)
                            .map(|item| item.size)
                    })
                    .unwrap_or(0),
                action,
                local: current,
                remote,
                base,
                reason,
            })
        })
        .collect()
}

/// 索引缓存是派生数据，不是事实来源。只要远端有可用快照就采用远端，
/// 避免两台设备的本地缓存生成时间不同被误报为用户内容冲突。
fn classify_index_cache(
    local: Option<&LocalMeta>,
    remote: Option<&RemoteEntry>,
) -> (PlannedAction, String) {
    match remote {
        Some(entry) if !entry.deleted => {
            if local.map(|item| item.hash.as_str()) == entry.content_hash.as_deref() {
                (PlannedAction::Noop, "索引缓存一致".to_string())
            } else {
                (PlannedAction::Download, "采用远端派生索引缓存".to_string())
            }
        }
        Some(entry) if entry.deleted => {
            if local.is_some() {
                (
                    PlannedAction::DeleteLocal,
                    "远端已删除可选索引缓存".to_string(),
                )
            } else {
                (PlannedAction::Noop, "两边均无索引缓存".to_string())
            }
        }
        _ => {
            if local.is_some() {
                (PlannedAction::Upload, "上传本地派生索引缓存".to_string())
            } else {
                (PlannedAction::Noop, "两边均无索引缓存".to_string())
            }
        }
    }
}

fn classify(
    base: Option<&BaseEntry>,
    local: Option<&LocalMeta>,
    remote: Option<&RemoteEntry>,
) -> (PlannedAction, String) {
    let local_hash = local.map(|item| item.hash.as_str());
    let remote_hash = remote
        .filter(|item| !item.deleted)
        .and_then(|item| item.content_hash.as_deref());
    let remote_missing = remote.is_none();
    let remote_deleted = remote.is_some_and(|item| item.deleted);
    if local_hash == remote_hash {
        return (PlannedAction::Noop, "本地与远端一致".to_string());
    }
    let Some(base) = base else {
        if remote_deleted {
            return if local.is_none() {
                (PlannedAction::Noop, "远端已删除且本地不存在".to_string())
            } else {
                (
                    PlannedAction::Conflict {
                        kind: "modifyDelete".to_string(),
                    },
                    "远端墓碑与本地独立新增冲突".to_string(),
                )
            };
        }
        return match (local_hash, remote_hash) {
            (Some(_), None) => (PlannedAction::Upload, "远端尚无此文件".to_string()),
            (None, Some(_)) => (PlannedAction::Download, "本地尚无远端文件".to_string()),
            (Some(_), Some(_)) => (
                PlannedAction::Conflict {
                    kind: "bothAdded".to_string(),
                },
                "两台设备独立新增了同一路径且内容不同".to_string(),
            ),
            (None, None) => (PlannedAction::Noop, "两边均无文件".to_string()),
        };
    };

    if remote_missing {
        return if local_hash == base.content_hash.as_deref() {
            (
                PlannedAction::Noop,
                "远端 manifest 缺少该路径的显式墓碑；为防误删，本地保持不动".to_string(),
            )
        } else {
            (
                PlannedAction::Upload,
                "本地有修改，远端没有该路径记录".to_string(),
            )
        };
    }
    if remote_deleted {
        return if local_hash == base.content_hash.as_deref() {
            (PlannedAction::DeleteLocal, "远端明确标记删除".to_string())
        } else if local.is_none() {
            (PlannedAction::Noop, "两边均已删除".to_string())
        } else {
            (
                PlannedAction::Conflict {
                    kind: "modifyDelete".to_string(),
                },
                "远端删除与本地修改冲突".to_string(),
            )
        };
    }
    if local_hash == base.content_hash.as_deref() {
        return (PlannedAction::Download, "远端文件已更新".to_string());
    }
    if remote_hash == base.content_hash.as_deref() {
        return if local.is_some() {
            (PlannedAction::Upload, "本地文件已更新".to_string())
        } else {
            (PlannedAction::DeleteRemote, "本地文件已删除".to_string())
        };
    }
    let kind = if local.is_none() || remote_hash.is_none() {
        "modifyDelete"
    } else {
        "bothModified"
    };
    (
        PlannedAction::Conflict {
            kind: kind.to_string(),
        },
        "本地与远端都相对共同祖先发生变化".to_string(),
    )
}

fn summarize(changes: &[PlannedChange], pending_queue: usize) -> SyncPreview {
    let mut preview = SyncPreview {
        pending_queue,
        ..Default::default()
    };
    for change in changes {
        let item = SyncPlanItem {
            kind: match change.action {
                PlannedAction::Upload => "upload",
                PlannedAction::Download => "download",
                PlannedAction::DeleteRemote => "deleteRemote",
                PlannedAction::DeleteLocal => "deleteLocal",
                PlannedAction::Conflict { .. } => "conflict",
                PlannedAction::Noop => "noop",
            }
            .to_string(),
            path: change.path.clone(),
            size_bytes: change.size,
            reason: change.reason.clone(),
        };
        preview.total_bytes = preview.total_bytes.saturating_add(change.size);
        match change.action {
            PlannedAction::Upload => preview.uploads.push(item),
            PlannedAction::Download => preview.downloads.push(item),
            PlannedAction::DeleteRemote => preview.remote_deletes.push(item),
            PlannedAction::DeleteLocal => preview.local_deletes.push(item),
            PlannedAction::Conflict { .. } => preview.conflicts.push(item),
            PlannedAction::Noop => {}
        }
    }
    if !preview.conflicts.is_empty() {
        preview.warnings.push(format!(
            "{} 个文件存在冲突；冲突文件不会自动覆盖，需在冲突面板选择版本或三方合并。",
            preview.conflicts.len()
        ));
    }
    if !preview.remote_deletes.is_empty() || !preview.local_deletes.is_empty() {
        preview
            .warnings
            .push("删除会先在本机同步历史中留档；请确认另一台设备已不再需要该版本。".to_string());
    }
    preview
}

fn queue_changes(state: &mut StoredSyncState, changes: &[PlannedChange]) {
    for change in changes {
        let action = match change.action {
            PlannedAction::Upload => "upload",
            PlannedAction::DeleteRemote => "delete",
            _ => continue,
        };
        if let Some(item) = state.queue.iter_mut().find(|item| item.path == change.path) {
            item.action = action.to_string();
            item.last_error = None;
        } else {
            state.queue.push(QueueItem {
                id: format!("queue_{}", Uuid::new_v4()),
                path: change.path.clone(),
                action: action.to_string(),
                queued_at: now_iso(),
                last_error: None,
            });
        }
    }
}

fn queue_offline_changes(
    state: &mut StoredSyncState,
    config: &StoredSyncConfig,
    local: &BTreeMap<String, LocalMeta>,
) {
    let paths: BTreeSet<String> = state
        .base
        .keys()
        .chain(local.keys())
        .filter(|path| selected_path(path, config))
        .cloned()
        .collect();
    for path in paths {
        let base = state.base.get(&path);
        let current = local.get(&path);
        let changed = match base {
            Some(base) if base.deleted => current.is_some(),
            Some(base) => current.map(|item| item.hash.as_str()) != base.content_hash.as_deref(),
            None => current.is_some(),
        };
        if !changed {
            continue;
        }
        let action = if current.is_some() {
            "upload"
        } else {
            "delete"
        };
        if let Some(item) = state.queue.iter_mut().find(|item| item.path == path) {
            item.action = action.to_string();
        } else {
            state.queue.push(QueueItem {
                id: format!("queue_{}", Uuid::new_v4()),
                path,
                action: action.to_string(),
                queued_at: now_iso(),
                last_error: Some("上次同步时远端不可达，等待重试。".to_string()),
            });
        }
    }
}

fn mark_queue_error(state: &mut StoredSyncState, changes: &[PlannedChange], message: &str) {
    for change in changes {
        if let Some(item) = state.queue.iter_mut().find(|item| item.path == change.path) {
            item.last_error = Some(message.to_string());
        }
    }
}

fn clear_queue(state: &mut StoredSyncState, changes: &[PlannedChange]) {
    let paths: BTreeSet<&str> = changes.iter().map(|item| item.path.as_str()).collect();
    state
        .queue
        .retain(|item| !paths.contains(item.path.as_str()));
}

fn summary_of(conflict: StoredConflict) -> SyncConflictSummary {
    SyncConflictSummary {
        id: conflict.id,
        can_merge_text: conflict.path.ends_with(".md"),
        path: conflict.path,
        kind: conflict.kind,
        local_hash: conflict.local_hash,
        remote_hash: conflict.remote_hash,
        base_hash: conflict.base_hash,
        detected_at: conflict.detected_at,
    }
}

fn selected_path(path: &str, config: &StoredSyncConfig) -> bool {
    if path == INDEX_SYNC_PATH {
        return config.sync_index;
    }
    if path.starts_with("attachments/") {
        return config.sync_attachments;
    }
    config.sync_notes
        && path.ends_with(".md")
        && !path.starts_with(".knowledge/")
        && !path.starts_with(".trash/")
}

fn object_id(path: &str, hash: &str) -> String {
    format!(
        "objects/{}-{}",
        &sha256_hex_str(path)[..24],
        &hash[..16.min(hash.len())]
    )
}

fn manifest_aad(prefix: &str) -> String {
    format!("manifest:{prefix}")
}

fn object_aad(prefix: &str, object_id: &str) -> String {
    format!("object:{prefix}:{object_id}")
}

fn present(value: &Option<String>) -> bool {
    value
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
}
