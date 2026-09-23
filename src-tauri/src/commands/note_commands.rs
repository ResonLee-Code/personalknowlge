//! 笔记相关命令（设计文档 14.3）。

use tauri::State;

use crate::application::note_service::NoteService;
use crate::core::error::Result;
use crate::domain::note::{
    CreateNoteInput, NoteDetail, NoteListQuery, NoteListResult, NoteStatus, NoteSummary,
    SaveNoteInput, SaveNoteResult,
};
use crate::state::AppState;
use crate::repositories::{NoteRepository, SqliteNoteRepository};

use super::run;

#[tauri::command]
pub async fn note_create(
    state: State<'_, AppState>,
    input: CreateNoteInput,
) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).create(input)).await
}

#[tauri::command]
pub async fn note_get(state: State<'_, AppState>, id: String) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).get(&id)).await
}

#[tauri::command]
pub async fn note_save(
    state: State<'_, AppState>,
    input: SaveNoteInput,
) -> Result<SaveNoteResult> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).save(input)).await
}

/// 与 `note_save` 等价的别名，符合设计文档 14.3 的接口清单。
#[tauri::command]
pub async fn note_update(
    state: State<'_, AppState>,
    input: SaveNoteInput,
) -> Result<SaveNoteResult> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).save(input)).await
}

#[tauri::command]
pub async fn note_list(
    state: State<'_, AppState>,
    query: NoteListQuery,
) -> Result<NoteListResult> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).list(query)).await
}

#[tauri::command]
pub async fn note_recent(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<NoteSummary>> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).recent(limit.unwrap_or(10))).await
}

#[tauri::command]
pub async fn note_list_deleted(state: State<'_, AppState>) -> Result<Vec<NoteSummary>> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).list_deleted()).await
}

#[tauri::command]
pub async fn note_list_missing(state: State<'_, AppState>) -> Result<Vec<NoteSummary>> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).list_missing()).await
}

#[tauri::command]
pub async fn note_delete(state: State<'_, AppState>, id: String) -> Result<()> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).delete(&id)).await
}

#[tauri::command]
pub async fn note_restore(state: State<'_, AppState>, id: String) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).restore(&id)).await
}

#[tauri::command]
pub async fn note_purge(state: State<'_, AppState>, id: String) -> Result<()> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).purge(&id)).await
}

#[tauri::command]
pub async fn note_rename(
    state: State<'_, AppState>,
    id: String,
    title: String,
    rename_file: Option<bool>,
) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).rename(&id, &title, rename_file.unwrap_or(true))).await
}

#[tauri::command]
pub async fn note_move(
    state: State<'_, AppState>,
    id: String,
    category_id: Option<String>,
) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).move_to_category(&id, category_id.as_deref())).await
}

#[tauri::command]
pub async fn note_duplicate(state: State<'_, AppState>, id: String) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).duplicate(&id)).await
}

#[tauri::command]
pub async fn note_set_favorite(
    state: State<'_, AppState>,
    id: String,
    favorite: bool,
) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).set_favorite(&id, favorite)).await
}

#[tauri::command]
pub async fn note_set_tags(
    state: State<'_, AppState>,
    id: String,
    tags: Vec<String>,
) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).set_tags(&id, &tags)).await
}

/// 按状态统计，用于侧边栏数字。
#[tauri::command]
pub async fn note_count(state: State<'_, AppState>, status: Option<String>) -> Result<i64> {
    let context = state.inner.context()?;
    let status = status
        .as_deref()
        .map(NoteStatus::from_str)
        .unwrap_or(NoteStatus::Active);
    run(move || {
        context.db.with_conn(|conn| {
            SqliteNoteRepository.count_by_status(conn, &context.workspace.id, status)
        })
    })
    .await
}
