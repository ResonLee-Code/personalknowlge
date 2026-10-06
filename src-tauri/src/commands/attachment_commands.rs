//! 附件命令（V1.1 M2-1）。

use tauri::State;

use crate::application::attachment_service::{
    AttachmentData, AttachmentService, ReadAttachmentInput, SaveAttachmentInput, SavedAttachment,
};
use crate::core::error::Result;
use crate::state::AppState;

use super::run;

/// 把编辑器粘贴的图片/文件写入 `attachments/YYYY-MM/`，返回可写入正文的链接路径。
#[tauri::command]
pub async fn attachment_save(
    state: State<'_, AppState>,
    input: SaveAttachmentInput,
) -> Result<SavedAttachment> {
    let context = state.inner.context()?;
    run(move || AttachmentService::new(context).save(input)).await
}

/// 读取工作区内的相对图片并返回 data URL（供 Markdown 预览内联显示）。
#[tauri::command]
pub async fn attachment_read(
    state: State<'_, AppState>,
    input: ReadAttachmentInput,
) -> Result<AttachmentData> {
    let context = state.inner.context()?;
    run(move || AttachmentService::new(context).read(input)).await
}
