//! 网页剪藏命令（V1.2.1 阶段 C1）。

use crate::application::clip_service::{clip_with_proxy, ClippedPage};
use crate::application::ai::policy::AiPolicyService;
use crate::core::error::Result;
use crate::state::AppState;
use tauri::State;

use super::run;

/// 抓取一个公开网页并转成 Markdown（**只读**，不写库）。
///
/// 写入仍由前端走常规「新建笔记」路径，保持单一写入路径与 frontmatter 约定。
#[tauri::command]
pub async fn note_clip_url(state: State<'_, AppState>, url: String) -> Result<ClippedPage> {
    // 没开工作区时读不到设置：退化为"跟随环境变量 / 系统代理"
    let proxy = state
        .inner
        .context()
        .ok()
        .and_then(|context| AiPolicyService::new(context).outbound_proxy());
    run(move || clip_with_proxy(&url, proxy.as_deref())).await
}
