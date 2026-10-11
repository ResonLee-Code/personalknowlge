//! AI 策略 / 工具命令（V1.2.1 阶段 D1）。
//!
//! 这一层**只做转发**：闸门在 `application::ai::policy`，工具在 `application::ai::tools`。

use tauri::State;

use crate::application::ai::policy::AiPolicyService;
use crate::application::ai::tools::{AiTools, KbHit};
use crate::application::clip_service::ClippedPage;
use crate::core::error::Result;
use crate::domain::ai::{AiAuditEntry, AiPolicy, UpdateAiPolicyInput};
use crate::infrastructure::credential_store;
use crate::state::AppState;

use super::run;

/// 读取当前 AI 策略（默认最保守）。
#[tauri::command]
pub async fn ai_policy_get(state: State<'_, AppState>) -> Result<AiPolicy> {
    let context = state.inner.context()?;
    run(move || Ok(AiPolicyService::new(context).policy())).await
}

/// 更新 AI 策略（总开关 / 联网档位 / 抓取正文 / 白黑名单 / 本地与在线模型 / 预算）。
///
/// **API Key 只写不读**：`remote_model_api_key` 在这里落到应用数据目录的凭据文件，
/// 不进工作区设置库（否则备份包、导出的 Markdown 会把 Key 一起带走）；
/// 响应里只用 `remote_model_key_set` 回显「配没配」。
#[tauri::command]
pub async fn ai_policy_set(
    state: State<'_, AppState>,
    input: UpdateAiPolicyInput,
) -> Result<AiPolicy> {
    let context = state.inner.context()?;
    let data_dir = state.inner.data_dir.clone();
    run(move || {
        if let Some(key) = input.remote_model_api_key.clone() {
            credential_store::set_remote_model_key(&data_dir, Some(&key))?;
        }
        AiPolicyService::new(context).update(input)
    })
    .await
}

/// 最近的行为审计（新的在前；不含正文）。
#[tauri::command]
pub async fn ai_audit_tail(state: State<'_, AppState>, limit: usize) -> Result<Vec<AiAuditEntry>> {
    let context = state.inner.context()?;
    run(move || AiPolicyService::new(context).audit_tail(limit)).await
}

/// `kb_search`：本地检索（复用 FTS5，不联网）。
#[tauri::command]
pub async fn ai_kb_search(
    state: State<'_, AppState>,
    keyword: String,
    limit: usize,
) -> Result<Vec<KbHit>> {
    let context = state.inner.context()?;
    run(move || AiTools::new(context).kb_search(&keyword, limit)).await
}

/// `web_fetch`：抓取网页（必须已开启 AI + 联网档位 + 抓取正文，并带单次确认）。
#[tauri::command]
pub async fn ai_web_fetch(
    state: State<'_, AppState>,
    url: String,
    confirmed: bool,
) -> Result<ClippedPage> {
    let context = state.inner.context()?;
    run(move || AiTools::new(context).web_fetch(&url, confirmed)).await
}

/// `web_search`：联网搜索（D3 接入；当前如实返回未接入，不伪造结果）。
#[tauri::command]
pub async fn ai_web_search(
    state: State<'_, AppState>,
    keyword: String,
    limit: usize,
    confirmed: bool,
) -> Result<Vec<super::super::application::ai::web_search::WebSearchHit>> {
    let context = state.inner.context()?;
    run(move || AiTools::new(context).web_search(&keyword, limit, confirmed)).await
}
