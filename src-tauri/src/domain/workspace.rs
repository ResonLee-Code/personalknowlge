//! 工作区领域模型（设计文档 10.1）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 工作区统计信息，用于首页与工作区切换器。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStats {
    pub note_count: i64,
    pub inbox_count: i64,
    pub trash_count: i64,
    pub category_count: i64,
    pub tag_count: i64,
    pub attachment_count: i64,
    /// 全部活跃笔记的正文字数合计（V1.2.1 统计明细）
    #[serde(default)]
    pub total_words: i64,
    /// 最近 7 天有更新的笔记数
    #[serde(default)]
    pub updated_last_7d: i64,
    /// 最近 30 天有更新的笔记数
    #[serde(default)]
    pub updated_last_30d: i64,
    /// 置顶笔记数
    #[serde(default)]
    pub pinned_count: i64,
}

/// 工作区列表项。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub workspace: Workspace,
    pub stats: WorkspaceStats,
    pub is_active: bool,
    /// 目录当前是否可访问（例如移动硬盘被拔掉时）
    pub is_available: bool,
}

/// 打开工作区后的返回结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub workspace: Workspace,
    pub stats: WorkspaceStats,
    pub settings: super::settings::AppSettings,
    pub index_status: super::search::IndexStatus,
    /// 打开时扫描发现的问题（例如缺失文件），用于提示用户
    pub warnings: Vec<String>,
}
