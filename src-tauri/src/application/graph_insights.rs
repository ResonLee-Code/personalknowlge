//! Graph 2.0：在现有图谱抽样子图上计算可解释连接、近重复与结构洞察。
//!
//! 这里不把洞察结果写回数据库：它们可由正文、链接、标签和本地向量随时重算。
//! 10 万节点库先按 GraphService 的分类配额 + 连接优先规则抽样，再在抽样子图上计算，
//! 避免全库两两比较。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use crate::application::ai::embeddings::RemoteEmbeddingProvider;
use crate::application::ai::research::polarity;
use crate::application::ai::vector_index::{cosine_similarity, VectorStore};
use crate::application::context::WorkspaceContext;
use crate::application::graph_service::GraphService;
use crate::core::error::Result;
use crate::core::hash::sha256_hex_str;
use crate::core::text::{is_cjk, markdown_to_plain_text, truncate_chars};
use crate::domain::graph::{
    BridgeNote, DuplicateCluster, DuplicateMember, GraphData, GraphEvidence, GraphInsights,
    GraphInsightsQuery, GraphQuery, IslandCategory, SemanticNeighbor,
};
use crate::domain::note::{Note, NoteStatus};
use crate::repositories::{
    NoteRepository, RelationRepository, SqliteNoteRepository, SqliteRelationRepository,
};

const DEFAULT_SAMPLE_NODES: usize = 2_000;
const MAX_SAMPLE_NODES: usize = 5_000;
const MAX_CONTENT_CHARS: usize = 12_000;
const MAX_EXCERPT_CHARS: usize = 180;
const MAX_POSTING: usize = 80;
const MAX_STRUCTURAL_ITEMS: usize = 50;

const STOP_TERMS: &[&str] = &[
    "the", "and", "for", "with", "this", "that", "from", "into", "are", "was", "were", "have",
    "has", "not", "you", "our", "但", "的", "了", "和", "与", "及", "在", "是", "有", "为", "一个",
    "这个", "这些", "我们", "可以", "没有", "以及", "进行", "使用", "需要", "如何", "时候", "内容",
    "笔记", "知识",
];

struct NoteCorpus {
    note: Note,
    plain: String,
    term_weights: HashMap<String, f32>,
    norm: f32,
    signature: String,
    top_terms: Vec<String>,
}

pub struct GraphInsightsService {
    ctx: Arc<WorkspaceContext>,
}

impl GraphInsightsService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    pub fn insights(&self, query: GraphInsightsQuery) -> Result<GraphInsights> {
        let started = Instant::now();
        let max_nodes = query
            .max_nodes
            .unwrap_or(DEFAULT_SAMPLE_NODES)
            .clamp(1, MAX_SAMPLE_NODES);
        let graph = GraphService::new(Arc::clone(&self.ctx)).graph(GraphQuery {
            category_id: query.category_id.clone(),
            tag_id: query.tag_id.clone(),
            keyword: query.keyword.clone(),
            updated_after: query.updated_after.clone(),
            include_orphans: Some(true),
            max_nodes: Some(max_nodes),
            max_edges: Some(max_nodes.saturating_mul(8)),
            ..Default::default()
        })?;

        let node_ids: HashSet<String> = graph.nodes.iter().map(|node| node.id.clone()).collect();
        let notes = self.ctx.db.with_conn(|conn| {
            SqliteNoteRepository.list_by_status(conn, &self.ctx.workspace.id, NoteStatus::Active)
        })?;
        let mut warnings = Vec::new();
        let mut failed_reads = 0usize;
        let mut corpus = Vec::new();
        for note in notes.into_iter().filter(|note| node_ids.contains(&note.id)) {
            let Ok(stored) = self.ctx.storage.read_note(&note.file_path) else {
                failed_reads += 1;
                continue;
            };
            let plain = markdown_to_plain_text(&stored.body);
            corpus.push(make_corpus(note, plain));
        }
        if failed_reads > 0 {
            warnings.push(format!(
                "{failed_reads} 篇笔记无法读取，已从本次语义洞察中排除。"
            ));
        }
        if corpus.len() < graph.nodes.len() {
            warnings.push(format!(
                "图谱返回 {} 个节点，本次只读取到 {} 篇正文。",
                graph.nodes.len(),
                corpus.len()
            ));
        }
        if graph.truncated {
            warnings.push(format!(
                "图谱共 {} 个节点；洞察只在当前 {} 个抽样节点内计算，未覆盖全库。",
                graph.total_nodes,
                graph.nodes.len()
            ));
        }

