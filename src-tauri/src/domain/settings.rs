//! 设置模型（设计文档 16.7）。

use serde::{Deserialize, Serialize};

/// 工作区级设置，保存在 `settings` 表。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub autosave_enabled: bool,
    pub autosave_delay_ms: u64,
    pub editor_theme: String,
    pub font_size: u32,
    /// `inbox` | `currentCategory` | `lastCategory`
    pub default_new_location: String,
    pub backup_dir: Option<String>,
    /// 编辑器默认模式：`edit` | `split` | `preview`
    pub editor_mode: String,
    /// 分屏模式下编辑器占比（0.3 ~ 0.7）
    pub editor_split_ratio: f32,
    pub sidebar_width: u32,
    pub list_width: u32,
    /// 关闭前最后打开的笔记，便于恢复现场
    pub last_opened_note_id: Option<String>,
    /// 最近一次使用的分类
    pub last_category_id: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            autosave_enabled: true,
            autosave_delay_ms: 1000,
            editor_theme: "light".to_string(),
            font_size: 15,
            default_new_location: "inbox".to_string(),
            backup_dir: None,
            editor_mode: "split".to_string(),
            editor_split_ratio: 0.5,
            sidebar_width: 240,
            list_width: 320,
            last_opened_note_id: None,
            last_category_id: None,
        }
    }
}

/// 应用级设置，保存在用户数据目录，独立于任何工作区。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSettings {
    pub default_workspace_root: Option<String>,
    /// 最近打开的工作区根目录（按时间倒序）
    pub recent_workspace_roots: Vec<String>,
    pub theme: String,
    pub language: String,
}

/// 最近打开的工作区。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRegistryEntry {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub last_opened_at: String,
}

/// 应用级注册表内容。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRegistry {
    pub workspaces: Vec<WorkspaceRegistryEntry>,
    pub settings: GlobalSettings,
}

/// 更新设置的输入（缺省字段表示不修改）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct UpdateSettingsInput {
    pub autosave_enabled: Option<bool>,
    pub autosave_delay_ms: Option<u64>,
    pub editor_theme: Option<String>,
    pub font_size: Option<u32>,
    pub default_new_location: Option<String>,
    pub backup_dir: Option<String>,
    pub editor_mode: Option<String>,
    /// 分屏模式下编辑器占比（0.3 ~ 0.7）
    pub editor_split_ratio: Option<f32>,
    pub sidebar_width: Option<u32>,
    pub list_width: Option<u32>,
    pub last_opened_note_id: Option<String>,
    pub last_category_id: Option<String>,
    pub theme: Option<String>,
}
