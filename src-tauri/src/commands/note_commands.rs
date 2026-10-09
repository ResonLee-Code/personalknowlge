//! 笔记相关命令（设计文档 14.3）。

use tauri::State;

use crate::application::history_service::{HistoryContent, HistoryEntry};
use crate::application::note_service::NoteService;
use crate::application::import_service::{ImportNotesInput, ImportReport, ImportService};
use crate::application::export_service::{ExportNoteInput, ExportNoteResult, ExportService};
use crate::core::error::Result;
use crate::domain::note::{
    CreateNoteInput, NoteDetail, NoteListQuery, NoteListResult, NoteStatus, NoteSummary,
    ReorderNotesInput, SaveNoteInput, SaveNoteResult, SetParentInput, SetSourceInput,
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

/// 更新来源与时效（N1）。
#[tauri::command]
pub async fn note_set_source(
    state: State<'_, AppState>,
    input: SetSourceInput,
) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).set_source(input)).await
}

/// 设置父文档（子文档层级）：`parentId = null` 移到顶层。
#[tauri::command]
pub async fn note_set_parent(
    state: State<'_, AppState>,
    input: SetParentInput,
) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).set_parent(input)).await
}

/// 手动排序：按传入顺序重写同层笔记的 `order`（返回实际更新的篇数）。
#[tauri::command]
pub async fn notes_reorder(
    state: State<'_, AppState>,
    input: ReorderNotesInput,
) -> Result<usize> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).reorder(input)).await
}

/// 版本历史：列出某篇笔记的快照（新的在前）。
#[tauri::command]
pub async fn note_history_list(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<Vec<HistoryEntry>> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).history_list(&note_id)).await
}

/// 版本历史：读取某个快照的内容。
#[tauri::command]
pub async fn note_history_read(
    state: State<'_, AppState>,
    note_id: String,
    entry_id: String,
) -> Result<HistoryContent> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).history_read(&note_id, &entry_id)).await
}

/// 版本历史：还原到某个快照（只还原正文，元数据保持当前值）。
#[tauri::command]
pub async fn note_history_restore(
    state: State<'_, AppState>,
    note_id: String,
    entry_id: String,
) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).history_restore(&note_id, &entry_id)).await
}

/// 版本历史：删除某个快照（返回剩余数量）。
#[tauri::command]
pub async fn note_history_remove(
    state: State<'_, AppState>,
    note_id: String,
    entry_id: String,
) -> Result<usize> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).history_remove(&note_id, &entry_id)).await
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

/// 置顶 / 取消置顶（对标语雀「置顶文档」）。
#[tauri::command]
pub async fn note_set_pinned(
    state: State<'_, AppState>,
    id: String,
    pinned: bool,
) -> Result<NoteDetail> {
    let context = state.inner.context()?;
    run(move || NoteService::new(context).set_pinned(&id, pinned)).await
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

/// 导入外部 Markdown / 文本文件或整个目录（复制进知识库，不修改源文件）。
#[tauri::command]
pub async fn note_import(
    state: State<'_, AppState>,
    input: ImportNotesInput,
) -> Result<ImportReport> {
    let context = state.inner.context()?;
    run(move || ImportService::new(context).import(input)).await
}

/// 导出单篇笔记（Markdown / HTML / Word(docx) / PDF 打印视图）。
#[tauri::command]
pub async fn note_export(
    state: State<'_, AppState>,
    input: ExportNoteInput,
) -> Result<ExportNoteResult> {
    let context = state.inner.context()?;
    run(move || ExportService::new(context).export_note(input)).await
}
