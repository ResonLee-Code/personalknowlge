//! 分类领域模型（设计文档 10.3）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub workspace_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    /// 目录名（相对工作区根目录）
    pub dir_name: String,
    pub note_count: i64,
}

/// 分类树节点。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryNode {
    pub category: Category,
    pub depth: usize,
    pub children: Vec<CategoryNode>,
}

/// 分类路径中的一段。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CategoryPathItem {
    pub id: String,
    pub name: String,
}

/// 删除分类时如何处理其下的笔记（设计文档 FR-C03）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CategoryDeleteStrategy {
    /// 笔记移动到 Inbox
    MoveToInbox,
    /// 笔记移动到父分类
    MoveToParent,
    /// 仅删除分类，笔记不再属于任何分类
    Unassign,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCategoryInput {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    /// 是否在磁盘上创建目录（默认 true）
    #[serde(default)]
    pub create_directory: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCategoryInput {
    pub id: String,
    pub strategy: CategoryDeleteStrategy,
}