        let by_id: HashMap<String, usize> = corpus
            .iter()
            .enumerate()
            .map(|(index, item)| (item.note.id.clone(), index))
            .collect();
        let mut vector_notes = self.load_note_vectors(&corpus, &mut warnings);
        let mode = if !vector_notes.is_empty() {
            "vector"
        } else {
            "local"
        }
        .to_string();

        let neighbors = self.neighbors(&query, &graph, &corpus, &by_id, &mut vector_notes);
        let duplicate_clusters = near_duplicates(
            &corpus,
            query.duplicate_threshold.unwrap_or(0.82).clamp(0.6, 0.99),
        );
        let orphan_notes = graph
            .nodes
            .iter()
            .filter(|node| node.orphan)
            .take(MAX_STRUCTURAL_ITEMS)
            .cloned()
            .collect();
        let bridge_notes = bridge_notes(&graph);
        let island_categories = self.island_categories(&graph)?;

        Ok(GraphInsights {
            mode,
            neighbors,
            duplicate_clusters,
            orphan_notes,
            bridge_notes,
            island_categories,
            sampled_nodes: graph.nodes.len(),
            total_nodes: graph.total_nodes,
            truncated: graph.truncated,
            warnings,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    fn load_note_vectors(
        &self,
        corpus: &[NoteCorpus],
        warnings: &mut Vec<String>,
    ) -> HashMap<String, Vec<f32>> {
        let policy = self.ctx.settings().ai;
        if !policy.enabled || !policy.vector_enabled || policy.vector_authorized_at.is_none() {
            warnings.push(
                "本轮使用标题、正文词项、引用、标签和结构关系计算；向量通道未启用。".to_string(),
            );
            return HashMap::new();
        }
        let Some(model) = policy.remote_embedding_model.as_deref() else {
            warnings.push("嵌入模型未配置；本轮使用本地可解释降级通道。".to_string());
            return HashMap::new();
        };
        let provider_id = RemoteEmbeddingProvider::provider_id(model);
        let ids: Vec<String> = corpus.iter().map(|item| item.note.id.clone()).collect();
        let rows = match self
            .ctx
            .db
            .with_conn(|conn| VectorStore::load_for_notes(conn, &provider_id, &ids))
        {
            Ok(rows) => rows,
            Err(error) => {
                warnings.push(format!(
                    "读取本地向量失败，已降级为本地词项：{}",
                    error.message
                ));
                return HashMap::new();
            }
        };
        let mut sums: HashMap<String, (Vec<f32>, usize)> = HashMap::new();
        for (note_id, _chunk_index, vector) in rows {
            let entry = sums
                .entry(note_id)
                .or_insert_with(|| (vec![0.0; vector.len()], 0));
            if entry.0.len() != vector.len() {
                continue;
            }
            for (sum, value) in entry.0.iter_mut().zip(vector) {
                *sum += value;
            }
            entry.1 += 1;
        }
        let mut vectors = HashMap::new();
        for (note_id, (mut vector, count)) in sums {
            if count == 0 || vector.is_empty() {
                continue;
            }
            let divisor = count as f32;
            let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
            if norm <= f32::EPSILON {
                continue;
            }
            for value in &mut vector {
                *value = *value / divisor / norm;
            }
            vectors.insert(note_id, vector);
        }
        if vectors.is_empty() {
            warnings
                .push("本地向量索引为空或与当前模型不匹配；已降级为本地可解释通道。".to_string());
        }
        vectors
    }

    fn neighbors(
        &self,
        query: &GraphInsightsQuery,
        graph: &GraphData,
        corpus: &[NoteCorpus],
        by_id: &HashMap<String, usize>,
        vectors: &mut HashMap<String, Vec<f32>>,
    ) -> Vec<SemanticNeighbor> {
        let Some(focus_id) = query
            .focus_note_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Vec::new();
        };
        let Some(focus_index) = by_id.get(focus_id).copied() else {
            return Vec::new();
        };
        let focus = &corpus[focus_index];
        let focus_vector = vectors.remove(focus_id);
        let mut adjacency: HashMap<String, HashSet<String>> = HashMap::new();
        for edge in &graph.edges {
            adjacency
                .entry(edge.source.clone())
                .or_default()
                .insert(edge.target.clone());
            adjacency
                .entry(edge.target.clone())
                .or_default()
                .insert(edge.source.clone());
        }
        let direct = adjacency.get(focus_id).cloned().unwrap_or_default();
        let outgoing: HashMap<String, HashSet<String>> = adjacency
            .iter()
            .map(|(source, targets)| (source.clone(), targets.clone()))
            .collect();
        let focus_references = outgoing.get(focus_id).cloned().unwrap_or_default();
        let tag_set: HashMap<&str, HashSet<&str>> = graph
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id.as_str(),
                    node.tags.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        let relations = self.relation_evidence(focus_id);
        let limit = query.neighbor_limit.unwrap_or(10).clamp(1, 20);
        let mut results = Vec::new();

        for candidate in corpus {
            if candidate.note.id == focus_id {
                continue;
            }
            let mut evidence = Vec::new();
            let mut structural = 0.0f64;
            if direct.contains(&candidate.note.id) {
                structural += 0.22;
                evidence.push(GraphEvidence {
                    kind: "reference".to_string(),
                    label: "两篇笔记之间存在双向链接".to_string(),
                    note_ids: vec![candidate.note.id.clone()],
                });
            }
            if let Some(relation) = relations.get(&candidate.note.id) {
                structural += 0.18;
                evidence.push(GraphEvidence {
                    kind: "relation".to_string(),
                    label: format!("已声明 {relation} 关系"),
                    note_ids: vec![candidate.note.id.clone()],
                });
            }
            let candidate_references = outgoing
                .get(&candidate.note.id)
                .cloned()
                .unwrap_or_default();
            let shared_references: Vec<String> = focus_references
                .intersection(&candidate_references)
                .filter(|id| *id != &candidate.note.id && *id != focus_id)
                .cloned()
                .collect();
            if !shared_references.is_empty() {
                structural += 0.08f64.min(0.03 * shared_references.len() as f64);
                evidence.push(GraphEvidence {
                    kind: "sharedReference".to_string(),
                    label: format!("共同引用 {} 篇笔记", shared_references.len()),
                    note_ids: shared_references.into_iter().take(3).collect(),
                });
            }
            let shared_tags: Vec<String> = tag_set
                .get(focus_id)
                .into_iter()
                .flatten()
                .filter(|tag| {
                    tag_set
                        .get(candidate.note.id.as_str())
                        .is_some_and(|tags| tags.contains(*tag))
                })
                .map(|tag| (*tag).to_string())
                .collect();
            if !shared_tags.is_empty() {
                structural += 0.05f64.min(0.02 * shared_tags.len() as f64);
                evidence.push(GraphEvidence {
                    kind: "sharedTag".to_string(),
                    label: format!("共同标签：{}", shared_tags.join("、")),
                    note_ids: vec![candidate.note.id.clone()],
                });
            }
            let shared_terms = shared_top_terms(focus, candidate);
            if !shared_terms.is_empty() {
                evidence.push(GraphEvidence {
                    kind: "sharedTopic".to_string(),
                    label: format!("共同主题：{}", shared_terms.join("、")),
                    note_ids: vec![candidate.note.id.clone()],
                });
            }

            let local_score = sparse_cosine(focus, candidate);
            let vector_score = focus_vector.as_ref().and_then(|left| {
                vectors
                    .get(&candidate.note.id)
                    .map(|right| cosine_similarity(left, right).clamp(0.0, 1.0) as f64)
            });
            let score = if let Some(vector_score) = vector_score {
                vector_score * 0.68 + local_score * 0.24 + structural
            } else {
                local_score * 0.78 + structural
            }
            .clamp(0.0, 1.0);
            if evidence.is_empty() || score < 0.08 {
                continue;
            }
            if vector_score.is_some_and(|value| value >= 0.45) {
                evidence.push(GraphEvidence {
                    kind: "similarContent".to_string(),
                    label: format!(
                        "本地向量相似度 {:.0}%",
                        vector_score.unwrap_or_default() * 100.0
                    ),
                    note_ids: vec![candidate.note.id.clone()],
                });
            } else if local_score >= 0.12 {
                evidence.push(GraphEvidence {
                    kind: "similarContent".to_string(),
                    label: format!("正文词项相似度 {:.0}%", local_score * 100.0),
                    note_ids: vec![candidate.note.id.clone()],
                });
            }
            results.push(SemanticNeighbor {
                note_id: candidate.note.id.clone(),
                title: candidate.note.title.clone(),
                category_id: candidate.note.category_id.clone(),
                score,
                evidence,
            });
        }
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.note_id.cmp(&right.note_id))
        });
        results.truncate(limit);
        results
    }

    fn relation_evidence(&self, focus_id: &str) -> HashMap<String, String> {
        let mut result = HashMap::new();
        let Ok(outgoing) = self
            .ctx
            .db
            .with_conn(|conn| SqliteRelationRepository.list_for_source(conn, focus_id))
        else {
            return result;
        };
        for row in outgoing {
            result.insert(row.target_note_id, row.relation_type);
        }
        let Ok(incoming) = self
            .ctx
            .db
            .with_conn(|conn| SqliteRelationRepository.list_for_target(conn, focus_id))
        else {
            return result;
        };
        for row in incoming {
            result.insert(row.source_note_id, row.relation_type);
        }
        result
    }

    fn island_categories(&self, graph: &GraphData) -> Result<Vec<IslandCategory>> {
        let lookup = self.ctx.category_lookup()?;
        let category_by_note: HashMap<&str, Option<&str>> = graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.category_id.as_deref()))
            .collect();
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut internal: HashMap<String, usize> = HashMap::new();
        let mut external: HashMap<String, usize> = HashMap::new();
        for node in &graph.nodes {
            if let Some(category) = &node.category_id {
                *counts.entry(category.clone()).or_default() += 1;
            }
        }
        for edge in &graph.edges {
            let left = category_by_note
                .get(edge.source.as_str())
                .copied()
                .flatten();
            let right = category_by_note
                .get(edge.target.as_str())
                .copied()
                .flatten();
            if left == right {
                if let Some(category) = left {
                    *internal.entry(category.to_string()).or_default() += 1;
                }
            } else {
                if let Some(category) = left {
                    *external.entry(category.to_string()).or_default() += 1;
                }
                if let Some(category) = right {
                    *external.entry(category.to_string()).or_default() += 1;
                }
            }
        }
        let mut islands: Vec<IslandCategory> = counts
            .into_iter()
            .filter_map(|(category_id, note_count)| {
                let external_edges = *external.get(&category_id).unwrap_or(&0);
                let internal_edges = *internal.get(&category_id).unwrap_or(&0);
                if note_count < 2 || external_edges > 0 || internal_edges == 0 {
                    return None;
                }
                Some(IslandCategory {
                    category_path: lookup.path_string(&category_id),
                    category_id,
                    note_count,
                    internal_edges,
                    external_edges,
                    explanation: format!(
                        "{note_count} 篇笔记在分类内形成 {internal_edges} 条连接，但未连接到其它分类。"
                    ),
                })
            })
            .collect();
        islands.sort_by(|left, right| {
            right
                .note_count
                .cmp(&left.note_count)
                .then_with(|| left.category_id.cmp(&right.category_id))
        });
        islands.truncate(MAX_STRUCTURAL_ITEMS);
        Ok(islands)
    }
}

