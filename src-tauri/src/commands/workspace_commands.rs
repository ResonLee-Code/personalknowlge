//! 工作区相关命令（设计文档 14.2）。

use std::sync::Arc;

use tauri::State;

use crate::application::workspace_service::WorkspaceService;
use crate::application::index_service::ScanReport;
use crate::core::error::Result;
use crate::domain::search::IndexStatus;
use crate::domain::settings::{AppSettings, UpdateSettingsInput};
use crate::domain::workspace::{WorkspaceStats, WorkspaceSummary};
use crate::state::AppState;

use super::run;

#[tauri::command]
pub async fn workspace_create(
    state: State<'_, AppState>,
    name: String,
    root: String,
) -> Result<crate::application::workspace_service::WorkspaceOpenResult> {
    let app = state.inner.clone();
    run(move || WorkspaceService::new(app).create(&name, &root)).await
}

#[tauri::command]
pub async fn workspace_open(
    state: State<'_, AppState>,
    root: String,
) -> Result<crate::application::workspace_service::WorkspaceOpenResult> {
    let app = state.inner.clone();
    run(move || WorkspaceService::new(app).open(&root)).await
}

#[tauri::command]
pub async fn workspace_switch(
    state: State<'_, AppState>,
    id: String,
) -> Result<crate::application::workspace_service::WorkspaceOpenResult> {
    let app = state.inner.clone();
    run(move || WorkspaceService::new(app).switch(&id)).await
}

#[tauri::command]
pub async fn workspace_close(state: State<'_, AppState>) -> Result<()> {
    let app = state.inner.clone();
    run(move || WorkspaceService::new(app).close()).await
}

#[tauri::command]
pub async fn workspace_get_current(
    state: State<'_, AppState>,
) -> Result<Option<crate::application::workspace_service::WorkspaceOpenResult>> {
    let app = state.inner.clone();
    run(move || WorkspaceService::new(app).current()).await
}

#[tauri::command]
pub async fn workspace_list(state: State<'_, AppState>) -> Result<Vec<WorkspaceSummary>> {
    let app = state.inner.clone();
    run(move || Ok(WorkspaceService::new(app).list())).await
}

#[tauri::command]
pub async fn workspace_stats(state: State<'_, AppState>) -> Result<WorkspaceStats> {
    let app = state.inner.clone();
    run(move || WorkspaceService::new(app).stats()).await
}

#[tauri::command]
pub async fn workspace_scan(state: State<'_, AppState>) -> Result<ScanReport> {
    let app = state.inner.clone();
    run(move || WorkspaceService::new(app).scan()).await
}

#[tauri::command]
pub async fn workspace_rebuild_index(state: State<'_, AppState>) -> Result<IndexStatus> {
    let app = state.inner.clone();
    run(move || WorkspaceService::new(app).rebuild_index()).await
}

#[tauri::command]
pub async fn workspace_forget(state: State<'_, AppState>, id: String) -> Result<()> {
    let app = state.inner.clone();
    run(move || WorkspaceService::new(app).forget(&id)).await
}

#[tauri::command]
pub async fn settings_get(state: State<'_, AppState>) -> Result<AppSettings> {
    let app = state.inner.clone();
    run(move || {
        let mut settings = app.context()?.settings();
        // 与「设置 → AI」同口径：只回显「API Key 配没配」，Key 本身不出现在任何响应里
        settings.ai.remote_model_key_set =
            crate::infrastructure::credential_store::remote_model_key_set(&app.data_dir);
        Ok(settings)
    })
    .await
}

#[tauri::command]
pub async fn settings_update(
    state: State<'_, AppState>,
    input: UpdateSettingsInput,
) -> Result<AppSettings> {
    let app = state.inner.clone();
    run(move || {
        let mut settings = WorkspaceService::new(Arc::clone(&app)).update_settings(input)?;
        settings.ai.remote_model_key_set =
            crate::infrastructure::credential_store::remote_model_key_set(&app.data_dir);
        Ok(settings)
    })
    .await
}

/// 已注册工作区（含当前活动标记）。
#[tauri::command]
pub async fn workspace_active_id(state: State<'_, AppState>) -> Result<Option<String>> {
    Ok(state.inner.active().map(|context| context.workspace.id.clone()))
}
