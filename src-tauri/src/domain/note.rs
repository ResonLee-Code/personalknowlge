//! 笔记领域模型（设计文档 10.2）。

use serde::{Deserialize, Serialize};

use crate::core::text::TextSegment;
use crate::domain::category::Category;
use crate::domain::tag::Tag;

/// 笔记状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteStatus {
    /// 正常使用
    Active,
    /// 已移入回收站
    Deleted,
    /// 文件被外部删除或移动
    Missing,
}

impl NoteStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            NoteStatus::Active => "active",
            NoteStatus::Deleted => "deleted",
            NoteStatus::Missing => "missing",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "deleted" => NoteStatus::Deleted,
            "missing" => NoteStatus::Missing,
            _ => NoteStatus::Active,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    /// 工作区相对路径，使用 `/` 分隔
    pub file_path: String,
    pub category_id: Option<String>,
    pub status: NoteStatus,
    pub content_hash: Option<String>,
    pub word_count: i64,
    pub favorite: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_indexed_at: Option<String>,
    /// 来源类型（官方文档 / 书籍 / 实测 / 个人推测 …）
    pub source_type: Option<String>,
    /// 来源引用（URL、书名、出处）
    pub source_ref: Option<String>,
    /// 来源可靠性 1–5
    pub source_reliability: Option<i64>,
    /// 时效：超过该日期视为可能过期
    pub valid_until: Option<String>,
    /// 文件大小（字节）与修改时间快照：扫描时用于短路，避免无谓的读取与哈希
    pub file_size: Option<i64>,
    pub file_mtime: Option<String>,
}

/// 列表展示用的笔记摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub id: String,
    pub title: String,
    pub file_path: String,
    pub category_id: Option<String>,
    /// `技术/数据库` 形式的分类路径
    pub category_path: String,
    pub status: NoteStatus,
    pub favorite: bool,
    pub word_count: i64,
    pub created_at: String,
    pub updated_at: String,
    /// 标签名称列表
    #[serde(default)]
    pub tags: Vec<String>,
    /// 搜索结果摘要（普通列表为空）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snippet: Vec<TextSegment>,
}

/// 打开笔记时返回的完整内容。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDetail {
    pub note: Note,
    /// 不含 frontmatter 的 Markdown 正文
    pub content: String,
    pub category: Option<Category>,
    pub category_path: String,
    pub tags: Vec<Tag>,
}

/// 保存结果（设计文档 13.1 返回保存状态）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNoteResult {
    pub note: Note,
    pub content_hash: String,
    pub saved_at: String,
    pub index_updated: bool,
    /// 索引失败时为提示信息，正文仍然已保存
    pub index_warning: Option<String>,
}

/// 笔记排序方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum NoteSort {
    #[default]
    UpdatedDesc,
    UpdatedAsc,
    CreatedDesc,
    TitleAsc,
}

impl NoteSort {
    pub fn order_by(self) -> &'static str {
        // 一律追加 id 作为次级排序键：同一毫秒写入的多篇笔记在 updated_at/created_at 上完全相同，
        // 没有次级键时 SQLite 的返回顺序不确定，会让分页与快照测试出现偶发失败。
        match self {
            NoteSort::UpdatedDesc => "n.updated_at DESC, n.id DESC",
            NoteSort::UpdatedAsc => "n.updated_at ASC, n.id ASC",
            NoteSort::CreatedDesc => "n.created_at DESC, n.id DESC",
            NoteSort::TitleAsc => "n.title COLLATE NOCASE ASC, n.id ASC",
        }
    }
}

/// 笔记列表查询条件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteListQuery {
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub tag_id: Option<String>,
    #[serde(default)]
    pub status: Option<NoteStatus>,
    #[serde(default)]
    pub favorite_only: bool,
    /// 只列出 Inbox（未分类）笔记
    #[serde(default)]
    pub inbox_only: bool,
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub page_size: Option<i64>,
    #[serde(default)]
    pub sort: Option<NoteSort>,
}

impl Default for NoteListQuery {
    fn default() -> Self {
        Self {
            category_id: None,
            tag_id: None,
            status: Some(NoteStatus::Active),
            favorite_only: false,
            inbox_only: false,
            keyword: None,
            page: None,
            page_size: None,
            sort: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteListResult {
    pub items: Vec<NoteSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

/// 创建笔记输入（设计文档 14.2）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteInput {
    pub title: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub initial_content: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

/// 更新来源与时效输入（N1）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSourceInput {
    pub id: String,
    /// 来源类型：official / book / experiment / personal / unknown …
    #[serde(default)]
    pub source_type: Option<String>,
    /// 来源引用（URL、书名、出处）
    #[serde(default)]
    pub source_ref: Option<String>,
    /// 可靠性 1–5，越界会被忽略
    #[serde(default)]
    pub source_reliability: Option<i64>,
    /// 时效（ISO 8601）；超过该日期在界面提示「可能已过期」
    #[serde(default)]
    pub valid_until: Option<String>,
}

/// 保存笔记输入。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveNoteInput {
    pub id: String,
    pub content: String,
    /// 编辑开始时的内容哈希；与磁盘不一致时返回冲突，避免静默覆盖
    #[serde(default)]
    pub base_content_hash: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}
