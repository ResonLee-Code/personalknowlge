//! 搜索领域模型（设计文档 10.5 / 18）。

use serde::{Deserialize, Serialize};

use crate::core::text::TextSegment;
use crate::domain::note::NoteSummary;

/// 搜索排序。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SearchSort {
    #[default]
    Relevance,
    UpdatedDesc,
    CreatedDesc,
    TitleAsc,
}

/// 搜索查询（设计文档 10.5）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct SearchQuery {
    pub keyword: String,
    pub category_id: Option<String>,
    pub tag_id: Option<String>,
    pub favorite_only: bool,
    /// ISO 8601 时间下限（含）
    pub updated_after: Option<String>,
    /// ISO 8601 时间上限（含）
    pub updated_before: Option<String>,
    pub sort: Option<SearchSort>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    /// 限定在某篇笔记内搜索（笔记内查找）
    pub scope_note_id: Option<String>,
}

/// 单条搜索结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub note: NoteSummary,
    /// BM25 分数（越小越相关）
    pub score: f64,
    pub title_matched: bool,
    pub snippet: Vec<TextSegment>,
    pub matched_terms: Vec<String>,
    /// 命中在**原文里的精确区间**（N5）。`None` = 没能定位（例如只命中标签/分类）。
    #[serde(default)]
    pub location: Option<SearchMatch>,
}

/// 命中在原文里的精确区间（N5）。
///
/// 区间是**正文（不含 frontmatter）里的字符下标**（Unicode 标量值），
/// 与编辑器里的正文是同一个坐标系；前端高亮时要按码点切片，不要直接用 UTF-16 下标。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    /// 命中所在分块的标题路径（空串 = 标题之前）
    pub heading: String,
    /// 命中所在分块的下标（`note_chunks.chunk_index`）——
    /// 检索 Provider 化（D3）用它做「同一条命中」的去重身份。
    #[serde(default)]
    pub chunk_index: i64,
    /// `[char_start, char_end)`
    pub char_start: i64,
    pub char_end: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub keyword: String,
    pub hits: Vec<SearchHit>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub took_ms: u64,
    /// 索引是否已经落后于数据（提示用户重建）
    pub index_stale: bool,
}

/// 索引状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexState {
    Ready,
    Stale,
    Rebuilding,
    Failed,
}

impl IndexState {
    pub fn as_str(self) -> &'static str {
        match self {
            IndexState::Ready => "ready",
            IndexState::Stale => "stale",
            IndexState::Rebuilding => "rebuilding",
            IndexState::Failed => "failed",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "stale" => IndexState::Stale,
            "rebuilding" => IndexState::Rebuilding,
            "failed" => IndexState::Failed,
            _ => IndexState::Ready,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    pub state: IndexState,
    pub indexed_notes: i64,
    pub total_notes: i64,
    pub last_rebuilt_at: Option<String>,
    pub last_error: Option<String>,
}
