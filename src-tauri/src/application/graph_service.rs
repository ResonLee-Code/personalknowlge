//! 知识图谱查询（V1.1 M3）。
//!
//! 图谱不落库：节点取自 `notes`（沿用列表查询的过滤能力），边取自 `note_links`
//! 中**已解析**的链接，再按聚焦跳数、孤立节点开关与容量上限裁剪。
//! 这样图谱永远和正文/反链面板一致，`links_rebuild` 即可整体修复。

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use crate::domain::graph::{GraphData, GraphEdge, GraphNode, GraphQuery};
use crate::domain::note::{NoteListQuery, NoteStatus};
use crate::repositories::tag_repository::tag_names_for_notes;
use crate::repositories::{
    LinkRepository, NoteRepository, SqliteLinkRepository, SqliteNoteRepository,
};

use super::context::WorkspaceContext;

/// 默认节点上限（超出时按度数优先保留，并在界面提示先筛选）。
pub const DEFAULT_MAX_NODES: usize = 2000;
/// 默认边上限。
pub const DEFAULT_MAX_EDGES: usize = 5000;
/// 聚焦跳数上限。
const MAX_HOPS: u32 = 3;

pub struct GraphService {
    ctx: Arc<WorkspaceContext>,
}

impl GraphService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    pub fn graph(&self, query: GraphQuery) -> crate::core::error::Result<GraphData> {
        let started = Instant::now();
        let workspace_id = self.ctx.workspace.id.clone();

