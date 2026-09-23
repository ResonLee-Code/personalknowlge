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
