//! 标签相关命令（设计文档 14.5）。

use tauri::State;

use crate::application::tag_service::TagService;
use crate::core::error::Result;
use crate::domain::tag::Tag;
use crate::state::AppState;

use super::run;

#[tauri::command]
pub async fn tag_list(state: State<'_, AppState>) -> Result<Vec<Tag>> {
    let context = state.inner.context()?;
    run(move || TagService::new(context).list()).await
}

#[tauri::command]
pub async fn tag_create(state: State<'_, AppState>, name: String) -> Result<Tag> {
    let context = state.inner.context()?;
    run(move || TagService::new(context).get_or_create(&name)).await
}

#[tauri::command]
pub async fn tag_rename(state: State<'_, AppState>, id: String, name: String) -> Result<Tag> {
    let context = state.inner.context()?;
    run(move || TagService::new(context).rename(&id, &name)).await
}

#[tauri::command]
pub async fn tag_delete(state: State<'_, AppState>, id: String) -> Result<()> {
    let context = state.inner.context()?;
    run(move || TagService::new(context).delete(&id)).await
}

#[tauri::command]
pub async fn note_set_tag_names(
    state: State<'_, AppState>,
    id: String,
    tags: Vec<String>,
) -> Result<Vec<Tag>> {
    let context = state.inner.context()?;
    run(move || TagService::new(context).set_note_tags(&id, &tags)).await
}
