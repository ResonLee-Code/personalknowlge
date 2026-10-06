//! 类型化关系命令（N2）。

use tauri::State;

use crate::application::relation_service::{AddRelationInput, RelationService, RelationView};
use crate::core::error::Result;
use crate::state::AppState;

use super::run;

async fn service(state: &State<'_, AppState>) -> Result<std::sync::Arc<crate::application::context::WorkspaceContext>> {
    state.inner.context()
}

/// 某篇笔记的关系（出边 + 入边）。
#[tauri::command]
pub async fn relations_list(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<RelationView> {
    let context = service(&state).await?;
    run(move || RelationService::new(context).list(&note_id)).await
}

/// 新增关系（写回来源笔记 frontmatter）。
#[tauri::command]
pub async fn relations_add(
    state: State<'_, AppState>,
    input: AddRelationInput,
) -> Result<RelationView> {
    let context = service(&state).await?;
    run(move || RelationService::new(context).add(input)).await
}

/// 删除关系（同步 frontmatter）。
#[tauri::command]
pub async fn relations_remove(
    state: State<'_, AppState>,
    id: String,
) -> Result<RelationView> {
    let context = service(&state).await?;
    run(move || RelationService::new(context).remove(&id)).await
}

/// 全量重建派生关系表。
#[tauri::command]
pub async fn relations_rebuild(state: State<'_, AppState>) -> Result<usize> {
    let context = service(&state).await?;
    run(move || RelationService::new(context).rebuild()).await
}
