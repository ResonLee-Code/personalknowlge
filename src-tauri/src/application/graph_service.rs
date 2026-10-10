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
        // 图谱需要**完整**节点集：无过滤时一次查询取全（避免大库下逐页往返被拖慢/截断）；
        // 有过滤（分类 / 标签 / 关键词）时才按 500/页分页，直到取完（上限 50 万兜底）。
        let unfiltered = list_query.category_id.is_none()
            && list_query.tag_id.is_none()
            && list_query.keyword.is_none();
        let mut notes = Vec::new();
        if unfiltered {
            notes = self.ctx.db.with_conn(|conn| {
                SqliteNoteRepository.list_by_status(conn, &workspace_id, NoteStatus::Active)
            })?;
        } else {
            let mut page = 1i64;
            loop {
                let mut paged = list_query.clone();
                paged.page = Some(page);
                paged.page_size = Some(500);
                let (batch, total) = self.ctx.db.with_conn(|conn| {
                    SqliteNoteRepository.list(conn, &workspace_id, &paged)
                })?;
                let batch_len = batch.len();
                notes.extend(batch);
                if batch_len == 0 || notes.len() as i64 >= total || notes.len() >= 500_000 {
                    break;
                }
                page += 1;
            }
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
            .with_conn(|conn| SqliteLinkRepository.list_resolved_pairs(conn, &workspace_id))?;

        // 大库下这里会走几十万次：键用 `HashMap` + 复用同一份字符串索引，避免 BTreeMap 的比较开销
        let mut edge_map: HashMap<(String, String), (String, u32)> = HashMap::new();
        let mut adjacency: HashMap<String, BTreeSet<String>> = HashMap::new();
        for (source, target, link_type) in links {
            if !allowed.contains(&source) || !allowed.contains(&target) {
                continue;
            }
            if source == target {
                // 自链接不计入图谱（避免自环影响布局与度数）
                continue;
            }
            let entry = edge_map
                .entry((source.clone(), target.clone()))
                .or_insert_with(|| (link_type.clone(), 0));
            entry.1 += 1;
            adjacency.entry(source.clone()).or_default().insert(target.clone());
            adjacency.entry(target).or_default().insert(source);
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
            nodes = sample_nodes(nodes, &adjacency, max_nodes);
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

/// 大库抽样：**连接优先 + 分类配额**。
///
/// 为什么不是「全局按度数取前 N」：那样会把图切碎——30k 篇基准里取 2000 个节点只剩 407 条边，
/// 用户看到的是散点而不是结构。这里改成：
/// 1. 按分类分配名额（与分类规模成比例，至少 1 个），保证各分类都有代表；
/// 2. 组内先取度数最高的节点作为种子，再**沿邻居向外扩张**填满名额（连通性优先）；
/// 3. 名额没填满时，用剩余节点按度数补足。
fn sample_nodes(
    nodes: Vec<GraphNode>,
    adjacency: &HashMap<String, BTreeSet<String>>,
    budget: usize,
) -> Vec<GraphNode> {
    let mut by_id: HashMap<String, GraphNode> =
        nodes.into_iter().map(|node| (node.id.clone(), node)).collect();

    // 分类分组（None 视为一个独立组），并统计规模
    let mut groups: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
    for node in by_id.values() {
        groups
            .entry(node.category_id.clone())
            .or_default()
            .push(node.id.clone());
    }
    for ids in groups.values_mut() {
        ids.sort_by(|left, right| {
            let degree = |id: &String| by_id.get(id).map(|node| node.degree).unwrap_or(0);
            degree(right).cmp(&degree(left)).then_with(|| right.cmp(left))
        });
    }
    let total: usize = groups.values().map(Vec::len).sum();
    let group_count = groups.len().max(1);

    let mut selected: Vec<String> = Vec::with_capacity(budget);
    let mut taken: HashSet<String> = HashSet::new();
    for (index, (_, ids)) in groups.iter().enumerate() {
        if selected.len() >= budget {
            break;
        }
        // 按规模配额；最后一个组拿走剩余名额，避免四舍五入丢名额
        let share = if index + 1 == group_count {
            budget.saturating_sub(selected.len())
        } else {
            ((ids.len() * budget) / total.max(1)).max(1)
        };
        let mut quota = share.min(budget - selected.len());
        for seed in ids {
            if quota == 0 {
                break;
            }
            if taken.contains(seed) {
                continue;
            }
            taken.insert(seed.clone());
            selected.push(seed.clone());
            quota -= 1;
            // 连接优先：先把种子的邻居拉进来
            if let Some(neighbours) = adjacency.get(seed) {
                for neighbour in neighbours {
                    if quota == 0 {
                        break;
                    }
                    if by_id.contains_key(neighbour) && taken.insert(neighbour.clone()) {
                        selected.push(neighbour.clone());
                        quota -= 1;
                    }
                }
            }
        }
    }

    // 名额没满（例如孤点很多）：按度数补足
    if selected.len() < budget {
        let mut rest: Vec<(u32, String)> = by_id
            .values()
            .filter(|node| !taken.contains(&node.id))
            .map(|node| (node.degree, node.id.clone()))
            .collect();
        rest.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
        for (_, id) in rest {
            if selected.len() >= budget {
                break;
            }
            taken.insert(id.clone());
            selected.push(id);
        }
    }

    selected
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect()
}
