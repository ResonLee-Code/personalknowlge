//! 备份与导出命令（设计文档 14.7）。

use tauri::State;

use crate::application::backup_service::{BackupService, RestoreReport};
use crate::core::error::Result;
use crate::domain::backup::{
    BackupCreateInput, BackupInfo, BackupValidation, ExportMarkdownInput, ExportResult,
    RestoreBackupInput,
};
use crate::state::AppState;

use super::run;

#[tauri::command]
pub async fn backup_create(
    state: State<'_, AppState>,
    input: Option<BackupCreateInput>,
) -> Result<BackupInfo> {
    let context = state.inner.context()?;
    let input = input.unwrap_or(BackupCreateInput { target_dir: None, note: None });
    run(move || BackupService::new(context).create(input)).await
}

#[tauri::command]
pub async fn backup_list(state: State<'_, AppState>) -> Result<Vec<BackupInfo>> {
    let context = state.inner.context()?;
    run(move || BackupService::new(context).list()).await
}

#[tauri::command]
pub async fn backup_validate(
    state: State<'_, AppState>,
    path: String,
) -> Result<BackupValidation> {
    let context = state.inner.context()?;
    run(move || BackupService::new(context).validate(&path)).await
}

#[tauri::command]
pub async fn backup_restore(
    state: State<'_, AppState>,
    input: RestoreBackupInput,
) -> Result<RestoreReport> {
    let context = state.inner.context()?;
    run(move || BackupService::new(context).restore(input)).await
}

#[tauri::command]
pub async fn export_markdown(
    state: State<'_, AppState>,
    input: ExportMarkdownInput,
) -> Result<ExportResult> {
    let context = state.inner.context()?;
    run(move || BackupService::new(context).export_markdown(input)).await
}

#[tauri::command]
pub async fn backup_directory(state: State<'_, AppState>) -> Result<String> {
    let context = state.inner.context()?;
    run(move || Ok(BackupService::new(context).backup_directory())).await
}
