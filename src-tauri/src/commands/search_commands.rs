//! 搜索相关命令（设计文档 14.6）。

use tauri::State;

use crate::application::search_service::SearchService;
use crate::core::error::Result;
use crate::domain::note::NoteSummary;
use crate::domain::search::{IndexStatus, SearchQuery, SearchResponse};
use crate::state::AppState;

use super::run;

#[tauri::command]
pub async fn search_notes(
    state: State<'_, AppState>,
    query: SearchQuery,
) -> Result<SearchResponse> {
    let context = state.inner.context()?;
    run(move || SearchService::new(context).search(query)).await
}

#[tauri::command]
pub async fn search_quick(
    state: State<'_, AppState>,
    keyword: String,
    limit: Option<i64>,
) -> Result<Vec<NoteSummary>> {
    let context = state.inner.context()?;
    run(move || SearchService::new(context).quick_search(&keyword, limit.unwrap_or(10))).await
}

#[tauri::command]
pub async fn search_rebuild_index(state: State<'_, AppState>) -> Result<IndexStatus> {
    let context = state.inner.context()?;
    run(move || SearchService::new(context).rebuild_index()).await
}

#[tauri::command]
pub async fn search_get_status(state: State<'_, AppState>) -> Result<IndexStatus> {
    let context = state.inner.context()?;
    run(move || SearchService::new(context).status()).await
}
