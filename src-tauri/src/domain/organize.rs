//! Organize 领域模型（V2.0 P0）。
//!
//! 建议和任务都是持久化对象；AI 只能生成建议，不能自行写 Markdown。
//! 接受建议时才走常规笔记写入路径，并保存接受前原文用于字节级撤销。

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuggestionKind {
    Summary,
    Tags,
    Relations,
    Duplicate,
}

impl SuggestionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Summary => "summary",
            Self::Tags => "tags",
            Self::Relations => "relations",
            Self::Duplicate => "duplicate",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "summary" => Some(Self::Summary),
            "tags" => Some(Self::Tags),
            "relations" => Some(Self::Relations),
            "duplicate" => Some(Self::Duplicate),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuggestionStatus {
    Pending,
    Accepted,
    Rejected,
    Superseded,
}

impl SuggestionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Superseded => "superseded",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            "superseded" => Some(Self::Superseded),
            _ => None,
        }
    }
}

/// 一条建议必须能追溯到的原文证据。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionEvidence {
    pub note_id: String,
    pub title: String,
    pub heading: String,
    pub snippet: String,
    pub char_start: i64,
    pub char_end: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedTag {
    pub tag: String,
    pub reason: String,
    /// 是否复用知识库中已有标签。
    pub existing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedRelation {
    pub target_note_id: String,
    pub target_title: String,
    pub relation_type: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCandidate {
    pub note_id: String,
    pub title: String,
    pub score: f32,
    pub reasons: Vec<String>,
}

/// 类型化建议内容。数据库存 JSON，读取后按 `kind` 对应解析。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AiSuggestionPayload {
    Summary {
        summary: String,
        bullets: Vec<String>,
    },
    Tags {
        tags: Vec<SuggestedTag>,
    },
    Relations {
        relations: Vec<SuggestedRelation>,
    },
    Duplicate {
        candidates: Vec<DuplicateCandidate>,
    },
}

/// 对外展示的一条建议。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSuggestion {
    pub id: String,
    pub note_id: String,
    pub note_title: String,
    pub kind: SuggestionKind,
    pub status: SuggestionStatus,
    pub model: String,
    pub prompt_version: String,
    pub evidence: Vec<SuggestionEvidence>,
    pub payload: AiSuggestionPayload,
    pub source_hash: String,
    pub created_at: String,
    pub updated_at: String,
    pub applied_at: Option<String>,
    pub undone_at: Option<String>,
    pub error: Option<String>,
    pub undo_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeBatchFailure {
    pub suggestion_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizeBatchReport {
    pub requested: usize,
    pub succeeded: usize,
    pub failures: Vec<OrganizeBatchFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiJobStatus {
    Queued,
    Running,
    Paused,
    Canceled,
    Completed,
    Failed,
}

impl AiJobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Canceled => "canceled",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "canceled" => Some(Self::Canceled),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AiJobItemStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl AiJobItemStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "skipped" => Some(Self::Skipped),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiJob {
    pub id: String,
    pub workspace_id: String,
    pub kind: String,
    pub status: AiJobStatus,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub payload: Value,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiJobItem {
    pub job_id: String,
    pub item_index: usize,
    pub note_id: String,
    pub status: AiJobItemStatus,
    pub attempts: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiJobProgress {
    pub job_id: String,
    pub status: AiJobStatus,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub current_note_id: Option<String>,
}
