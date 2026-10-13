//! 研究助手命令（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §9）。
//!
//! 这一层**只做转发**：闸门在 `application::ai::policy`，流程在 `application::ai::research`。
//! 五步各自是一个命令，便于 UI 逐步推进、显示进度、随时中止。

use tauri::State;

use crate::application::ai::research::ResearchService;
use crate::core::error::Result;
use crate::domain::research::{
    ResearchOutboundPreview, ResearchRequest, ResearchSession, ResearchSessionSummary,
    ResearchWriteInput, ResearchWriteResult,
};
use crate::state::AppState;

use super::run;

/// ① 立项：课题 → 研究计划（子问题 + 本地基线）。
#[tauri::command]
pub async fn research_start(
    state: State<'_, AppState>,
    input: ResearchRequest,
) -> Result<ResearchSession> {
    let context = state.inner.context()?;
    run(move || ResearchService::new(context).start(input)).await
}

/// 读取一个会话（刷新页面后恢复现场）。
#[tauri::command]
pub async fn research_get(state: State<'_, AppState>, session_id: String) -> Result<ResearchSession> {
    let context = state.inner.context()?;
    run(move || ResearchService::new(context).get(&session_id)).await
}

/// 最近的会话（新的在前）。
#[tauri::command]
pub async fn research_list(
    state: State<'_, AppState>,
    limit: usize,
) -> Result<Vec<ResearchSessionSummary>> {
    let context = state.inner.context()?;
    run(move || ResearchService::new(context).list(limit)).await
}

/// ② 收集：本地 FTS5 + 可选外部检索。
///
/// `confirmed` 表示**本次允许外发**：既用于联网检索（闸门二），也用于在线模型打分
/// （Track C2 的逐次确认）——一个调用只有一次"用户已确认这一次"的机会。
#[tauri::command]
pub async fn research_collect(
    state: State<'_, AppState>,
    session_id: String,
    confirmed: bool,
) -> Result<ResearchSession> {
    let context = state.inner.context()?;
    run(move || ResearchService::new(context).collect(&session_id, confirmed)).await
}

/// ③ 验证：≥2 个独立来源才算交叉验证。
///
/// `confirm_outbound`（Track C2/C5）：冲突判定可能调用在线模型；为 `false` 时在线档
/// **不参与**（零请求），冲突判定回退本地模型 / 规则档。
#[tauri::command]
pub async fn research_verify(
    state: State<'_, AppState>,
    session_id: String,
    confirm_outbound: Option<bool>,
) -> Result<ResearchSession> {
    let context = state.inner.context()?;
    run(move || {
        ResearchService::new(context).verify(&session_id, confirm_outbound.unwrap_or(false))
    })
    .await
}

/// ④ 汇总：渲染固定六段报告。
///
/// `confirm_outbound`（Track C2）：开启「逐次外发确认」时，为 `true` 才允许把片段交给
/// 在线模型；为 `false` 时在线档**不参与**（零请求），自动降级到本地 / 模板。
#[tauri::command]
pub async fn research_compose(
    state: State<'_, AppState>,
    session_id: String,
    confirm_outbound: Option<bool>,
) -> Result<ResearchSession> {
    let context = state.inner.context()?;
    run(move || {
        ResearchService::new(context).compose(&session_id, confirm_outbound.unwrap_or(false))
    })
    .await
}

/// 逐次外发确认的预览（Track C2）：**将要发送的片段 + 目标主机**；只读，不发请求。
#[tauri::command]
pub async fn research_outbound_preview(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<ResearchOutboundPreview> {
    let context = state.inner.context()?;
    run(move || ResearchService::new(context).outbound_preview(&session_id)).await
}

/// ⑤ 写入提案（纯建议，不落库；落库走 `research_write` 并由用户确认）。
#[tauri::command]
pub async fn research_proposal(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<ResearchSession> {
    let context = state.inner.context()?;
    run(move || ResearchService::new(context).proposal(&session_id)).await
}

/// ⑤ 落库：**用户确认后**把报告写入知识库（默认 Inbox、同标题不覆盖、可回滚）。
#[tauri::command]
pub async fn research_write(
    state: State<'_, AppState>,
    input: ResearchWriteInput,
) -> Result<ResearchWriteResult> {
    let context = state.inner.context()?;
    run(move || ResearchService::new(context).write(input)).await
}

/// 中止会话（已得材料保留为草稿）。
#[tauri::command]
pub async fn research_abort(
    state: State<'_, AppState>,
    session_id: String,
    reason: Option<String>,
) -> Result<ResearchSession> {
    let context = state.inner.context()?;
    run(move || ResearchService::new(context).abort(&session_id, reason)).await
}
