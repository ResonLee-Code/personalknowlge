//! 应用级命令：版本信息、打开路径、日志位置。

use serde::Serialize;
use tauri::State;

use crate::core::error::Result;
use crate::core::logging;
use crate::state::AppState;

use super::run;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub schema_version: i64,
    pub data_dir: String,
    pub platform: String,
    pub sqlite_version: String,
}

#[tauri::command]
pub async fn app_info(state: State<'_, AppState>) -> Result<AppInfo> {
    let app = state.inner.clone();
    run(move || {
        let sqlite_version = app
            .active()
            .map(|context| {
                context
                    .db
                    .with_conn(|conn| {
                        let version: String =
                            conn.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
                        Ok(version)
                    })
                    .unwrap_or_else(|_| "unknown".to_string())
            })
            .unwrap_or_else(|| "unknown".to_string());
        Ok(AppInfo {
            name: "Personal Knowledge OS".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version: crate::infrastructure::db::SCHEMA_VERSION,
            data_dir: app.data_dir.to_string_lossy().to_string(),
            platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            sqlite_version,
        })
    })
    .await
}

/// 工作区根目录（用于「打开目录」）。
#[tauri::command]
pub async fn workspace_root_path(state: State<'_, AppState>) -> Result<String> {
    let context = state.inner.context()?;
    Ok(context.workspace.root_path.clone())
}

/// 日志文件路径。
#[tauri::command]
pub async fn app_log_path(state: State<'_, AppState>) -> Result<Option<String>> {
    let context = state.inner.active();
    let path = context
        .map(|context| context.paths.log_dir().join("app.log"))
        .or_else(logging::log_file_path);
    Ok(path.map(|path| path.to_string_lossy().to_string()))
}

/// 前端启动完成后的握手，返回当前工作区（若已存在）。
#[tauri::command]
pub async fn app_ready(state: State<'_, AppState>) -> Result<bool> {
    let _ = state.inner.active();
    Ok(true)
}

/// 单条前端日志的最大长度（字符）。超长一律截断：这条通道只用来记失败原因，
/// 不是正文通道（历史上出现过把笔记内容写进日志的隐患，这里从入口就掐住）。
const MAX_UI_LOG_CHARS: usize = 500;

fn normalize_ui_log_level(level: &str) -> &'static str {
    match level.trim().to_ascii_lowercase().as_str() {
        "error" => "error",
        "warn" | "warning" => "warn",
        "debug" => "debug",
        _ => "info",
    }
}

fn cap_ui_log_message(message: &str) -> String {
    // `truncate_chars` 会在截断后补一个省略号，所以这里少给一个字，保证最终不超过上限。
    crate::core::text::truncate_chars(message.trim(), MAX_UI_LOG_CHARS.saturating_sub(1))
}

/// 前端把界面上看得见的失败写进同一份应用日志（Track B3 / E4）。
///
/// 为什么要它：保存 / 导出 / 恢复 / 扫描这些失败以前只弹个 toast 就没了，
/// 用户报障时手上只有界面描述、没有日志可查。现在前端把同一条原因写进应用日志，
/// 与 Rust 侧共用一个文件（`…/logs/app.log`），排障时按时间一搜就能对上。
#[tauri::command]
pub async fn app_log(level: String, scope: String, message: String) -> Result<()> {
    let level = normalize_ui_log_level(&level);
    let scope = scope.trim().to_string();
    let message = cap_ui_log_message(&message);
    match level {
        "error" => tracing::error!(target: "ui", "[{}] {}", scope, message),
        "warn" => tracing::warn!(target: "ui", "[{}] {}", scope, message),
        "debug" => tracing::debug!(target: "ui", "[{}] {}", scope, message),
        _ => tracing::info!(target: "ui", "[{}] {}", scope, message),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_log_levels_fall_back_to_info() {
        assert_eq!(normalize_ui_log_level("ERROR"), "error");
        assert_eq!(normalize_ui_log_level(" warning "), "warn");
        assert_eq!(normalize_ui_log_level("debug"), "debug");
        assert_eq!(normalize_ui_log_level("info"), "info");
        // 未知 / 空白一律按 info：不让前端因为拼错级别就报错
        assert_eq!(normalize_ui_log_level(""), "info");
        assert_eq!(normalize_ui_log_level("fatal"), "info");
    }

    #[test]
    fn ui_log_message_is_truncated_not_dumped() {
        let long = "错".repeat(MAX_UI_LOG_CHARS + 200);
        let trimmed = cap_ui_log_message(&long);
        assert!(
            trimmed.chars().count() <= MAX_UI_LOG_CHARS,
            "超长消息必须截断，实际 {} 字",
            trimmed.chars().count()
        );
        assert!(trimmed.ends_with('…'), "截断后要能看出被截断过");
        // 正常长度的原因原样保留（不要把有用的信息吃掉）
        assert_eq!(cap_ui_log_message("  磁盘只读  "), "磁盘只读");
    }
}
