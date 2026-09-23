//! 应用级工作区注册表（最近打开的知识库）。
//!
//! 保存在用户数据目录，不依赖任何工作区，因此可以在未打开工作区时读取。

use std::path::PathBuf;

use crate::core::error::Result;
use crate::core::time::now_iso;
use crate::domain::settings::{
    GlobalSettings, WorkspaceRegistry, WorkspaceRegistryEntry,
};
use crate::domain::workspace::Workspace;

pub struct WorkspaceRegistryStore {
    path: PathBuf,
}

impl WorkspaceRegistryStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// 读取注册表。文件缺失或损坏时返回默认值，不影响应用启动。
    pub fn load(&self) -> WorkspaceRegistry {
        match std::fs::read_to_string(&self.path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => WorkspaceRegistry::default(),
        }
    }

    pub fn save(&self, registry: &WorkspaceRegistry) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(registry)?;
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(&temp, content)?;
        std::fs::rename(&temp, &self.path)?;
        Ok(())
    }

    pub fn upsert(&self, workspace: &Workspace) -> Result<WorkspaceRegistry> {
        let mut registry = self.load();
        registry.workspaces.retain(|entry| entry.id != workspace.id);
        registry.workspaces.insert(
            0,
            WorkspaceRegistryEntry {
                id: workspace.id.clone(),
                name: workspace.name.clone(),
                root_path: workspace.root_path.clone(),
                last_opened_at: now_iso(),
            },
        );
        registry.workspaces.truncate(20);
        self.save(&registry)?;
        Ok(registry)
    }

    pub fn remove(&self, id: &str) -> Result<WorkspaceRegistry> {
        let mut registry = self.load();
        registry.workspaces.retain(|entry| entry.id != id);
        self.save(&registry)?;
        Ok(registry)
    }

    pub fn find_by_root(&self, root: &str) -> Option<WorkspaceRegistryEntry> {
        self.load()
            .workspaces
            .into_iter()
            .find(|entry| entry.root_path == root)
    }

    pub fn update_global_settings(&self, mutate: impl FnOnce(&mut GlobalSettings)) -> Result<WorkspaceRegistry> {
        let mut registry = self.load();
        mutate(&mut registry.settings);
        self.save(&registry)?;
        Ok(registry)
    }
}
