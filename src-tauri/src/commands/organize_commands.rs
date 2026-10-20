//! Organize P0 命令入口。业务与隐私闸门在 `application::ai::organize`。

use tauri::State;

use crate::application::ai::organize::OrganizeService;
use crate::core::error::{AppError, Result};
use crate::domain::organize::{
    AiJob, AiSuggestion, OrganizeBatchReport, SuggestionKind, SuggestionStatus,
};
use crate::state::AppState;

use super::run;

fn kind(value: &str) -> Result<SuggestionKind> {
    SuggestionKind::parse(value).ok_or_else(|| {
        AppError::invalid_input("建议类型必须是 summary / tags / relations / duplicate。")
    })
}

fn status(value: &str) -> Result<SuggestionStatus> {
    SuggestionStatus::parse(value).ok_or_else(|| AppError::invalid_input("未知建议状态。"))
}

#[tauri::command]
pub async fn organize_generate(
    state: State<'_, AppState>,
    note_id: String,
    kind_value: String,
    confirm_outbound: Option<bool>,
) -> Result<AiSuggestion> {
    let context = state.inner.context()?;
    let suggestion_kind = kind(&kind_value)?;
    run(move || {
        OrganizeService::new(context).generate_with_confirmation(
            &note_id,
            suggestion_kind,
            confirm_outbound.unwrap_or(false),
        )
    })
    .await
}

#[tauri::command]
pub async fn organize_suggestions(
    state: State<'_, AppState>,
    status_value: Option<String>,
    kind_value: Option<String>,
    limit: usize,
) -> Result<Vec<AiSuggestion>> {
    let context = state.inner.context()?;
    let suggestion_status = status_value.as_deref().map(status).transpose()?;
    let suggestion_kind = kind_value.as_deref().map(kind).transpose()?;
    run(move || {
        OrganizeService::new(context).list_suggestions(suggestion_status, suggestion_kind, limit)
    })
    .await
}

#[tauri::command]
pub async fn organize_accept(state: State<'_, AppState>, id: String) -> Result<AiSuggestion> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).accept(&id)).await
}

#[tauri::command]
pub async fn organize_accept_many(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<OrganizeBatchReport> {
    let context = state.inner.context()?;
    run(move || Ok(OrganizeService::new(context).accept_many(&ids))).await
}

#[tauri::command]
pub async fn organize_reject(state: State<'_, AppState>, id: String) -> Result<AiSuggestion> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).reject(&id)).await
}

#[tauri::command]
pub async fn organize_undo(state: State<'_, AppState>, id: String) -> Result<AiSuggestion> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).undo(&id)).await
}

#[tauri::command]
pub async fn organize_job_create(
    state: State<'_, AppState>,
    kind_value: String,
    note_ids: Vec<String>,
    confirm_outbound: Option<bool>,
) -> Result<AiJob> {
    let context = state.inner.context()?;
    let suggestion_kind = kind(&kind_value)?;
    run(move || {
        OrganizeService::new(context).create_job_with_confirmation(
            suggestion_kind,
            &note_ids,
            confirm_outbound.unwrap_or(false),
        )
    })
    .await
}

#[tauri::command]
pub async fn organize_job_get(state: State<'_, AppState>, id: String) -> Result<AiJob> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).get_job(&id)).await
}

#[tauri::command]
pub async fn organize_job_list(state: State<'_, AppState>, limit: usize) -> Result<Vec<AiJob>> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).list_jobs(limit)).await
}

#[tauri::command]
pub async fn organize_job_run(state: State<'_, AppState>, id: String) -> Result<AiJob> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).run_job(&id)).await
}

#[tauri::command]
pub async fn organize_job_pause(state: State<'_, AppState>, id: String) -> Result<AiJob> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).pause_job(&id)).await
}

#[tauri::command]
pub async fn organize_job_resume(state: State<'_, AppState>, id: String) -> Result<AiJob> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).resume_job(&id)).await
}

#[tauri::command]
pub async fn organize_job_cancel(state: State<'_, AppState>, id: String) -> Result<AiJob> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).cancel_job(&id)).await
}

#[tauri::command]
pub async fn organize_job_retry(state: State<'_, AppState>, id: String) -> Result<AiJob> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).retry_job(&id)).await
}

#[tauri::command]
pub async fn organize_recover(state: State<'_, AppState>) -> Result<usize> {
    let context = state.inner.context()?;
    run(move || OrganizeService::new(context).recover_interrupted()).await
}
