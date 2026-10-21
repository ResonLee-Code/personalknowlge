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

/// Graph 2.0 洞察查询。
///
/// 洞察只在当前图谱抽样子图上计算，避免 10 万库上做全量两两比较。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphInsightsQuery {
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub tag_id: Option<String>,
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub updated_after: Option<String>,
    /// 以哪篇笔记为中心找近邻；为空时仍返回重复、桥梁与孤岛结构洞察。
    #[serde(default)]
    pub focus_note_id: Option<String>,
    /// 图谱抽样上限（1..=5000，缺省 2000）。
    #[serde(default)]
    pub max_nodes: Option<usize>,
    /// 近邻数量（1..=20，缺省 10）。
    #[serde(default)]
    pub neighbor_limit: Option<usize>,
    /// 近重复阈值（0.6..=0.99，缺省 0.82）。
    #[serde(default)]
    pub duplicate_threshold: Option<f64>,
}

/// 一条可解释的连接原因。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEvidence {
    /// `reference` / `sharedReference` / `sharedTag` / `sharedTopic` / `similarContent`
    pub kind: String,
    /// 面向用户的一句话原因。
    pub label: String,
    /// 证据涉及的其它笔记 id（用于界面展开与跳转）。
    pub note_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticNeighbor {
    pub note_id: String,
    pub title: String,
    pub category_id: Option<String>,
    /// 0..=1；结构证据可加分，但最终会夹取到 1。
    pub score: f64,
    pub evidence: Vec<GraphEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateMember {
    pub note_id: String,
    pub title: String,
    pub excerpt: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCluster {
    pub id: String,
    /// 簇内最相近的一对笔记的分数。
    pub similarity: f64,
    pub reason: String,
    pub members: Vec<DuplicateMember>,
    /// 只做预览；真正合并必须用户在界面二次确认。
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeNote {
    pub note_id: String,
    pub title: String,
    pub category_id: Option<String>,
    pub connected_categories: usize,
    pub bridge_score: f64,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IslandCategory {
    pub category_id: String,
    pub category_path: String,
    pub note_count: usize,
    pub internal_edges: usize,
    pub external_edges: usize,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphInsights {
    /// `vector` = 使用本地已建向量；`local` = 标题/分块词项与结构证据。
    pub mode: String,
    pub neighbors: Vec<SemanticNeighbor>,
    pub duplicate_clusters: Vec<DuplicateCluster>,
    pub orphan_notes: Vec<GraphNode>,
    pub bridge_notes: Vec<BridgeNote>,
    pub island_categories: Vec<IslandCategory>,
    pub sampled_nodes: usize,
    pub total_nodes: usize,
    pub truncated: bool,
    pub warnings: Vec<String>,
    pub duration_ms: u64,
}
