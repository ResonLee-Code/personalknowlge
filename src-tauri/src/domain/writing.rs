//! Writing 2.0：大纲、续写、改写与篇幅调整的只读草稿契约。

use serde::{Deserialize, Serialize};

use super::ai::AskCitation;
use super::note::NoteDetail;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WritingMode {
    #[default]
    Outline,
    Continue,
    Rewrite,
    Shorten,
    Expand,
}

impl WritingMode {
    pub fn label(self) -> &'static str {
        match self {
            WritingMode::Outline => "生成大纲",
            WritingMode::Continue => "续写",
            WritingMode::Rewrite => "改写",
            WritingMode::Shorten => "缩短",
            WritingMode::Expand => "扩写",
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WritingRequest {
    pub note_id: String,
    pub mode: Option<WritingMode>,
    pub instruction: String,
    pub selection: Option<String>,
    pub selection_start: Option<i64>,
    pub selection_end: Option<i64>,
    pub confirm_outbound: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritingSegment {
    pub id: String,
    pub text: String,
    pub citations: Vec<AskCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritingDraft {
    pub note_id: String,
    pub mode: WritingMode,
    pub text: String,
    pub segments: Vec<WritingSegment>,
    pub citations: Vec<AskCitation>,
    pub provider: String,
    pub tier: String,
    pub evidence_sufficient: bool,
    pub warnings: Vec<String>,
    pub base_content_hash: String,
    pub target_start: i64,
    pub target_end: i64,
    pub input_tokens: usize,
    pub took_ms: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WritingApplyInput {
    pub note_id: String,
    pub base_content_hash: String,
    pub mode: WritingMode,
    pub target_start: i64,
    pub target_end: i64,
    pub accepted_segments: Vec<WritingSegment>,
    pub confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritingApplyPreview {
    pub note_id: String,
    pub content: String,
    pub added_chars: usize,
    pub removed_chars: usize,
    pub accepted_segments: usize,
    pub citations: Vec<AskCitation>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritingApplyResult {
    pub detail: NoteDetail,
    pub accepted_segments: usize,
    pub source_count: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WritingSummarySaveInput {
    pub title: String,
    pub segments: Vec<WritingSegment>,
    pub confirm: bool,
}
