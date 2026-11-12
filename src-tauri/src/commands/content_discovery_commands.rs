//! 内容发现命令：只做参数转发，抓取/状态/保存逻辑在 application 层。

use tauri::State;

use crate::application::content_discovery::ContentDiscoveryService;
use crate::core::error::Result;
use crate::domain::content_discovery::{
    ContentCapture, ContentCaptureInput, ContentCaptureSaveInput, ContentCaptureSaveResult,
    ContentDiscoveryRunInput,
};
use crate::state::AppState;

use super::run;

#[tauri::command]
pub async fn content_capture_create(
    state: State<'_, AppState>,
    input: ContentCaptureInput,
) -> Result<ContentCapture> {
    let context = state.inner.context()?;
    run(move || ContentDiscoveryService::new(context).create(input)).await
}

#[tauri::command]
pub async fn content_capture_list(
    state: State<'_, AppState>,
    status: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<ContentCapture>> {
    let context = state.inner.context()?;
    run(move || ContentDiscoveryService::new(context).list(status.as_deref(), limit.unwrap_or(50)))
        .await
}

#[tauri::command]
pub async fn content_capture_get(
    state: State<'_, AppState>,
    capture_id: String,
) -> Result<ContentCapture> {
    let context = state.inner.context()?;
    run(move || ContentDiscoveryService::new(context).get(&capture_id)).await
}

#[tauri::command]
pub async fn content_discovery_run(
    state: State<'_, AppState>,
    input: ContentDiscoveryRunInput,
) -> Result<ContentCapture> {
    let context = state.inner.context()?;
    run(move || ContentDiscoveryService::new(context).crawl_selected(input)).await
}

#[tauri::command]
pub async fn content_capture_refresh(
    state: State<'_, AppState>,
    capture_id: String,
    confirmed: bool,
) -> Result<ContentCapture> {
    let context = state.inner.context()?;
    run(move || ContentDiscoveryService::new(context).refresh(&capture_id, confirmed)).await
}

#[tauri::command]
pub async fn content_capture_save(
    state: State<'_, AppState>,
    input: ContentCaptureSaveInput,
) -> Result<ContentCaptureSaveResult> {
    let context = state.inner.context()?;
    run(move || ContentDiscoveryService::new(context).save(input)).await
}

#[tauri::command]
pub async fn content_capture_ignore(
    state: State<'_, AppState>,
    capture_id: String,
) -> Result<ContentCapture> {
    let context = state.inner.context()?;
    run(move || ContentDiscoveryService::new(context).ignore(&capture_id)).await
}