fn make_corpus(note: Note, plain: String) -> NoteCorpus {
    let sampled = truncate_chars(plain.trim(), MAX_CONTENT_CHARS);
    let (mut term_weights, top_terms) = note_terms(&note.title, &sampled);
    let norm = term_weights
        .values()
        .map(|value| value * value)
        .sum::<f32>()
        .sqrt();
    if norm > f32::EPSILON {
        for value in term_weights.values_mut() {
            *value /= norm;
        }
    }
    let signature = sha256_hex_str(
        &sampled
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>(),
    );
    NoteCorpus {
        note,
        plain: sampled,
        term_weights,
        norm,
        signature,
        top_terms,
    }
}

fn note_terms(title: &str, body: &str) -> (HashMap<String, f32>, Vec<String>) {
    let mut terms = HashMap::new();
    add_terms(&mut terms, title, 2.5);
    add_terms(&mut terms, body, 1.0);
    let mut top: Vec<(String, f32)> = terms
        .iter()
        .map(|(term, weight)| (term.clone(), *weight))
        .collect();
    top.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.0.cmp(&right.0))
    });
    (
        terms,
        top.into_iter().take(8).map(|(term, _)| term).collect(),
    )
}

fn add_terms(target: &mut HashMap<String, f32>, text: &str, weight: f32) {
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0usize;
    while index < chars.len() {
        if is_cjk(chars[index]) {
            let start = index;
            while index < chars.len() && is_cjk(chars[index]) {
                index += 1;
            }
            let run = &chars[start..index];
            if run.len() == 1 {
                insert_term(target, run[0].to_string(), weight * 0.25);
            } else {
                for pair in run.windows(2) {
                    insert_term(target, pair.iter().collect(), weight);
                }
            }
            continue;
        }
        if chars[index].is_alphanumeric() {
            let start = index;
            while index < chars.len() && chars[index].is_alphanumeric() && !is_cjk(chars[index]) {
                index += 1;
            }
            let word: String = chars[start..index]
                .iter()
                .flat_map(|ch| ch.to_lowercase())
                .collect();
            if word.chars().count() >= 3 {
                insert_term(target, word, weight);
            }
            continue;
        }
        index += 1;
    }
}

