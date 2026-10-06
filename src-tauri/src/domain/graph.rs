//! 知识图谱领域模型（V1.1 M3）。
//!
//! 图谱是**派生视图**：节点来自 `notes`，边来自 `note_links`，
//! 不做额外存储；任何过滤/截断都只影响本次返回结果。

use serde::{Deserialize, Serialize};

/// 图谱查询条件。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphQuery {
    /// 只看某个分类（含子分类）
    #[serde(default)]
    pub category_id: Option<String>,
    /// 只看带某个标签的笔记
    #[serde(default)]
    pub tag_id: Option<String>,
    /// 只看标题/正文包含关键词的笔记
    #[serde(default)]
    pub keyword: Option<String>,
    /// 聚焦某篇笔记（配合 `hops` 做 N 跳邻域）
    #[serde(default)]
    pub focus_note_id: Option<String>,
    /// 聚焦跳数：1 或 2（缺省 1）
    #[serde(default)]
    pub hops: Option<u32>,
    /// 是否包含孤立节点（没有出链也没有反链）
    #[serde(default)]
    pub include_orphans: Option<bool>,
    /// 只保留该时间之后更新的笔记（ISO 8601）
    #[serde(default)]
    pub updated_after: Option<String>,
    /// 最多返回节点数（缺省 2000）
    #[serde(default)]
    pub max_nodes: Option<usize>,
    /// 最多返回边数（缺省 5000）
    #[serde(default)]
    pub max_edges: Option<usize>,
}

/// 图谱节点（一篇笔记）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub title: String,
    pub category_id: Option<String>,
    pub updated_at: String,
    /// 在**本次返回结果**中的邻居数（出链 + 入链去重；同一对笔记多条链接算一个邻居）
    pub degree: u32,
    pub tags: Vec<String>,
    /// 没有连接任何节点
    pub orphan: bool,
}

/// 图谱边（一条链接，按「来源 → 目标」去重）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    /// `wikilink` / `markdown`
    pub link_type: String,
    /// 同一对笔记之间的链接条数（用于加粗）
    pub weight: u32,
}

/// 图谱数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    /// 因超出上限被截断
    pub truncated: bool,
    /// 过滤后、截断前的节点/边总数
    pub total_nodes: usize,
    pub total_edges: usize,
    pub duration_ms: u64,
}
