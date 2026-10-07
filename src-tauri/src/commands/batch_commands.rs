//! 批量操作命令（V1.1.2）。

use tauri::State;

use crate::application::batch_service::{
    BatchDeleteInput, BatchMoveInput, BatchReport, BatchService, BatchTagInput,
};
use crate::core::error::Result;
use crate::state::AppState;

use super::run;

/// 批量移动到分类（`categoryId = null` 表示移回 Inbox）。
#[tauri::command]
pub async fn notes_move_batch(
    state: State<'_, AppState>,
    input: BatchMoveInput,
) -> Result<BatchReport> {
    let context = state.inner.context()?;
    run(move || BatchService::new(context).move_notes(input)).await
}

/// 批量打标签（mode: add / remove / replace）。
#[tauri::command]
pub async fn notes_tag_batch(
    state: State<'_, AppState>,
    input: BatchTagInput,
) -> Result<BatchReport> {
    let context = state.inner.context()?;
    run(move || BatchService::new(context).tag_notes(input)).await
}

/// 批量移入回收站。
#[tauri::command]
pub async fn notes_delete_batch(
    state: State<'_, AppState>,
    input: BatchDeleteInput,
) -> Result<BatchReport> {
    let context = state.inner.context()?;
    run(move || BatchService::new(context).delete_notes(input)).await
}
