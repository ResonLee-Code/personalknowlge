//! 设置模型（设计文档 16.7）。

use serde::{Deserialize, Deserializer, Serialize};

/// 宽容地把数字（整数或小数）反序列化为 `u32`。
///
/// 前端拖拽分栏时，Retina 屏上的 `clientX` 可能是小数（如 320.5），
/// 若直接按 `u32` 解析会报 "invalid args `input` for command `settings_update`"。
fn de_opt_u32<'de, D>(deserializer: D) -> std::result::Result<Option<u32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<f64>::deserialize(deserializer)?;
    Ok(value.and_then(|number| {
        if number.is_finite() && number >= 0.0 {
            Some(number.round() as u32)
        } else {
            None
        }
    }))
}

/// 宽容地把数字反序列化为 `f32`（同样兼容整数输入）。
fn de_opt_f32<'de, D>(deserializer: D) -> std::result::Result<Option<f32>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<f64>::deserialize(deserializer)?;
    Ok(value.and_then(|number| {
        if number.is_finite() {
            Some(number as f32)
        } else {
            None
        }
    }))
}

/// 宽容地把数字反序列化为 `u64`。
fn de_opt_u64<'de, D>(deserializer: D) -> std::result::Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<f64>::deserialize(deserializer)?;
    Ok(value.and_then(|number| {
        if number.is_finite() && number >= 0.0 {
            Some(number.round() as u64)
        } else {
            None
        }
    }))
}

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
    /// 笔记列表排序：`updatedDesc` | `updatedAsc` | `createdDesc` | `titleAsc` | `manual`
    pub list_sort: String,
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
            list_sort: "updatedDesc".to_string(),
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
    #[serde(default, deserialize_with = "de_opt_u64")]
    pub autosave_delay_ms: Option<u64>,
    pub editor_theme: Option<String>,
    #[serde(default, deserialize_with = "de_opt_u32")]
    pub font_size: Option<u32>,
    pub default_new_location: Option<String>,
    pub backup_dir: Option<String>,
    pub editor_mode: Option<String>,
    /// 分屏模式下编辑器占比（0.3 ~ 0.7）
    #[serde(default, deserialize_with = "de_opt_f32")]
    pub editor_split_ratio: Option<f32>,
    #[serde(default, deserialize_with = "de_opt_u32")]
    pub sidebar_width: Option<u32>,
    #[serde(default, deserialize_with = "de_opt_u32")]
    pub list_width: Option<u32>,
    /// 笔记列表排序（持久化「自定义顺序」等选择）
    pub list_sort: Option<String>,
    pub last_opened_note_id: Option<String>,
    pub last_category_id: Option<String>,
    pub theme: Option<String>,
}
