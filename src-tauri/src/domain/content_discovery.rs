//! 内容发现领域模型：外部内容收件箱、候选链接与抓取页面。

use serde::{Deserialize, Serialize};

/// 外部内容来源类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentSourceKind {
    Web,
    Wechat,
    Git,
}

impl ContentSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Wechat => "wechat",
            Self::Git => "git",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "wechat" => Self::Wechat,
            "git" => Self::Git,
            _ => Self::Web,
        }
    }
}

/// 内容收件箱状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentCaptureStatus {
    Received,
    Discovering,
    AwaitingSelection,
    Fetching,
    Ready,
    Saved,
    Failed,
    Ignored,
    Duplicate,
}

impl ContentCaptureStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Received => "received",
            Self::Discovering => "discovering",
            Self::AwaitingSelection => "awaitingSelection",
            Self::Fetching => "fetching",
            Self::Ready => "ready",
            Self::Saved => "saved",
            Self::Failed => "failed",
            Self::Ignored => "ignored",
            Self::Duplicate => "duplicate",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "received" => Self::Received,
            "discovering" => Self::Discovering,
            "awaitingSelection" => Self::AwaitingSelection,
            "fetching" => Self::Fetching,
            "ready" => Self::Ready,
            "saved" => Self::Saved,
            "ignored" => Self::Ignored,
            "duplicate" => Self::Duplicate,
            _ => Self::Failed,
        }
    }
}

/// 从 URL / 分享文本创建内容记录。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContentCaptureInput {
    pub url: String,
    pub shared_text: Option<String>,
    pub user_note: Option<String>,
    pub confirmed: bool,
}

/// 递归抓取选项。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContentDiscoveryOptions {
    pub max_depth: u32,
    pub max_pages: u32,
    pub same_path: bool,
    pub include_subdomains: bool,
    pub include_external: bool,
    pub include_hidden: bool,
    pub keep_full_text: bool,
}

impl Default for ContentDiscoveryOptions {
    fn default() -> Self {
        Self {
            max_depth: 1,
            max_pages: 20,
            same_path: true,
            include_subdomains: false,
            include_external: false,
            include_hidden: false,
            keep_full_text: false,
        }
    }
}

impl ContentDiscoveryOptions {
    pub fn clamped(mut self) -> Self {
        self.max_depth = self.max_depth.clamp(0, 3);
        self.max_pages = self.max_pages.clamp(1, 100);
        self
    }
}

/// 深度抓取请求。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContentDiscoveryRunInput {
    pub capture_id: String,
    pub selected_urls: Vec<String>,
    pub options: ContentDiscoveryOptions,
    pub confirmed: bool,
}

/// 页面中发现的一条候选链接。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentDiscoveryCandidate {
    pub url: String,
    pub canonical_url: String,
    pub parent_url: String,
    pub depth: u32,
    pub title: String,
    pub anchor_text: Option<String>,
    pub context: String,
    pub hidden: bool,
    pub same_origin: bool,
    pub same_site: bool,
    pub eligible: bool,
    pub blocked_reason: Option<String>,
    pub relevance_score: Option<f32>,
    pub relevance_reason: Option<String>,
    pub selected: bool,
}

/// 已抓取页面（正文存放在工作区缓存文件，不进入备份数据库）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentDiscoveryPage {
    pub url: String,
    pub canonical_url: String,
    pub parent_url: Option<String>,
    pub depth: u32,
    pub title: String,
    pub site: Option<String>,
    pub content_hash: String,
    pub word_count: i64,
    pub bytes: usize,
    pub cache_path: String,
    pub status: String,
    pub fetched_at: String,
    pub error: Option<String>,
}

/// 内容收件箱主记录。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCapture {
    pub id: String,
    pub workspace_id: String,
    pub requested_url: String,
    pub final_url: String,
    pub canonical_url: String,
    pub source_kind: ContentSourceKind,
    pub title: String,
    pub author: Option<String>,
    pub site: Option<String>,
    pub published_at: Option<String>,
    pub captured_at: String,
    pub user_note: Option<String>,
    pub status: ContentCaptureStatus,
    pub summary: Option<String>,
    pub key_points: Vec<String>,
    pub tags: Vec<String>,
    pub related_note_ids: Vec<String>,
    /// 详情接口填充：相关笔记的标题，避免前端只拿到不可读的 id。
    #[serde(default)]
    pub related_notes: Vec<ContentRelatedNote>,
    pub suggested_category_id: Option<String>,
    pub decision_provider: Option<String>,
    pub keep_full_text: bool,
    pub page_count: usize,
    pub word_count: i64,
    pub cache_dir: String,
    pub error: Option<String>,
    pub note_id: Option<String>,
    pub duplicate_of: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub saved_at: Option<String>,
    /// 最近一次刷新是否检测到来源正文变化；不会自动覆盖已入库笔记。
    pub source_changed: bool,
    pub last_refreshed_at: Option<String>,
    #[serde(default)]
    pub candidates: Vec<ContentDiscoveryCandidate>,
    #[serde(default)]
    pub pages: Vec<ContentDiscoveryPage>,
    /// 详情接口填充：根页面与已选页面的 Markdown 预览。
    #[serde(default)]
    pub markdown: Option<String>,
}

/// 内容发现给出的相关笔记建议。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentRelatedNote {
    pub note_id: String,
    pub title: String,
}

/// 内容写入普通笔记的输入。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ContentCaptureSaveInput {
    pub capture_id: String,
    pub title: Option<String>,
    pub category_id: Option<String>,
    /// 入库前可编辑的标签；缺省时使用内容发现生成的建议标签。
    pub tags: Option<Vec<String>>,
    /// 入库时是否在正式笔记中保留完整抓取正文。
    pub keep_full_text: Option<bool>,
}

/// 内容写入结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentCaptureSaveResult {
    pub capture_id: String,
    pub note_id: String,
    pub note_title: String,
}
