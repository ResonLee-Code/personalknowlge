//! 应用级状态：当前工作区、全局注册表与事件出口。

use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use crate::application::context::WorkspaceContext;
use crate::application::registry::WorkspaceRegistryStore;
use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::EventSink;
use crate::domain::settings::WorkspaceRegistry;

pub struct AppInner {
    pub data_dir: PathBuf,
    pub events: Arc<dyn EventSink>,
    registry: Mutex<WorkspaceRegistry>,
    active: RwLock<Option<Arc<WorkspaceContext>>>,
}

impl AppInner {
    pub fn new(data_dir: PathBuf, events: Arc<dyn EventSink>) -> Result<Arc<Self>> {
        std::fs::create_dir_all(&data_dir)?;
        let store = WorkspaceRegistryStore::new(data_dir.join("workspaces.json"));
        let registry = store.load();
        Ok(Arc::new(Self {
            data_dir,
            events,
            registry: Mutex::new(registry),
            active: RwLock::new(None),
        }))
    }

    /// 最近打开的工作区记录（快照）。
    pub fn registry_snapshot(&self) -> WorkspaceRegistry {
        self.registry
            .lock()
            .map(|registry| registry.clone())
            .unwrap_or_default()
    }

    pub fn registry_store(&self) -> WorkspaceRegistryStore {
        WorkspaceRegistryStore::new(self.data_dir.join("workspaces.json"))
    }

    pub fn set_registry(&self, registry: WorkspaceRegistry) -> Result<()> {
        let mut guard = self
            .registry
            .lock()
            .map_err(|_| AppError::internal("注册表锁异常。"))?;
        *guard = registry;
        Ok(())
    }

    pub fn active(&self) -> Option<Arc<WorkspaceContext>> {
        self.active.read().ok().and_then(|guard| guard.clone())
    }

    pub fn set_active(&self, context: Option<Arc<WorkspaceContext>>) -> Result<()> {
        let mut guard = self
            .active
            .write()
            .map_err(|_| AppError::internal("工作区状态锁异常。"))?;
        *guard = context;
        Ok(())
    }

    /// 取当前工作区上下文，未打开时返回 `NOT_INITIALIZED`。
    pub fn context(&self) -> Result<Arc<WorkspaceContext>> {
        self.active().ok_or_else(AppError::not_initialized)
    }

    /// 应用数据目录（跨平台）。
    pub fn default_data_dir() -> PathBuf {
        dirs::data_dir()
            .map(|dir| dir.join("PersonalKnowledgeOS"))
            .unwrap_or_else(|| PathBuf::from(".").join(".personal-knowledge-os"))
    }
}

/// Tauri 状态包装。
pub struct AppState {
    pub inner: Arc<AppInner>,
}

impl AppState {
    pub fn new(inner: Arc<AppInner>) -> Self {
        Self { inner }
    }

    /// 校验当前工作区可用。
    pub fn workspace_required(&self) -> Result<()> {
        if self.inner.active().is_none() {
            return Err(AppError::new(
                ErrorCode::NotInitialized,
                "请先创建或打开一个知识库。",
            ));
        }
        Ok(())
    }
}
