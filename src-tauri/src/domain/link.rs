//! 双向链接领域模型。

use serde::{Deserialize, Serialize};

/// 链接类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkType {
    /// `[[标题]]` 形式的双向链接
    Wikilink,
    /// 指向工作区内其它 Markdown 文件的相对链接
    Markdown,
    /// http/https 外链（不进入图谱，仅用于出链列表）
    Url,
}

impl LinkType {
    pub fn as_str(self) -> &'static str {
        match self {
            LinkType::Wikilink => "wikilink",
            LinkType::Markdown => "markdown",
            LinkType::Url => "url",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "markdown" => LinkType::Markdown,
            "url" => LinkType::Url,
            _ => LinkType::Wikilink,
        }
    }
}

/// 从正文中解析出的一条链接。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedLink {
    /// 链接目标文本（`[[标题|别名]]` 取「标题」）
    pub target: String,
    /// 显示别名
    pub alias: Option<String>,
    /// `[[标题#小节]]` 中的小节
    pub anchor: Option<String>,
    /// 该链接在正文中的顺序（从 0 开始）
    pub position: usize,
    /// 命中行的上下文摘要
    pub context: String,
    pub link_type: LinkType,
}

/// 引用某篇笔记的来源。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backlink {
    pub note_id: String,
    pub title: String,
    pub file_path: String,
    pub context: String,
    pub anchor: Option<String>,
    pub updated_at: String,
}

/// 一篇笔记指向外部的链接（含未解析与失效）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingLink {
    pub target_title: String,
    pub target_note_id: Option<String>,
    pub target_exists: bool,
    pub anchor: Option<String>,
    pub link_type: LinkType,
    pub context: String,
}

/// 全库范围内未解析的链接。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedLink {
    pub source_note_id: String,
    pub source_title: String,
    pub target_title: String,
    pub context: String,
}

/// 引用计数（用于元信息栏）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkCounts {
    pub backlinks: i64,
    pub outgoing: i64,
    pub unresolved: i64,
}

/// 全量重建链接的结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRebuildReport {
    pub scanned_notes: i64,
    pub links: i64,
    pub resolved: i64,
    pub unresolved: i64,
    pub duration_ms: u64,
}
