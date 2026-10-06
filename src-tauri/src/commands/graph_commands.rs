//! 知识图谱命令（V1.1 M3）。

use tauri::State;

use crate::application::graph_service::GraphService;
use crate::core::error::Result;
use crate::domain::graph::{GraphData, GraphQuery};
use crate::state::AppState;

use super::run;

/// 查询知识图谱（节点 + 边 + 过滤 + 截断标记）。
#[tauri::command]
pub async fn graph_get(state: State<'_, AppState>, query: GraphQuery) -> Result<GraphData> {
    let context = state.inner.context()?;
    run(move || GraphService::new(context).graph(query)).await
}