        // 1) 节点候选：复用笔记列表的过滤（分类含子分类、标签、关键词）
        let list_query = NoteListQuery {
            category_id: query.category_id.clone(),
            tag_id: query.tag_id.clone(),
            keyword: query.keyword.clone(),
            status: Some(NoteStatus::Active),
            ..Default::default()
        };
        // 仓储单页上限 500，图谱需要完整节点集 → 按页取全部
        let mut notes = Vec::new();
        let mut page = 1i64;
        loop {
            let mut paged = list_query.clone();
            paged.page = Some(page);
            let (batch, total) = self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.list(conn, &workspace_id, &paged)
            })?;
            let batch_len = batch.len();
            notes.extend(batch);
            if batch_len == 0 || notes.len() as i64 >= total || page >= 100 {
                break;
            }
            page += 1;
        }

        let updated_after = query.updated_after.clone();
        let candidates: Vec<_> = notes
            .into_iter()
            .filter(|note| match &updated_after {
                Some(threshold) => note.updated_at.as_str() >= threshold.as_str(),
                None => true,
            })
            .collect();

        // 2) 边：全库已解析链接，只保留两端都在候选集合内的
        let allowed: HashSet<String> = candidates.iter().map(|note| note.id.clone()).collect();
        let links = self
            .ctx
            .db
            .with_conn(|conn| SqliteLinkRepository.list_resolved(conn, &workspace_id))?;

        let mut edge_map: BTreeMap<(String, String), (String, u32)> = BTreeMap::new();
        let mut adjacency: HashMap<String, BTreeSet<String>> = HashMap::new();
        for link in links {
            let Some(target) = link.target_note_id.clone() else {
                continue;
            };
            if !allowed.contains(&link.source_note_id) || !allowed.contains(&target) {
                continue;
            }
            if link.source_note_id == target {
                // 自链接不计入图谱（避免自环影响布局与度数）
                continue;
            }
            let key = (link.source_note_id.clone(), target.clone());
            let entry = edge_map
                .entry(key)
                .or_insert_with(|| (link.link_type.as_str().to_string(), 0));
            entry.1 += 1;
            adjacency
                .entry(link.source_note_id.clone())
                .or_default()
                .insert(target.clone());
            adjacency
                .entry(target)
                .or_default()
                .insert(link.source_note_id.clone());
        }

        // 3) 聚焦：以目标笔记为起点做 N 跳 BFS
        let mut focus: Option<HashSet<String>> = None;
        if let Some(focus_id) = query.focus_note_id.as_deref().filter(|id| !id.is_empty()) {
            let hops = query.hops.unwrap_or(1).clamp(1, MAX_HOPS);
            if allowed.contains(focus_id) {
                let mut visited: HashSet<String> = HashSet::new();
                let mut frontier: Vec<String> = vec![focus_id.to_string()];
                visited.insert(focus_id.to_string());
                for _ in 0..hops {
                    let mut next = Vec::new();
                    for node in &frontier {
                        if let Some(neighbors) = adjacency.get(node) {
                            for neighbor in neighbors {
                                if visited.insert(neighbor.clone()) {
                                    next.push(neighbor.clone());
                                }
                            }
                        }
                    }
                    if next.is_empty() {
                        break;
                    }
                    frontier = next;
                }
                focus = Some(visited);
            }
        }

        // 4) 组装节点（可先过滤孤立节点），再按度数截断
        let include_orphans = query.include_orphans.unwrap_or(true);
        let mut nodes: Vec<GraphNode> = candidates
            .into_iter()
            .filter(|note| focus.as_ref().is_none_or(|set| set.contains(&note.id)))
            .map(|note| {
                let degree = adjacency.get(&note.id).map(|set| set.len() as u32).unwrap_or(0);
                GraphNode {
                    id: note.id.clone(),
                    title: note.title.clone(),
                    category_id: note.category_id.clone(),
                    updated_at: note.updated_at.clone(),
                    degree,
                    tags: Vec::new(),
                    orphan: degree == 0,
                }
            })
            .filter(|node| include_orphans || !node.orphan)
            .collect();

        let total_nodes = nodes.len();
        let max_nodes = query.max_nodes.unwrap_or(DEFAULT_MAX_NODES).max(1);
        let mut truncated = false;
        if nodes.len() > max_nodes {
            nodes.sort_by(|left, right| {
                right
                    .degree
                    .cmp(&left.degree)
                    .then_with(|| right.updated_at.cmp(&left.updated_at))
            });
            nodes.truncate(max_nodes);
            truncated = true;
        }

        let kept: HashSet<String> = nodes.iter().map(|node| node.id.clone()).collect();
        let mut edges: Vec<GraphEdge> = edge_map
            .into_iter()
            .filter(|((source, target), _)| kept.contains(source) && kept.contains(target))
            .map(|((source, target), (link_type, weight))| GraphEdge {
                source,
                target,
                link_type,
                weight,
            })
            .collect();

        let total_edges = edges.len();
        let max_edges = query.max_edges.unwrap_or(DEFAULT_MAX_EDGES).max(1);
        if edges.len() > max_edges {
            edges.sort_by(|left, right| right.weight.cmp(&left.weight));
            edges.truncate(max_edges);
            truncated = true;
        }

        // 截断可能让部分节点失去连接：按最终边集重新计算「邻居数」
        // （同一对笔记之间多条链接只算一个邻居，节点大小才不会被重复链接放大）
        let mut neighbours: HashMap<&str, HashSet<&str>> = HashMap::new();
        for edge in &edges {
            neighbours.entry(edge.source.as_str()).or_default().insert(edge.target.as_str());
            neighbours.entry(edge.target.as_str()).or_default().insert(edge.source.as_str());
        }
        for node in nodes.iter_mut() {
            node.degree = neighbours
                .get(node.id.as_str())
                .map(|set| set.len() as u32)
                .unwrap_or(0);
            node.orphan = node.degree == 0;
        }

        // 5) 标签：用于悬浮提示与图例
        let ids: Vec<String> = nodes.iter().map(|node| node.id.clone()).collect();
        let tags = self
            .ctx
            .db
            .with_conn(|conn| tag_names_for_notes(conn, &ids))?;
        for node in nodes.iter_mut() {
            node.tags = tags.get(&node.id).cloned().unwrap_or_default();
        }

        Ok(GraphData {
            nodes,
            edges,
            truncated,
            total_nodes,
            total_edges,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }
}
