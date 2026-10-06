//! 双向链接命令（`docs/PLAN_V1.1.md` M1-2）。

use tauri::State;

use crate::application::link_service::LinkService;
use crate::core::error::Result;
use crate::domain::link::{Backlink, LinkCounts, LinkRebuildReport, OutgoingLink, UnresolvedLink};
use crate::state::AppState;

use super::run;

/// 反向链接：谁引用了这篇笔记。
#[tauri::command]
pub async fn links_get_backlinks(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<Vec<Backlink>> {
    let context = state.inner.context()?;
    run(move || LinkService::new(context).backlinks(&note_id)).await
}

/// 出链：这篇笔记引用了谁（含未解析与外链）。
#[tauri::command]
pub async fn links_get_outgoing(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<Vec<OutgoingLink>> {
    let context = state.inner.context()?;
    run(move || LinkService::new(context).outgoing(&note_id)).await
}

/// 全库未解析链接。
#[tauri::command]
pub async fn links_get_unresolved(state: State<'_, AppState>) -> Result<Vec<UnresolvedLink>> {
    let context = state.inner.context()?;
    run(move || LinkService::new(context).unresolved()).await
}

/// 引用计数（反链 / 出链 / 未解析）。
#[tauri::command]
pub async fn links_get_counts(state: State<'_, AppState>, note_id: String) -> Result<LinkCounts> {
    let context = state.inner.context()?;
    run(move || LinkService::new(context).counts(&note_id)).await
}

/// 全量重建链接索引。
#[tauri::command]
pub async fn links_rebuild(state: State<'_, AppState>) -> Result<LinkRebuildReport> {
    let context = state.inner.context()?;
    run(move || LinkService::new(context).rebuild()).await
}
