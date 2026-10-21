use tauri::State;

use crate::application::sync::SyncService;
use crate::core::error::Result;
use crate::domain::sync::{
    ResolveSyncConflictInput, RestoreSyncHistoryInput, SyncConflictDetail, SyncConflictSummary,
    SyncHistoryEntry, SyncMergePreview, SyncPreview, SyncRunInput, SyncRunReport, SyncSettings,
    UpdateSyncSettingsInput,
};
use crate::state::AppState;

use super::run;

#[tauri::command]
pub async fn sync_settings_get(state: State<'_, AppState>) -> Result<SyncSettings> {
    let context = state.inner.context()?;
    run(move || Ok(SyncService::new(context).settings())).await
}

#[tauri::command]
pub async fn sync_settings_update(
    state: State<'_, AppState>,
    input: UpdateSyncSettingsInput,
) -> Result<SyncSettings> {
    let context = state.inner.context()?;
    run(move || SyncService::new(context).update_settings(input)).await
}

#[tauri::command]
pub async fn sync_preview(state: State<'_, AppState>, passphrase: String) -> Result<SyncPreview> {
    let context = state.inner.context()?;
    run(move || SyncService::new(context).preview(&passphrase)).await
}

#[tauri::command]
pub async fn sync_run(state: State<'_, AppState>, input: SyncRunInput) -> Result<SyncRunReport> {
    let context = state.inner.context()?;
    run(move || SyncService::new(context).run(input)).await
}

#[tauri::command]
pub async fn sync_conflicts(state: State<'_, AppState>) -> Result<Vec<SyncConflictSummary>> {
    let context = state.inner.context()?;
    run(move || SyncService::new(context).conflicts()).await
}

#[tauri::command]
pub async fn sync_conflict_detail(
    state: State<'_, AppState>,
    conflict_id: String,
) -> Result<SyncConflictDetail> {
    let context = state.inner.context()?;
    run(move || SyncService::new(context).conflict_detail(&conflict_id)).await
}

#[tauri::command]
pub async fn sync_conflict_merge_preview(
    state: State<'_, AppState>,
    conflict_id: String,
) -> Result<SyncMergePreview> {
    let context = state.inner.context()?;
    run(move || SyncService::new(context).merge_preview(&conflict_id)).await
}

#[tauri::command]
pub async fn sync_conflict_resolve(
    state: State<'_, AppState>,
    input: ResolveSyncConflictInput,
) -> Result<()> {
    let context = state.inner.context()?;
    run(move || SyncService::new(context).resolve_conflict(input)).await
}

#[tauri::command]
pub async fn sync_history(state: State<'_, AppState>) -> Result<Vec<SyncHistoryEntry>> {
    let context = state.inner.context()?;
    run(move || SyncService::new(context).history()).await
}

#[tauri::command]
pub async fn sync_history_restore(
    state: State<'_, AppState>,
    input: RestoreSyncHistoryInput,
) -> Result<String> {
    let context = state.inner.context()?;
    run(move || SyncService::new(context).restore_history(input)).await
}
