//! 事件出口。
//!
//! 应用服务不直接依赖 Tauri，而是通过 `EventSink` 推送事件，
//! 这样服务层可以在没有 UI 的测试环境中运行（设计文档 8.2）。

use std::sync::Arc;

use serde_json::Value;

/// 前端需要监听的事件名。
pub mod events {
    /// 工作区内文件发生变化（外部编辑）。
    pub const FILES_CHANGED: &str = "workspace://files-changed";
    /// 索引状态变化。
    pub const INDEX_STATUS: &str = "search://index-status";
    /// 工作区元数据变化（笔记新增/删除/移动）。
    pub const NOTES_CHANGED: &str = "notes://changed";
}

pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: Value);
}

/// 无副作用实现，用于单元测试与集成测试。
pub struct NoopEventSink;

impl EventSink for NoopEventSink {
    fn emit(&self, _event: &str, _payload: Value) {}
}

/// 基于 Tauri 事件系统的实现。
pub struct TauriEventSink {
    app: tauri::AppHandle,
}

impl TauriEventSink {
    pub fn new(app: tauri::AppHandle) -> Arc<Self> {
        Arc::new(Self { app })
    }
}

impl EventSink for TauriEventSink {
    fn emit(&self, event: &str, payload: Value) {
        use tauri::Emitter;
        if let Err(error) = self.app.emit(event, payload) {
            tracing::warn!(target: "events", "事件推送失败 {event}: {error}");
        }
    }
}
