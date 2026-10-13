//! 检索 Provider 化 + RRF 融合（D3 / N7 的第一步）。
//!
//! 为什么要有 Provider：检索本来就不止一个通道（关键词 BM25 / 向量 / 图谱），
//! 每个通道各自召回，再**融合**成一个结果集。关键纪律是：
//! **任何一档不可用时，检索都必须仍然可用**——所以默认通道是 BM25（永远在），
//! 其余通道"装了/授权了才参与"，并且**实际用了哪些通道要如实标注**。
//!
//! 这一步**不引入任何外发**：只把现有 FTS 包成 `Bm25Retriever`，把融合与通道标注做真。

use std::collections::HashMap;
use std::sync::Arc;

use crate::application::ai::traits::{RetrievalOrigin, RetrievedChunk, Retriever};
use crate::application::ai::embeddings::EmbeddingProvider;
use crate::application::ai::vector_index::VectorIndexService;
use crate::application::context::WorkspaceContext;
use crate::application::search_service::SearchService;
use crate::core::error::Result;
use crate::domain::search::SearchQuery;

/// RRF 的平滑常数（原论文取 60）：越小越偏向头部名次。
pub const RRF_K: f32 = 60.0;

/// 一个通道的可用性说明——界面/报告要能说清"这次是谁在召回"。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelStatus {
    pub id: String,
    pub available: bool,
    /// 人读说明（可用/不可用的原因）
    pub note: String,
}

/// 融合后的一条命中：原始片段 + 贡献它的通道 + RRF 分数。
#[derive(Debug, Clone)]
pub struct FusedChunk {
    pub chunk: RetrievedChunk,
    /// 贡献了这条命中的通道（按加入顺序）
    pub providers: Vec<String>,
    pub rrf_score: f32,
}

/// 命中身份：同一条分块在不同通道里会出现，融合时必须并成一条。
fn key_of(chunk: &RetrievedChunk) -> (String, i64) {
    (chunk.note_id.clone(), chunk.chunk_index)
}

