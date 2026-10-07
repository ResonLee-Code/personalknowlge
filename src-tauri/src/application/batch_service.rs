//! 批量笔记操作（V1.1.2）。
//!
//! 多选移动 / 打标签 / 删除都会走这里：逐篇复用常规服务（保证链接、索引、关系等联动一致），
//! 但只在整批结束后刷新一次列表与统计，避免 N 次 UI 抖动。
//! 单篇失败不会中断整批，失败原因逐条返回。

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::application::note_service::NoteService;
use crate::core::error::Result;

use super::context::WorkspaceContext;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BatchReport {
    /// 成功处理的笔记数
    pub succeeded: usize,
    /// 逐条失败原因（笔记 id + 后端 message）
    pub failures: Vec<BatchFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchFailure {
    pub note_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchMoveInput {
    pub ids: Vec<String>,
    /// 目标分类；`null` 表示移回 Inbox（未分类）
    #[serde(default)]
    pub category_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchTagInput {
    pub ids: Vec<String>,
    pub tags: Vec<String>,
    /// `add`（默认，合并）| `remove`（移除给定标签）| `replace`（整体替换）
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDeleteInput {
    pub ids: Vec<String>,
}

pub struct BatchService {
    ctx: Arc<WorkspaceContext>,
}

impl BatchService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    pub fn move_notes(&self, input: BatchMoveInput) -> Result<BatchReport> {
        let service = NoteService::new(Arc::clone(&self.ctx));
        let mut report = BatchReport::default();
        for id in dedup(&input.ids) {
            match service.move_to_category(&id, input.category_id.as_deref()) {
                Ok(_) => report.succeeded += 1,
                Err(error) => report.failures.push(BatchFailure { note_id: id, message: error.message }),
            }
        }
        Ok(report)
    }

    pub fn tag_notes(&self, input: BatchTagInput) -> Result<BatchReport> {
        let service = NoteService::new(Arc::clone(&self.ctx));
        let mode = input.mode.as_deref().unwrap_or("add");
        let mut report = BatchReport::default();
        for id in dedup(&input.ids) {
            let current = match service.get(&id) {
                Ok(detail) => detail.tags.into_iter().map(|tag| tag.name).collect::<Vec<_>>(),
                Err(error) => {
                    report.failures.push(BatchFailure { note_id: id, message: error.message });
                    continue;
                }
            };
            let next: Vec<String> = match mode {
                "remove" => current
                    .into_iter()
                    .filter(|name| !input.tags.iter().any(|tag| tag.eq_ignore_ascii_case(name)))
                    .collect(),
                "replace" => input.tags.clone(),
                _ => {
                    let mut merged = current;
                    for tag in &input.tags {
                        if !merged.iter().any(|name| name.eq_ignore_ascii_case(tag)) {
                            merged.push(tag.clone());
                        }
                    }
                    merged
                }
            };
            match service.set_tags(&id, &next) {
                Ok(_) => report.succeeded += 1,
                Err(error) => report.failures.push(BatchFailure { note_id: id, message: error.message }),
            }
        }
        Ok(report)
    }

    pub fn delete_notes(&self, input: BatchDeleteInput) -> Result<BatchReport> {
        let service = NoteService::new(Arc::clone(&self.ctx));
        let mut report = BatchReport::default();
        for id in dedup(&input.ids) {
            match service.delete(&id) {
                Ok(_) => report.succeeded += 1,
                Err(error) => report.failures.push(BatchFailure { note_id: id, message: error.message }),
            }
        }
        Ok(report)
    }
}

/// 去重并保持顺序，避免同一篇笔记被处理多次。
fn dedup(ids: &[String]) -> Vec<String> {
    let mut seen = Vec::new();
    for id in ids {
        if !seen.contains(id) {
            seen.push(id.clone());
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_keeps_order_and_removes_duplicates() {
        let ids = vec!["a".to_string(), "b".to_string(), "a".to_string()];
        assert_eq!(dedup(&ids), vec!["a".to_string(), "b".to_string()]);
    }
}
