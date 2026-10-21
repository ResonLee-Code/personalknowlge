//! 知识图谱命令（V1.1 M3）。

use tauri::State;

use crate::application::graph_insights::GraphInsightsService;
use crate::application::graph_service::GraphService;
use crate::core::error::Result;
use crate::domain::graph::{GraphData, GraphInsights, GraphInsightsQuery, GraphQuery};
use crate::state::AppState;

use super::run;

/// 查询知识图谱（节点 + 边 + 过滤 + 截断标记）。
#[tauri::command]
pub async fn graph_get(state: State<'_, AppState>, query: GraphQuery) -> Result<GraphData> {
    let context = state.inner.context()?;
    run(move || GraphService::new(context).graph(query)).await
}

/// Graph 2.0：可解释近邻、近重复、桥梁、孤儿与孤岛分类。
#[tauri::command]
pub async fn graph_insights(
    state: State<'_, AppState>,
    query: GraphInsightsQuery,
) -> Result<GraphInsights> {
    let context = state.inner.context()?;
    run(move || GraphInsightsService::new(context).insights(query)).await
}
