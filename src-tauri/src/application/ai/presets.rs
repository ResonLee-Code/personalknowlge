//! 本地模型的「低配机型预设」（Track C3）。
//!
//! 为什么要它：本地模型的门槛一半不在部署，而在**选哪个模型**——名字写错、
//! 或者一上来就拉一个跑不动的大家伙，用户会直接下结论「本地模型不能用」。
//! 这里给两档经过体积评估的常见选择，让「填一次就能用」成立。
//!
//! 纪律：**预设值只在这里写一份**（前端只负责展示与填充，不再硬编码）。
//! 体积是 Ollama 官方库的**近似下载体积**，会随量化版本变化，只用于"心里有数"，
//! 界面与文案都必须写明「以实际下载为准」，不得当成精确值使用。

use serde::Serialize;

/// 一条本地模型预设。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalModelPreset {
    /// Ollama 模型名（可直接 `ollama pull`）
    pub name: &'static str,
    /// 展示用标签
    pub label: &'static str,
    /// 近似下载体积（字节）；仅用于粗估与排序，不作为精确值
    pub approx_bytes: u64,
    /// 展示用的体积文案
    pub approx_size: &'static str,
    /// 内存提示（定性，不编造具体阈值）
    pub memory_hint: &'static str,
    /// 一句话说明适合谁
    pub note: &'static str,
}

/// 预设表：由小到大（低配在前）。
pub const LOCAL_MODEL_PRESETS: &[LocalModelPreset] = &[
    LocalModelPreset {
        name: "qwen2.5:3b",
        label: "Qwen2.5 3B（轻量）",
        approx_bytes: 1_900_000_000,
        approx_size: "≈1.9 GB",
        memory_hint: "内存吃紧时选它",
        note: "低配机首选；中文写作与摘要够用，长文理解弱于 7B",
    },
    LocalModelPreset {
        name: "qwen2.5:7b",
        label: "Qwen2.5 7B（均衡）",
        approx_bytes: 4_700_000_000,
        approx_size: "≈4.7 GB",
        memory_hint: "内存充裕时更稳",
        note: "质量更稳；首次加载更慢，磁盘占用更大",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_non_empty_and_well_formed() {
        assert!(!LOCAL_MODEL_PRESETS.is_empty(), "预设表不能是空的");
        for preset in LOCAL_MODEL_PRESETS {
            // 必须是可直接 `ollama pull` 的名字（`模型:标签`），否则填进去也拉不下来
            assert!(
                preset.name.contains(':') && !preset.name.starts_with(':') && !preset.name.ends_with(':'),
                "预设名不是 `模型:标签` 形状：{}",
                preset.name
            );
            assert!(!preset.label.trim().is_empty(), "{} 缺标签", preset.name);
            assert!(preset.approx_bytes > 0, "{} 的体积必须是正数", preset.name);
            assert!(!preset.approx_size.trim().is_empty(), "{} 缺体积文案", preset.name);
            assert!(!preset.memory_hint.trim().is_empty(), "{} 缺内存提示", preset.name);
            assert!(!preset.note.trim().is_empty(), "{} 缺说明", preset.name);
        }
    }

    #[test]
    fn preset_names_are_unique_and_sorted_by_size() {
        let mut names: Vec<&str> = LOCAL_MODEL_PRESETS.iter().map(|p| p.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "预设名不能重复");

        let sizes: Vec<u64> = LOCAL_MODEL_PRESETS.iter().map(|p| p.approx_bytes).collect();
        assert!(
            sizes.windows(2).all(|pair| pair[0] <= pair[1]),
            "预设应按体积从小到大排列（低配在前）：{sizes:?}"
        );
    }

    #[test]
    fn every_preset_serializes_to_camel_case_fields() {
        let json = serde_json::to_value(LOCAL_MODEL_PRESETS[0]).expect("序列化预设");
        for field in ["name", "label", "approxBytes", "approxSize", "memoryHint", "note"] {
            assert!(json.get(field).is_some(), "序列化缺字段 {field}：{json}");
        }
    }
}
