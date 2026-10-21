use tauri::State;

use crate::application::ai::writing::WritingService;
use crate::core::error::Result;
use crate::domain::writing::{
    WritingApplyInput, WritingApplyPreview, WritingApplyResult, WritingDraft, WritingRequest,
    WritingSummarySaveInput,
};
use crate::state::AppState;

use super::run;

#[tauri::command]
pub async fn writing_draft(
    state: State<'_, AppState>,
    input: WritingRequest,
) -> Result<WritingDraft> {
    let context = state.inner.context()?;
    run(move || WritingService::new(context).draft(input)).await
}

#[tauri::command]
pub async fn writing_apply_preview(
    state: State<'_, AppState>,
    input: WritingApplyInput,
) -> Result<WritingApplyPreview> {
    let context = state.inner.context()?;
    run(move || WritingService::new(context).apply_preview(&input)).await
}

#[tauri::command]
pub async fn writing_apply(
    state: State<'_, AppState>,
    input: WritingApplyInput,
) -> Result<WritingApplyResult> {
    let context = state.inner.context()?;
    run(move || WritingService::new(context).apply(input)).await
}

#[tauri::command]
pub async fn writing_summary_save(
    state: State<'_, AppState>,
    input: WritingSummarySaveInput,
) -> Result<WritingApplyResult> {
    let context = state.inner.context()?;
    run(move || WritingService::new(context).save_summary(input)).await
}
