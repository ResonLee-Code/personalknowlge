//! Personal Knowledge OS —— 本地优先的个人知识管理桌面应用。
//!
//! 分层结构（设计文档 8）：
//! `commands` -> `application` -> `domain` / `repositories` -> `infrastructure`。

pub mod application;
pub mod commands;
pub mod core;
pub mod domain;
pub mod infrastructure;
pub mod repositories;
pub mod state;

use crate::core::events::TauriEventSink;
use crate::core::logging;
use crate::state::{AppInner, AppState};

pub fn run() {
    let data_dir = AppInner::default_data_dir();
    logging::init(Some(&data_dir.join("logs")));
    tracing::info!(target: "app", "启动 Personal Knowledge OS {}", env!("CARGO_PKG_VERSION"));

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(move |app| {
            use tauri::Manager;
            let events = TauriEventSink::new(app.handle().clone());
            let inner = AppInner::new(data_dir.clone(), events)?;
            app.manage(AppState::new(inner));
            tracing::info!(target: "app", "数据目录：{}", data_dir.display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_commands::app_info,
            commands::app_commands::app_ready,
            commands::app_commands::app_log_path,
            commands::app_commands::workspace_root_path,
            commands::workspace_commands::workspace_create,
            commands::workspace_commands::workspace_open,
            commands::workspace_commands::workspace_switch,
            commands::workspace_commands::workspace_close,
            commands::workspace_commands::workspace_get_current,
            commands::workspace_commands::workspace_list,
            commands::workspace_commands::workspace_stats,
            commands::workspace_commands::workspace_scan,
            commands::workspace_commands::workspace_rebuild_index,
            commands::workspace_commands::workspace_forget,
            commands::workspace_commands::workspace_active_id,
            commands::workspace_commands::settings_get,
            commands::workspace_commands::settings_update,
            commands::note_commands::note_create,
            commands::note_commands::note_get,
            commands::note_commands::note_save,
            commands::note_commands::note_update,
            commands::note_commands::note_list,
            commands::note_commands::note_recent,
            commands::note_commands::note_list_deleted,
            commands::note_commands::note_list_missing,
            commands::note_commands::note_delete,
            commands::note_commands::note_restore,
            commands::note_commands::note_purge,
            commands::note_commands::note_rename,
            commands::note_commands::note_move,
            commands::note_commands::note_duplicate,
            commands::note_commands::note_set_favorite,
            commands::note_commands::note_set_tags,
            commands::note_commands::note_count,
            commands::note_commands::note_import,
            commands::note_commands::note_export,
            commands::category_commands::category_list,
            commands::category_commands::category_tree,
            commands::category_commands::category_create,
            commands::category_commands::category_update,
            commands::category_commands::category_move,
            commands::category_commands::category_delete,
            commands::tag_commands::tag_list,
            commands::tag_commands::tag_create,
            commands::tag_commands::tag_rename,
            commands::tag_commands::tag_delete,
            commands::tag_commands::note_set_tag_names,
            commands::search_commands::search_notes,
            commands::search_commands::search_quick,
            commands::search_commands::search_rebuild_index,
            commands::search_commands::search_get_status,
            commands::link_commands::links_get_backlinks,
            commands::link_commands::links_get_outgoing,
            commands::link_commands::links_get_unresolved,
            commands::link_commands::links_get_counts,
            commands::link_commands::links_rebuild,
            commands::backup_commands::backup_create,
            commands::backup_commands::backup_list,
            commands::backup_commands::backup_validate,
            commands::backup_commands::backup_restore,
            commands::backup_commands::export_markdown,
            commands::backup_commands::backup_directory,
            commands::attachment_commands::attachment_save,
            commands::attachment_commands::attachment_read,
            commands::template_commands::templates_list,
            commands::template_commands::template_render,
        ])
        .run(tauri::generate_context!())
        .expect("Personal Knowledge OS 启动失败");
}

/// 供集成测试复用：构建没有 Tauri 运行时的应用上下文。
pub fn build_app_inner(
    data_dir: std::path::PathBuf,
    events: std::sync::Arc<dyn crate::core::events::EventSink>,
) -> core::error::Result<std::sync::Arc<AppInner>> {
    AppInner::new(data_dir, events)
}
