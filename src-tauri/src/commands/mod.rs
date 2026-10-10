//! Tauri 命令层（设计文档 14）。
//!
//! 命令只做参数转发：把阻塞的业务逻辑放到后台线程执行，
//! 避免阻塞 UI 线程（设计文档 6.1：索引更新不阻塞编辑器）。

pub mod ai_commands;
pub mod attachment_commands;
pub mod app_commands;
pub mod backup_commands;
pub mod batch_commands;
pub mod category_commands;
pub mod clip_commands;
pub mod graph_commands;
pub mod link_commands;
pub mod note_commands;
pub mod relation_commands;
pub mod research_commands;
pub mod search_commands;
pub mod tag_commands;
pub mod template_commands;
pub mod workspace_commands;

use crate::core::error::{AppError, Result};

/// 在后台线程执行阻塞任务。
pub async fn run<T, F>(task: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(task).await {
        Ok(result) => result,
        Err(error) => Err(AppError::internal(format!("后台任务执行失败：{error}"))),
    }
}