/// Reciprocal Rank Fusion：`score = Σ 1/(k + rank)`（rank 从 1 起）。
///
/// 只用**名次**不用原始分数是刻意的：不同通道的量纲没法比（BM25 越小越好、
/// 余弦相似度越大越好），按名次融合才稳。
pub fn reciprocal_rank_fusion(
    lists: Vec<(String, Vec<RetrievedChunk>)>,
    limit: usize,
) -> Vec<FusedChunk> {
    let mut merged: HashMap<(String, i64), FusedChunk> = HashMap::new();
    let mut order: Vec<(String, i64)> = Vec::new();

    for (provider, chunks) in lists {
        for (index, chunk) in chunks.into_iter().enumerate() {
            let rank = (index + 1) as f32;
            let contribution = 1.0 / (RRF_K + rank);
            let key = key_of(&chunk);
            match merged.get_mut(&key) {
                Some(existing) => {
                    existing.rrf_score += contribution;
                    if !existing.providers.contains(&provider) {
                        existing.providers.push(provider.clone());
                    }
                }
                None => {
                    order.push(key.clone());
                    merged.insert(
                        key,
                        FusedChunk {
                            chunk,
                            providers: vec![provider.clone()],
                            rrf_score: contribution,
                        },
                    );
                }
            }
        }
    }

    // 按 RRF 分数降序；同分时按首次出现顺序（稳定、可复现）
    let mut fused: Vec<FusedChunk> = order
        .into_iter()
        .filter_map(|key| merged.remove(&key))
        .collect();
    fused.sort_by(|left, right| {
        right
            .rrf_score
            .partial_cmp(&left.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    fused.truncate(limit.max(1));
    fused
}

/// 一次检索的报告：**用了哪些通道**（含不可用的与原因）+ 融合后的命中。
#[derive(Debug, Clone)]
pub struct RetrievalOutcome {
    pub channels: Vec<ChannelStatus>,
    pub hits: Vec<FusedChunk>,
}

/// 按顺序跑每个通道，再融合。
///
/// 两条纪律：
/// - **单个通道出问题不影响整次检索**：不可用/报错只如实标注，其它通道照常；
/// - 因此只要 BM25 在，检索就永远有结果（这正是"检索永远可用"的落地方式）。
pub fn retrieve(providers: &[&dyn Retriever], query: &str, limit: usize) -> RetrievalOutcome {
    let mut channels = Vec::new();
    let mut lists = Vec::new();
    for provider in providers {
        let id = provider.id().to_string();
        if !provider.is_available() {
            channels.push(ChannelStatus {
                id,
                available: false,
                note: "这一档当前不可用，已跳过（不影响其它通道）".to_string(),
            });
            continue;
        }
        match provider.search(query, limit) {
            Ok(chunks) => {
                let count = chunks.len();
                channels.push(ChannelStatus {
                    id: id.clone(),
                    available: true,
                    note: format!("召回 {count} 条"),
                });
                if !chunks.is_empty() {
                    lists.push((id, chunks));
                }
            }
            Err(error) => {
                channels.push(ChannelStatus {
                    id,
                    available: false,
                    note: format!("检索失败，已跳过（不影响其它通道）：{}", error.message),
                });
            }
        }
    }
    RetrievalOutcome { channels, hits: reciprocal_rank_fusion(lists, limit) }
}

/// 转成对外的 DTO（命令层用）。
pub fn to_dto(outcome: RetrievalOutcome) -> crate::domain::ai::RetrievalReport {
    crate::domain::ai::RetrievalReport {
        channels: outcome
            .channels
            .into_iter()
            .map(|channel| crate::domain::ai::RetrievalChannelStatus {
                id: channel.id,
                available: channel.available,
                note: channel.note,
            })
            .collect(),
        hits: outcome
            .hits
            .into_iter()
            .map(|fused| crate::domain::ai::RetrievalHit {
                note_id: fused.chunk.note_id,
                chunk_index: fused.chunk.chunk_index,
                heading: fused.chunk.heading_path.join(" > "),
                snippet: fused.chunk.snippet,
                char_start: fused.chunk.char_start,
                char_end: fused.chunk.char_end,
                score: fused.rrf_score,
                providers: fused.providers,
            })
            .collect(),
    }
}

/// BM25 通道：把现有 FTS 检索包成 `Retriever`，并带上 D1 的**精确区间**。
pub struct Bm25Retriever {
    ctx: Arc<WorkspaceContext>,
}

impl Bm25Retriever {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }
}

impl Retriever for Bm25Retriever {
    fn id(&self) -> &'static str {
        "bm25"
    }

    /// 关键词检索永远可用（不依赖模型、不联网）——这是"检索永远可用"的底座。
    fn is_available(&self) -> bool {
        true
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<RetrievedChunk>> {
        let keyword = query.trim();
        if keyword.is_empty() {
            return Ok(Vec::new());
        }
        let response = SearchService::new(Arc::clone(&self.ctx)).search(SearchQuery {
            keyword: keyword.to_string(),
            page: Some(1),
            page_size: Some(limit.clamp(1, 50) as i64),
            ..Default::default()
        })?;

        Ok(response
            .hits
            .into_iter()
            .map(|hit| {
                let snippet: String = hit.snippet.iter().map(|segment| segment.text.as_str()).collect();
                let (chunk_index, char_start, char_end, heading_path) = match &hit.location {
                    Some(location) => (
                        location.chunk_index,
                        location.char_start,
                        location.char_end,
                        location
                            .heading
                            .split(" > ")
                            .map(str::to_string)
                            .collect::<Vec<_>>(),
                    ),
                    // 没定位到（例如只命中标签）：仍作为结果返回，但区间是"粗略"的
                    None => (
                        0,
                        0,
                        snippet.chars().count() as i64,
                        vec![hit.note.title.clone()],
                    ),
                };
                RetrievedChunk {
                    note_id: hit.note.id.clone(),
                    chunk_index,
                    heading_path: heading_path
                        .into_iter()
                        .filter(|part| !part.is_empty())
                        .collect(),
                    snippet,
                    char_start,
                    char_end,
                    // FTS 的 BM25 分数**越小越相关**；这里统一成"越大越好"，便于融合与展示
                    score: (-hit.score) as f32,
                    origin: RetrievalOrigin::Bm25,
                }
            })
            .collect())
    }
}

/// 向量通道（D3/N7 第二步）：把本地存的向量拿来做相似度召回。
///
/// **不可用时不参与**：`is_available()` 为 false（没开、没授权、没建索引），
/// `retrieve()` 会如实标注并跳过——BM25 照常，检索永远可用。
pub struct VectorRetriever {
    service: VectorIndexService,
    provider: Arc<dyn EmbeddingProvider>,
}

impl VectorRetriever {
    pub fn new(ctx: Arc<WorkspaceContext>, provider: Arc<dyn EmbeddingProvider>) -> Self {
        Self { service: VectorIndexService::new(ctx), provider }
    }
}

impl Retriever for VectorRetriever {
    fn id(&self) -> &'static str {
        "vector"
    }

    fn is_available(&self) -> bool {
        let provider_id = self.provider.id();
        self.service
            .status(Some(&provider_id))
            .map(|status| status.available)
            .unwrap_or(false)
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<RetrievedChunk>> {
        self.service.search(self.provider.as_ref(), query, limit)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(note: &str, index: i64, score: f32) -> RetrievedChunk {
        RetrievedChunk {
            note_id: note.to_string(),
            chunk_index: index,
            heading_path: vec!["标题".to_string()],
            snippet: format!("{note}#{index}"),
            char_start: 0,
            char_end: 1,
            score,
            origin: RetrievalOrigin::Bm25,
        }
    }

    #[test]
    fn a_single_channel_passes_through_in_its_own_order() {
        let fused = reciprocal_rank_fusion(
            vec![("bm25".to_string(), vec![chunk("a", 0, 9.0), chunk("b", 0, 1.0)])],
            10,
        );
        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].chunk.note_id, "a");
        assert_eq!(fused[0].providers, vec!["bm25".to_string()]);
        // 名次越靠前分数越高
        assert!(fused[0].rrf_score > fused[1].rrf_score);
    }

    #[test]
    fn the_same_chunk_from_two_channels_is_merged_and_annotated() {
        let fused = reciprocal_rank_fusion(
            vec![
                ("bm25".to_string(), vec![chunk("shared", 3, 5.0)]),
                ("vector".to_string(), vec![chunk("shared", 3, 0.9)]),
            ],
            10,
        );
        assert_eq!(fused.len(), 1, "同一条分块必须并成一条");
        let mut providers = fused[0].providers.clone();
        providers.sort();
        assert_eq!(providers, vec!["bm25".to_string(), "vector".to_string()]);
        // 两个通道都命中同一名次 → 分数是两倍
        assert!((fused[0].rrf_score - 2.0 / (RRF_K + 1.0)).abs() < 1e-6);
    }

    #[test]
    fn a_chunk_found_by_both_channels_outranks_one_found_by_a_single_channel() {
        let fused = reciprocal_rank_fusion(
            vec![
                ("bm25".to_string(), vec![chunk("only_bm25", 0, 9.0), chunk("both", 1, 8.0)]),
                ("vector".to_string(), vec![chunk("both", 1, 0.9)]),
            ],
            10,
        );
        assert_eq!(fused[0].chunk.note_id, "both", "两个通道都认可的排前面：{fused:?}");
    }

    #[test]
    fn empty_and_limited_inputs_are_handled() {
        assert!(reciprocal_rank_fusion(vec![], 10).is_empty());
        assert!(reciprocal_rank_fusion(vec![("bm25".to_string(), vec![])], 10).is_empty());
        let fused = reciprocal_rank_fusion(
            vec![(
                "bm25".to_string(),
                vec![chunk("a", 0, 1.0), chunk("b", 0, 1.0), chunk("c", 0, 1.0)],
            )],
            2,
        );
        assert_eq!(fused.len(), 2, "limit 要生效");
    }
}
