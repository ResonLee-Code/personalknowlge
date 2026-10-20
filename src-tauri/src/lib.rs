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
            #[cfg(target_os = "macos")]
            {
                configure_macos_window(app);
                relabel_macos_fullscreen_menu(app);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_commands::app_info,
            commands::app_commands::app_ready,
            commands::app_commands::app_log_path,
            commands::app_commands::app_log,
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
            commands::note_commands::note_set_source,
            commands::note_commands::note_set_parent,
            commands::note_commands::notes_reorder,
            commands::note_commands::note_set_pinned,
            commands::note_commands::note_history_list,
            commands::note_commands::note_history_read,
            commands::note_commands::note_history_restore,
            commands::note_commands::note_history_remove,
            commands::note_commands::note_history_save_version,
            commands::note_commands::note_history_label,
            commands::clip_commands::note_clip_url,
            commands::ai_commands::ai_policy_get,
            commands::ai_commands::ai_policy_set,
            commands::ai_commands::ai_audit_tail,
            commands::ai_commands::ai_model_presets,
            commands::ai_commands::ai_diagnose,
            commands::ai_commands::ai_retrieval_search,
            commands::ai_commands::ai_vector_index_status,
            commands::ai_commands::ai_decision_probe,
            commands::ai_commands::ai_vector_index_preview,
            commands::ai_commands::ai_vector_index_build,
            commands::ai_commands::ai_vector_index_clear,
            commands::ai_commands::ai_kb_search,
            commands::ai_commands::ai_web_fetch,
            commands::ai_commands::ai_web_search,
            commands::research_commands::research_start,
            commands::research_commands::research_get,
            commands::research_commands::research_list,
            commands::research_commands::research_collect,
            commands::research_commands::research_verify,
            commands::research_commands::research_compose,
            commands::research_commands::research_outbound_preview,
            commands::research_commands::research_proposal,
            commands::research_commands::research_abort,
            commands::research_commands::research_write,
            commands::batch_commands::notes_move_batch,
            commands::batch_commands::notes_tag_batch,
            commands::batch_commands::notes_delete_batch,
            commands::note_commands::note_count,
            commands::note_commands::note_due_for_review,
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
            commands::graph_commands::graph_get,
            commands::relation_commands::relations_list,
            commands::relation_commands::relations_add,
            commands::relation_commands::relations_remove,
            commands::relation_commands::relations_rebuild,
            commands::backup_commands::backup_create,
            commands::backup_commands::backup_list,
            commands::backup_commands::backup_validate,
            commands::backup_commands::backup_restore,
            commands::backup_commands::export_markdown,
            commands::backup_commands::backup_directory,
            commands::attachment_commands::attachment_save,
            commands::attachment_commands::attachment_read,
            commands::attachment_commands::attachment_list,
            commands::attachment_commands::attachment_delete,
            commands::attachment_commands::attachment_references,
            commands::template_commands::templates_list,
            commands::template_commands::template_render,
            commands::template_commands::template_save,
        ])
        .run(tauri::generate_context!())
        .expect("Personal Knowledge OS 启动失败");
}

/// macOS：禁用“原生全屏”，让绿色按钮执行系统**缩放**（zoom）。
///
/// 背景（2026-10-19）：原生全屏会把标题栏与系统红黄绿一起藏起来，鼠标移到顶部也唤不出；
/// 自绘一套按钮又被指出“像外挂上去的”。最终选择：不再进原生全屏——把窗口的
/// collection behavior 设为 `FullScreenNone`，绿色按钮改为 macOS 原生“缩放”：
/// 窗口铺满可用区域，但标题栏、菜单栏和系统红黄绿全部保留。
#[cfg(target_os = "macos")]
fn configure_macos_window(app: &tauri::App) {
    use std::ffi::CStr;
    use std::sync::OnceLock;

    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::{class, msg_send, sel};
    use tauri::Manager;

    let Some(window) = app.get_webview_window("main") else {
        tracing::warn!(target: "app", "未找到主窗口，跳过 macOS 全屏配置");
        return;
    };
    match window.ns_window() {
        Ok(pointer) => {
            extern "C-unwind" fn toggle_full_screen_as_zoom(
                this: &mut AnyObject,
                _cmd: Sel,
                _sender: *mut AnyObject,
            ) {
                // SAFETY: 由 AppKit 在主线程调用；zoom: 是 NSWindow 的标准方法。
                let _: () = unsafe { msg_send![this, zoom: std::ptr::null_mut::<AnyObject>()] };
            }

            // 不要用 object_setClass 换窗口类：实测会让 AppKit 在关闭窗口
            // （-[NSView removeFromSuperview]）时抛 ObjC 异常并 SIGABRT。
            // 改为只替换 NSWindow 的 toggleFullScreen: 实现，窗口类保持不变。
            static ZOOM_REDIRECT: OnceLock<()> = OnceLock::new();
            ZOOM_REDIRECT.get_or_init(|| {
                unsafe {
                    let types = CStr::from_bytes_with_nul(b"v@:@\0").expect("method types");
                    let imp: Imp = std::mem::transmute(
                        toggle_full_screen_as_zoom
                            as extern "C-unwind" fn(&mut AnyObject, Sel, *mut AnyObject),
                    );
                    objc2::ffi::class_replaceMethod(
                        class!(NSWindow) as *const AnyClass as *mut AnyClass,
                        sel!(toggleFullScreen:),
                        imp,
                        types.as_ptr(),
                    );
                }
            });

            let _ = pointer;
            tracing::info!(
                target: "app",
                "macOS 绿色按钮保留系统全屏箭头外观，但 toggleFullScreen: 已重定向为 zoom:"
            );
        }
        Err(error) => {
            tracing::warn!(target: "app", "读取 macOS 窗口指针失败：{error}");
        }
    }
}

/// 原生全屏已被禁用，菜单里的 “Toggle Full Screen” 实际执行的是系统缩放。
/// 把标签改成 “Zoom”，避免用户以为还能进全屏。
#[cfg(target_os = "macos")]
fn relabel_macos_fullscreen_menu(app: &tauri::App) {
    let Some(menu) = app.menu() else {
        return;
    };
    let Ok(items) = menu.items() else {
        return;
    };
    for item in items {
        let Some(submenu) = item.as_submenu() else {
            continue;
        };
        let Ok(sub_items) = submenu.items() else {
            continue;
        };
        for sub in sub_items {
            let text = sub
                .as_menuitem()
                .and_then(|entry| entry.text().ok())
                .or_else(|| {
                    sub.as_predefined_menuitem()
                        .and_then(|entry| entry.text().ok())
                });
            if text.as_deref() != Some("Toggle Full Screen") {
                continue;
            }
            if let Some(entry) = sub.as_menuitem() {
                let _ = entry.set_text("Zoom");
            }
            if let Some(entry) = sub.as_predefined_menuitem() {
                let _ = entry.set_text("Zoom");
            }
        }
    }
}

/// 供集成测试复用：构建没有 Tauri 运行时的应用上下文。
pub fn build_app_inner(
    data_dir: std::path::PathBuf,
    events: std::sync::Arc<dyn crate::core::events::EventSink>,
) -> core::error::Result<std::sync::Arc<AppInner>> {
    AppInner::new(data_dir, events)
}
