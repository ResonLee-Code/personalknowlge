//! 标签领域模型（设计文档 10.4）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub created_at: String,
    pub note_count: i64,
}