fn insert_term(target: &mut HashMap<String, f32>, term: String, weight: f32) {
    if STOP_TERMS.contains(&term.as_str()) {
        return;
    }
    *target.entry(term).or_default() += weight;
}

fn sparse_cosine(left: &NoteCorpus, right: &NoteCorpus) -> f64 {
    if left.norm <= f32::EPSILON || right.norm <= f32::EPSILON {
        return 0.0;
    }
    let (small, large) = if left.term_weights.len() <= right.term_weights.len() {
        (&left.term_weights, &right.term_weights)
    } else {
        (&right.term_weights, &left.term_weights)
    };
    small
        .iter()
        .map(|(term, weight)| {
            large
                .get(term)
                .map(|other| (weight * other) as f64)
                .unwrap_or(0.0)
        })
        .sum()
}

fn shared_top_terms(left: &NoteCorpus, right: &NoteCorpus) -> Vec<String> {
    let right_terms: HashSet<&str> = right.top_terms.iter().map(String::as_str).collect();
    left.top_terms
        .iter()
        .filter(|term| right_terms.contains(term.as_str()))
        .take(4)
        .cloned()
        .collect()
}

fn near_duplicates(corpus: &[NoteCorpus], threshold: f64) -> Vec<DuplicateCluster> {
    let mut by_signature: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, item) in corpus.iter().enumerate() {
        by_signature
            .entry(item.signature.as_str())
            .or_default()
            .push(index);
    }

    let postings_by_term: HashMap<&str, Vec<usize>> = {
        let mut postings: HashMap<&str, Vec<usize>> = HashMap::new();
        for (index, item) in corpus.iter().enumerate() {
            for term in item.top_terms.iter().take(6) {
                postings.entry(term.as_str()).or_default().push(index);
            }
        }
        postings
    };
    let mut pair_votes: HashMap<(usize, usize), usize> = HashMap::new();
    let mut exact_pairs: HashSet<(usize, usize)> = HashSet::new();
    for members in postings_by_term.values() {
        if members.len() < 2 || members.len() > MAX_POSTING {
            continue;
        }
        for left in 0..members.len() {
            for right in (left + 1)..members.len() {
                let pair = (
                    members[left].min(members[right]),
                    members[left].max(members[right]),
                );
                *pair_votes.entry(pair).or_default() += 1;
            }
        }
    }
    for members in by_signature.values().filter(|members| members.len() > 1) {
        for left in 0..members.len() {
            for right in (left + 1)..members.len() {
                let pair = (
                    members[left].min(members[right]),
                    members[left].max(members[right]),
                );
                pair_votes.entry(pair).or_insert(3);
                exact_pairs.insert(pair);
            }
        }
    }

    let mut union = UnionFind::new(corpus.len());
    let mut pair_score: HashMap<(usize, usize), f64> = HashMap::new();
    let polarities: Vec<i32> = corpus.iter().map(|item| polarity(&item.plain)).collect();
    for (pair, votes) in pair_votes {
        if votes < 2 {
            continue;
        }
        let left_polarity = polarities[pair.0];
        let right_polarity = polarities[pair.1];
        if (left_polarity > 0 && right_polarity < 0) || (left_polarity < 0 && right_polarity > 0) {
            continue;
        }
        let score = if exact_pairs.contains(&pair) {
            1.0
        } else {
            sparse_cosine(&corpus[pair.0], &corpus[pair.1])
        };
        if score >= threshold {
            union.union(pair.0, pair.1);
            pair_score.insert(pair, score);
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for index in 0..corpus.len() {
        groups.entry(union.find(index)).or_default().push(index);
    }
    let mut clusters: Vec<DuplicateCluster> = groups
        .into_values()
        .filter(|members| members.len() >= 2)
        .map(|members| {
            let similarity = members
                .iter()
                .flat_map(|left| members.iter().map(move |right| (*left, *right)))
                .filter_map(|pair| {
                    let ordered = (pair.0.min(pair.1), pair.0.max(pair.1));
                    pair_score.get(&ordered).copied()
                })
                .fold(0.0f64, f64::max);
            let mut member_views: Vec<DuplicateMember> = members
                .iter()
                .map(|index| {
                    let item = &corpus[*index];
                    DuplicateMember {
                        note_id: item.note.id.clone(),
                        title: item.note.title.clone(),
                        excerpt: truncate_chars(item.plain.trim(), MAX_EXCERPT_CHARS),
                        updated_at: item.note.updated_at.clone(),
                    }
                })
                .collect();
            member_views.sort_by(|left, right| {
                right
                    .updated_at
                    .cmp(&left.updated_at)
                    .then_with(|| left.note_id.cmp(&right.note_id))
            });
            let first_id = member_views
                .first()
                .map(|member| member.note_id.as_str())
                .unwrap_or("");
            DuplicateCluster {
                id: format!("duplicate-{first_id}"),
                similarity,
                reason: format!(
                    "正文词项相似度至少 {:.0}%，可能是同一主题的不同版本；合并前请核对差异。",
                    similarity * 100.0
                ),
                members: member_views,
                requires_confirmation: true,
            }
        })
        .collect();
    clusters.sort_by(|left, right| {
        right
            .members
            .len()
            .cmp(&left.members.len())
            .then_with(|| {
                right
                    .similarity
                    .partial_cmp(&left.similarity)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.id.cmp(&right.id))
    });
    clusters.truncate(20);
    clusters
}

fn bridge_notes(graph: &GraphData) -> Vec<BridgeNote> {
    let categories: HashMap<&str, Option<&str>> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.category_id.as_deref()))
        .collect();
    let mut neighbours: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &graph.edges {
        neighbours
            .entry(edge.source.as_str())
            .or_default()
            .push(edge.target.as_str());
        neighbours
            .entry(edge.target.as_str())
            .or_default()
            .push(edge.source.as_str());
    }
    let mut result = Vec::new();
    for node in &graph.nodes {
        let Some(connected) = neighbours.get(node.id.as_str()) else {
            continue;
        };
        let own = node.category_id.as_deref();
        let other_categories: HashSet<Option<&str>> = connected
            .iter()
            .filter_map(|neighbor| categories.get(neighbor).copied())
            .filter(|category| *category != own)
            .collect();
        if other_categories.is_empty() {
            continue;
        }
        let cross_edges = connected
            .iter()
            .filter(|neighbor| categories.get(*neighbor).copied().flatten() != own)
            .count();
        let ratio = cross_edges as f64 / connected.len().max(1) as f64;
        if ratio < 0.5 && other_categories.len() < 2 {
            continue;
        }
        result.push(BridgeNote {
            note_id: node.id.clone(),
            title: node.title.clone(),
            category_id: node.category_id.clone(),
            connected_categories: other_categories.len(),
            bridge_score: (ratio * (1.0 + other_categories.len() as f64).ln()).clamp(0.0, 1.0),
            explanation: format!(
                "连接 {} 个其它分类，跨分类邻居占 {:.0}%。",
                other_categories.len(),
                ratio * 100.0
            ),
        });
    }
    result.sort_by(|left, right| {
        right
            .bridge_score
            .partial_cmp(&left.bridge_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.note_id.cmp(&right.note_id))
    });
    result.truncate(MAX_STRUCTURAL_ITEMS);
    result
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
        }
    }

    fn find(&mut self, value: usize) -> usize {
        if self.parent[value] != value {
            self.parent[value] = self.find(self.parent[value]);
        }
        self.parent[value]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        if left != right {
            self.parent[right] = left;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::graph::{GraphEdge, GraphNode};

    fn note(id: &str, title: &str, category: Option<&str>, body: &str) -> NoteCorpus {
        let note = Note {
            id: id.to_string(),
            workspace_id: "ws".to_string(),
            title: title.to_string(),
            file_path: format!("{id}.md"),
            category_id: category.map(str::to_string),
            status: NoteStatus::Active,
            content_hash: None,
            word_count: body.chars().count() as i64,
            favorite: false,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-02T00:00:00Z".to_string(),
            last_indexed_at: None,
            source_type: None,
            source_ref: None,
            source_reliability: None,
            valid_until: None,
            file_size: None,
            file_mtime: None,
            parent_id: None,
            declared_parent: None,
            sort_order: 0,
            pinned: false,
        };
        make_corpus(note, body.to_string())
    }

    fn graph_node(id: &str, category: Option<&str>) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            title: id.to_string(),
            category_id: category.map(str::to_string),
            updated_at: "2026-01-02T00:00:00Z".to_string(),
            degree: 1,
            tags: Vec::new(),
            orphan: false,
        }
    }

    #[test]
    fn topic_similarity_is_explainable_and_nonzero() {
        let left = note(
            "a",
            "SQLite 检索",
            None,
            "SQLite 全文索引使用倒排索引和分块检索",
        );
        let right = note("b", "检索优化", None, "全文索引需要分块检索并优化倒排索引");
        assert!(sparse_cosine(&left, &right) > 0.2);
        assert!(!shared_top_terms(&left, &right).is_empty());
    }

    #[test]
    fn duplicate_detection_requires_enough_shared_terms() {
        let left = note(
            "a",
            "版本甲",
            None,
            "知识管理需要本地优先并支持全文检索和引用",
        );
        let right = note(
            "b",
            "版本乙",
            None,
            "知识管理需要本地优先并支持全文检索和引用",
        );
        let distinct = note("c", "购物清单", None, "牛奶鸡蛋和面包");
        let clusters = near_duplicates(&[left, right, distinct], 0.82);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].members.len(), 2);
        assert!(clusters[0].requires_confirmation);
    }

    #[test]
    fn bridge_notes_explain_cross_category_connections() {
        let graph = GraphData {
            nodes: vec![
                graph_node("a", Some("cat_a")),
                graph_node("b", Some("cat_b")),
                graph_node("bridge", Some("cat_a")),
            ],
            edges: vec![
                GraphEdge {
                    source: "bridge".to_string(),
                    target: "b".to_string(),
                    link_type: "wikilink".to_string(),
                    weight: 1,
                },
                GraphEdge {
                    source: "bridge".to_string(),
                    target: "a".to_string(),
                    link_type: "wikilink".to_string(),
                    weight: 1,
                },
            ],
            truncated: false,
            total_nodes: 3,
            total_edges: 2,
            duration_ms: 0,
        };
        let bridges = bridge_notes(&graph);
        assert!(bridges.iter().any(|bridge| bridge.note_id == "bridge"));
    }
}
