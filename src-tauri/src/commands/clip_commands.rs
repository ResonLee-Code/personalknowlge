//! 网页剪藏命令（V1.2.1 阶段 C1）。

use crate::application::clip_service::{clip, ClippedPage};
use crate::core::error::Result;

use super::run;

/// 抓取一个公开网页并转成 Markdown（**只读**，不写库）。
///
/// 写入仍由前端走常规「新建笔记」路径，保持单一写入路径与 frontmatter 约定。
#[tauri::command]
pub async fn note_clip_url(url: String) -> Result<ClippedPage> {
    run(move || clip(&url)).await
}
