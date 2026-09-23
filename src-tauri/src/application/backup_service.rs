//! 备份与导出服务（设计文档 20 / FR-B01 ~ FR-B03）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::ids::ulid;
use crate::core::time::now_iso;
use crate::domain::backup::{
    BackupCreateInput, BackupInfo, BackupValidation, ExportMarkdownInput, ExportResult,
    RestoreBackupInput,
};
use crate::domain::note::NoteStatus;
use crate::infrastructure::backup::{
    extract_archive, read_manifest, validate_archive, write_archive, BackupEntry, BackupManifest,
    BackupSource, APP_NAME, APP_VERSION, MANIFEST_NAME,
};
use crate::repositories::{NoteRepository, SqliteNoteRepository};

use super::context::WorkspaceContext;

/// 恢复结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReport {
    pub target_dir: String,
    pub extracted_files: i64,
    pub validation: BackupValidation,
}

pub struct BackupService {
    ctx: Arc<WorkspaceContext>,
}

impl BackupService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    /// 创建备份（FR-B01 / 20.2 / 20.3）。
    pub fn create(&self, input: BackupCreateInput) -> Result<BackupInfo> {
        let target_dir = match input.target_dir.as_deref() {
            Some(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
            _ => self.ctx.paths.backups_dir(),
        };
        std::fs::create_dir_all(&target_dir)?;

        let prefix = input
            .note
            .as_deref()
            .map(|note| crate::core::text::sanitize_file_name(note))
            .filter(|note| !note.is_empty())
            .unwrap_or_else(|| "KnowledgeBackup".to_string());
        let file_name = format!("{}_{}.zip", prefix, crate::core::time::now_file_timestamp());
        let archive = target_dir.join(&file_name);

        // 数据库一致性快照
        let snapshot = self
            .ctx
            .paths
            .backups_dir()
            .join(format!(".snapshot-{}.sqlite", ulid()));
        self.ctx.db.snapshot_to(&snapshot)?;

        let mut entries: Vec<BackupEntry> = Vec::new();
        entries.push(BackupEntry {
            name: ".knowledge/database.sqlite".to_string(),
            source: BackupSource::File(snapshot.clone()),
        });
        let config_path = self.ctx.paths.config_path();
        if config_path.exists() {
            entries.push(BackupEntry {
                name: ".knowledge/config.json".to_string(),
                source: BackupSource::File(config_path),
            });
        }

        let notes = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list_by_status(conn, &self.ctx.workspace.id, NoteStatus::Active)
        })?;
        for note in notes {
            let path = self.ctx.paths.resolve_relative(&note.file_path)?;
            if path.exists() {
                entries.push(BackupEntry {
                    name: note.file_path.clone(),
                    source: BackupSource::File(path),
                });
            }
        }
        for attachment in self.ctx.storage.list_attachments()? {
            let path = self.ctx.paths.resolve_relative(&attachment)?;
            if path.exists() {
                entries.push(BackupEntry {
                    name: attachment,
                    source: BackupSource::File(path),
                });
            }
        }

        let mut manifest = BackupManifest::new(
            &self.ctx.workspace.id,
            &self.ctx.workspace.name,
            crate::infrastructure::db::SCHEMA_VERSION,
        );
        let result = write_archive(&archive, &entries, &mut manifest);
        let _ = std::fs::remove_file(&snapshot);
        result?;

        let metadata = std::fs::metadata(&archive)?;
        tracing::info!(target: "backup", "备份完成 {file_name}");

        Ok(BackupInfo {
            file_name,
            path: archive.to_string_lossy().to_string(),
            created_at: manifest.created_at.clone(),
            size_bytes: metadata.len(),
            note_count: manifest.note_count,
            attachment_count: manifest.attachment_count,
            includes_database: manifest.includes_database,
        })
    }

    /// 校验备份（FR-B01 / 20.4）。
    pub fn validate(&self, archive_path: &str) -> Result<BackupValidation> {
        let path = Path::new(archive_path);
        if !path.exists() {
            return Err(AppError::new(ErrorCode::BackupFailed, "备份文件不存在。"));
        }
        validate_archive(path)
    }

    /// 列出现有备份。
    pub fn list(&self) -> Result<Vec<BackupInfo>> {
        let dir = self.ctx.paths.backups_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut backups = Vec::new();
        for entry in std::fs::read_dir(&dir)?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("zip") {
                continue;
            }
            let metadata = entry.metadata()?;
            let manifest = read_manifest(&path).ok();
            backups.push(BackupInfo {
                file_name: entry.file_name().to_string_lossy().to_string(),
                path: path.to_string_lossy().to_string(),
                created_at: manifest
                    .as_ref()
                    .map(|manifest| manifest.created_at.clone())
                    .unwrap_or_else(|| {
                        metadata
                            .modified()
                            .ok()
                            .map(crate::core::time::system_time_to_iso)
                            .unwrap_or_else(now_iso)
                    }),
                size_bytes: metadata.len(),
                note_count: manifest.as_ref().map(|m| m.note_count).unwrap_or(0),
                attachment_count: manifest.as_ref().map(|m| m.attachment_count).unwrap_or(0),
                includes_database: manifest
                    .as_ref()
                    .map(|m| m.includes_database)
                    .unwrap_or(false),
            });
        }
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(backups)
    }

    /// 恢复到目标目录（FR-B03 / 20.5）。目标目录必须不存在或为空。
    pub fn restore(&self, input: RestoreBackupInput) -> Result<RestoreReport> {
        let archive = PathBuf::from(&input.archive_path);
        if !archive.exists() {
            return Err(AppError::new(ErrorCode::BackupFailed, "备份文件不存在。"));
        }
        let validation = validate_archive(&archive)?;
        if !validation.valid {
            return Err(AppError::new(
                ErrorCode::BackupFailed,
                "备份文件校验未通过，已中止恢复以避免数据损坏。",
            )
            .with_details(serde_json::to_value(&validation)?));
        }
        let target = PathBuf::from(&input.target_dir);
        let extracted = extract_archive(&archive, &target)?;
        tracing::info!(
            target: "backup",
            files = extracted,
            "备份恢复完成"
        );
        Ok(RestoreReport {
            target_dir: target.to_string_lossy().to_string(),
            extracted_files: extracted as i64,
            validation,
        })
    }

    /// 导出 Markdown（FR-B02）。
    pub fn export_markdown(&self, input: ExportMarkdownInput) -> Result<ExportResult> {
        let target = PathBuf::from(&input.target_dir);
        if target.exists() {
            let is_empty = std::fs::read_dir(&target)
                .map(|mut entries| entries.next().is_none())
                .unwrap_or(false);
            if !is_empty {
                return Err(AppError::new(
                    ErrorCode::ExportFailed,
                    "导出目录已存在且不为空，请选择空目录以避免覆盖。",
                ));
            }
        }
        std::fs::create_dir_all(&target)?;

        let notes = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list_by_status(conn, &self.ctx.workspace.id, NoteStatus::Active)
        })?;
        let mut note_count = 0i64;
        for note in &notes {
            let source = self.ctx.paths.resolve_relative(&note.file_path)?;
            if !source.exists() {
                continue;
            }
            let destination = target.join(&note.file_path);
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source, &destination)?;
            note_count += 1;
        }

        let mut attachment_count = 0i64;
        if input.include_attachments {
            for attachment in self.ctx.storage.list_attachments()? {
                let source = self.ctx.paths.resolve_relative(&attachment)?;
                if !source.exists() {
                    continue;
                }
                let destination = target.join(&attachment);
                if let Some(parent) = destination.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&source, &destination)?;
                attachment_count += 1;
            }
        }

        let summary = serde_json::json!({
            "app": APP_NAME,
            "appVersion": APP_VERSION,
            "exportedAt": now_iso(),
            "workspaceName": self.ctx.workspace.name,
            "noteCount": note_count,
            "attachmentCount": attachment_count,
        });
        std::fs::write(
            target.join("export-manifest.json"),
            serde_json::to_string_pretty(&summary)?,
        )?;

        Ok(ExportResult {
            target_dir: target.to_string_lossy().to_string(),
            note_count,
            attachment_count,
        })
    }

    /// 备份目录路径（设置页展示用）。
    pub fn backup_directory(&self) -> String {
        self.ctx
            .settings()
            .backup_dir
            .unwrap_or_else(|| self.ctx.paths.backups_dir().to_string_lossy().to_string())
    }
}

/// 备份包内清单文件名（供前端展示校验细节）。
pub fn manifest_name() -> &'static str {
    MANIFEST_NAME
}
