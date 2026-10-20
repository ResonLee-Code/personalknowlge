//! Platform 2.0：签名更新检查、下载、安装与重启。
//!
//! 更新公钥在发布构建时通过 `PKOS_UPDATER_PUBKEY` 注入；缺失时命令明确拒绝，
//! 不会退化成“无签名下载”。下载后的签名验证由 Tauri updater 完成，失败时不会安装。

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, Updater, UpdaterExt};

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::events;

const DEFAULT_ENDPOINT: &str =
    "https://github.com/ResonLee-Code/personalknowlge/releases/latest/download/latest.json";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub version: String,
    pub date: Option<String>,
    pub body: Option<String>,
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInstallResult {
    pub installed: bool,
    pub version: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    stage: String,
    downloaded: u64,
    content_length: Option<u64>,
}

fn updater(app: &AppHandle) -> Result<Updater> {
    let pubkey = option_env!("PKOS_UPDATER_PUBKEY")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::InvalidInput,
                "当前构建未配置更新公钥。发布构建必须注入 PKOS_UPDATER_PUBKEY，不能使用无签名更新。",
            )
        })?;
    let endpoint = option_env!("PKOS_UPDATER_ENDPOINT")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_ENDPOINT);
    let endpoint = endpoint.parse::<tauri::Url>().map_err(|error| {
        AppError::new(ErrorCode::InvalidInput, format!("更新地址不合法：{error}"))
    })?;
    app.updater_builder()
        .pubkey(pubkey)
        .endpoints(vec![endpoint])
        .map_err(update_error)?
        .build()
        .map_err(update_error)
}

fn update_error(error: tauri_plugin_updater::Error) -> AppError {
    AppError::new(ErrorCode::InvalidInput, format!("更新流程失败：{error}"))
}

fn update_info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        date: update.date.as_ref().map(ToString::to_string),
        body: update.body.clone(),
        target: update.target.clone(),
    }
}

/// 只检查更新，不下载、不安装。
#[tauri::command]
pub async fn app_update_check(app: AppHandle) -> Result<Option<UpdateInfo>> {
    let updater = updater(&app)?;
    updater
        .check()
        .await
        .map(|update| update.as_ref().map(update_info))
        .map_err(update_error)
}

/// 重新检查并下载安装。签名不匹配或安装失败都返回错误；旧版本进程不会被成功结果覆盖。
#[tauri::command]
pub async fn app_update_install(app: AppHandle) -> Result<UpdateInstallResult> {
    let updater = updater(&app)?;
    let update = updater
        .check()
        .await
        .map_err(update_error)?
        .ok_or_else(|| AppError::new(ErrorCode::InvalidInput, "当前已经是最新版本。"))?;
    let version = update.version.clone();
    let event_app = app.clone();
    let mut downloaded = 0u64;
    update
        .download_and_install(
            move |chunk_length, content_length| {
                downloaded = downloaded.saturating_add(chunk_length as u64);
                let _ = event_app.emit(
                    events::UPDATE_PROGRESS,
                    UpdateProgress {
                        stage: "downloading".to_string(),
                        downloaded,
                        content_length,
                    },
                );
            },
            || {
                let _ = app.emit(
                    events::UPDATE_PROGRESS,
                    UpdateProgress {
                        stage: "verifying".to_string(),
                        downloaded,
                        content_length: None,
                    },
                );
            },
        )
        .await
        .map_err(update_error)?;
    Ok(UpdateInstallResult {
        installed: true,
        version,
        message: "更新包已通过签名校验并安装；请重启应用以运行新版本。".to_string(),
    })
}

/// 更新安装完成后由用户触发重启。
#[tauri::command]
pub async fn app_restart(app: AppHandle) -> Result<()> {
    app.restart();
}
