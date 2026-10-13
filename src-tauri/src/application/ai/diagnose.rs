//! 模型自检 / 诊断（Track C4）。
//!
//! 只回答一个问题：**这次「写报告」会用哪一档，为什么？**（在线 / 本地 / 模板）
//!
//! 三条纪律：
//! - **只读**：不改任何设置，不写审计；
//! - **只探测本机**：本地档用 `GET /api/tags` 探一次；**绝不联系在线端点**——
//!   诊断不能变成一次外发。在线档只核对「配置 + Key + 授权」状态；
//! - **口径同源**：provider 标识用 `OllamaModel::provider_id` / `RemoteModel::provider_id` /
//!   `TemplateGenerativeModel::id`，与研究报告里的 `composer` 是同一个字符串，
//!   保证「自检说的」和「报告写的」一致。
//!
//! 探测失败不是错误：只表示「这一档现在不可用」，其它档不受影响。

use std::time::Duration;

use serde::Serialize;

use crate::application::ai::local_model::OllamaModel;
use crate::application::ai::remote_model::RemoteModel;
use crate::application::ai::template_provider::TemplateGenerativeModel;
use crate::application::ai::traits::GenerativeModel;
use crate::domain::ai::AiPolicy;

/// 自检探测超时：诊断要"快"，不能像生成那样等两分钟。
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// 本地模型探测结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalProbe {
    pub reachable: bool,
    pub model_present: bool,
    pub detail: String,
}

/// 本地模型探测器（真机走 HTTP，测试用假实现）。
pub trait LocalModelProbe: Send + Sync {
    fn probe(&self, endpoint: &str, model: &str) -> LocalProbe;
}

/// 真机实现：`GET {endpoint}/api/tags`——看服务在不在、目标模型有没有 pull 下来。
pub struct HttpLocalModelProbe;

impl LocalModelProbe for HttpLocalModelProbe {
    fn probe(&self, endpoint: &str, model: &str) -> LocalProbe {
        // 只探本机：`agent_for` 对 loopback 不启用代理
        let agent = crate::core::net::agent_for(endpoint, PROBE_TIMEOUT, None);
        let response = match agent.get(format!("{endpoint}/api/tags")).call() {
            Ok(response) => response,
            Err(error) => {
                return LocalProbe {
                    reachable: false,
                    model_present: false,
                    detail: format!("连不上 {endpoint}（{error}）——Ollama 没在运行？"),
                }
            }
        };
        let text = match response.into_body().read_to_string() {
            Ok(text) => text,
            Err(error) => {
                return LocalProbe {
                    reachable: true,
                    model_present: false,
                    detail: format!("{endpoint} 有响应，但读取模型列表失败：{error}"),
                }
            }
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(_) => {
                return LocalProbe {
                    reachable: true,
                    model_present: false,
                    detail: format!("{endpoint} 有响应，但不是 Ollama 的模型列表格式"),
                }
            }
        };
        let names: Vec<String> = value
            .get("models")
            .and_then(|models| models.as_array())
            .map(|models| {
                models
                    .iter()
                    .filter_map(|item| item.get("name").and_then(|name| name.as_str()))
                    .map(|name| name.to_string())
                    .collect()
            })
            .unwrap_or_default();
        let present = model_present_in(&names, model);
        LocalProbe {
            reachable: true,
            model_present: present,
            detail: if present {
                format!("{endpoint} 可用，模型 `{model}` 已就绪")
            } else {
                format!("{endpoint} 可用，但没有模型 `{model}`（先 `ollama pull {model}`）")
            },
        }
    }
}

/// 模型名是否在列表里（Ollama 的默认标签是 `latest`）。
fn model_present_in(names: &[String], model: &str) -> bool {
    let model = model.trim();
    names
        .iter()
        .any(|name| name.trim() == model || name.trim() == format!("{model}:latest"))
}

/// 一次自检的输入（都已解析好，便于单测）。
pub struct DiagnoseInputs<'a> {
    pub policy: &'a AiPolicy,
    /// 规范化后的本地模型 `(endpoint, model)`；`None` = 未配置或地址不合法
    pub local: Option<(String, String)>,
    /// 在线模型 `(基地址, 模型名)`（不含 Key 是否在手，单独一个字段）
    pub remote_base_name: Option<(String, String)>,
    /// API Key 是否已存（只回显布尔）
    pub remote_key_present: bool,
}

/// 逐档核对结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDiagnosisCheck {
    pub tier: String,
    pub label: String,
    pub available: bool,
    pub detail: String,
}

/// 自检结论。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDiagnosis {
    pub enabled: bool,
    /// `online` | `local` | `template`
    pub tier: String,
    /// 与研究报告 `composer` 同源的 provider 标识
    pub provider_id: String,
    pub reason: String,
    pub checks: Vec<AiDiagnosisCheck>,
    /// 需要用户注意的补充说明
    pub note: Option<String>,
}

/// 判定「写报告用哪一档」。顺序与 `ResearchService::composer_candidates` 一致：
/// 在线（需已授权）→ 本地（需探测通过）→ 模板（永远可用）。
pub fn evaluate(inputs: DiagnoseInputs<'_>, probe: &dyn LocalModelProbe) -> AiDiagnosis {
    let policy = inputs.policy;
    let enabled = policy.enabled;

    // —— 在线档：只核对状态，不发任何请求 ——
    let remote_ok = enabled
        && policy.remote_model_compose
        && inputs.remote_base_name.is_some()
        && inputs.remote_key_present
        && policy.remote_model_authorized_at.is_some();
    let remote_detail = if !enabled {
        "AI 总开关关闭，不会调用".to_string()
    } else if !policy.remote_model_compose {
        "未开启「用在线模型写报告」".to_string()
    } else if inputs.remote_base_name.is_none() {
        "未配置基地址或模型名".to_string()
    } else if !inputs.remote_key_present {
        "缺 API Key（只写不读，请到「在线模型」重新填写）".to_string()
    } else if policy.remote_model_authorized_at.is_none() {
        "已配置但未授权：内容会离开本机，需在「在线模型」里确认一次".to_string()
    } else {
        "已配置、已授权，可用于写报告（内容会离开本机）".to_string()
    };

    // —— 本地档：仅当确实要用时才探一次本机 ——
    let local_probe = match (&inputs.local, enabled, policy.local_model_compose) {
        (Some((endpoint, model)), true, true) => Some(probe.probe(endpoint, model)),
        _ => None,
    };
    let local_ok = enabled
        && policy.local_model_compose
        && inputs.local.is_some()
        && local_probe
            .as_ref()
            .is_some_and(|probe| probe.reachable && probe.model_present);
    let local_detail = if !enabled {
        "AI 总开关关闭，不会调用".to_string()
    } else if !policy.local_model_compose {
        "未开启「用本地模型写报告」".to_string()
    } else if inputs.local.is_none() {
        "未配置地址或模型名（地址只接受本机）".to_string()
    } else {
        local_probe
            .as_ref()
            .map(|probe| probe.detail.clone())
            .unwrap_or_else(|| "未探测".to_string())
    };

    let template_id = TemplateGenerativeModel::new().id().to_string();

    let (tier, provider_id, reason) = if !enabled {
        (
            "template",
            template_id,
            "AI 总开关关闭：所有 AI 功能停用，不调用任何模型，报告走模板实现。".to_string(),
        )
    } else if remote_ok {
        let (base, model) = inputs.remote_base_name.clone().expect("已在 remote_ok 分支");
        (
            "online",
            RemoteModel::provider_id(&model),
            format!("在线模型已配置并授权（写报告开关已开），报告由在线模型撰写；目标主机由 {base} 决定。"),
        )
    } else if local_ok {
        let (_, model) = inputs.local.clone().expect("已在 local_ok 分支");
        (
            "local",
            OllamaModel::provider_id(&model),
            "本地模型地址可用、模型已就绪（写报告开关已开），报告由本地模型撰写，不出本机。".to_string(),
        )
    } else {
        (
            "template",
            template_id,
            "没有可用的模型（在线未配置或未授权、本地不可用），报告走模板实现，不会外发任何内容。"
                .to_string(),
        )
    };

    let note = if policy.local_model_score || policy.remote_model_score {
        Some("以上是「写报告」的档位；「打分」另有开关，自检只回答写报告用哪一档。".to_string())
    } else {
        None
    };

    AiDiagnosis {
        enabled,
        tier: tier.to_string(),
        provider_id,
        reason,
        checks: vec![
            AiDiagnosisCheck {
                tier: "online".to_string(),
                label: "在线模型（内容会离开本机）".to_string(),
                available: remote_ok,
                detail: remote_detail,
            },
            AiDiagnosisCheck {
                tier: "local".to_string(),
                label: "本地模型（只连本机）".to_string(),
                available: local_ok,
                detail: local_detail,
            },
            AiDiagnosisCheck {
                tier: "template".to_string(),
                label: "模板实现（始终可用）".to_string(),
                available: true,
                detail: "不联网、不调用模型；结论段会标注「未接模型」".to_string(),
            },
        ],
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ai::AiPolicy;

    /// 假探测器：不碰网络，直接给结论。
    struct FakeProbe(LocalProbe);
    impl LocalModelProbe for FakeProbe {
        fn probe(&self, _endpoint: &str, _model: &str) -> LocalProbe {
            self.0.clone()
        }
    }

    fn reachable() -> FakeProbe {
        FakeProbe(LocalProbe {
            reachable: true,
            model_present: true,
            detail: "stub 可用，模型已就绪".to_string(),
        })
    }

    fn unreachable() -> FakeProbe {
        FakeProbe(LocalProbe {
            reachable: false,
            model_present: false,
            detail: "连不上 http://127.0.0.1:11434——Ollama 没在运行？".to_string(),
        })
    }

    /// 场景：只配了本地模型且服务可用 → 本地档。
    #[test]
    fn stub_service_up_means_local_tier() {
        let mut policy = AiPolicy { enabled: true, ..Default::default() };
        policy.local_model_compose = true;
        policy.local_model_endpoint = Some("http://127.0.0.1:11434".to_string());
        policy.local_model_name = Some("qwen2.5:3b".to_string());

        let diagnosis = evaluate(
            DiagnoseInputs {
                policy: &policy,
                local: Some(("http://127.0.0.1:11434".to_string(), "qwen2.5:3b".to_string())),
                remote_base_name: None,
                remote_key_present: false,
            },
            &reachable(),
        );

        assert_eq!(diagnosis.tier, "local");
        assert_eq!(diagnosis.provider_id, "local.ollama:qwen2.5:3b");
        let local = diagnosis.checks.iter().find(|c| c.tier == "local").unwrap();
        assert!(local.available);
        assert!(local.detail.contains("已就绪"), "详情要说清为什么：{}", local.detail);
    }

    /// 场景：本地地址配了但服务没起来（未部署）→ 退回模板，并说清"连不上"。
    #[test]
    fn not_deployed_local_falls_back_to_template_and_says_why() {
        let mut policy = AiPolicy { enabled: true, ..Default::default() };
        policy.local_model_compose = true;
        policy.local_model_endpoint = Some("http://127.0.0.1:11434".to_string());
        policy.local_model_name = Some("qwen2.5:3b".to_string());

        let diagnosis = evaluate(
            DiagnoseInputs {
                policy: &policy,
                local: Some(("http://127.0.0.1:11434".to_string(), "qwen2.5:3b".to_string())),
                remote_base_name: None,
                remote_key_present: false,
            },
            &unreachable(),
        );

        assert_eq!(diagnosis.tier, "template");
        assert_eq!(diagnosis.provider_id, "template.draft");
        let local = diagnosis.checks.iter().find(|c| c.tier == "local").unwrap();
        assert!(!local.available);
        assert!(local.detail.contains("连不上"), "要说清是连不上：{}", local.detail);
    }

    /// 场景：在线模型配好了但**没授权** → 不能算可用，也不能悄悄外发。
    #[test]
    fn unconfirmed_remote_is_not_available() {
        let mut policy = AiPolicy { enabled: true, ..Default::default() };
        policy.remote_model_compose = true;
        policy.remote_model_base_url = Some("https://api.example.com/v1".to_string());
        policy.remote_model_name = Some("deepseek-chat".to_string());
        policy.remote_model_authorized_at = None; // 关键：未授权

        let diagnosis = evaluate(
            DiagnoseInputs {
                policy: &policy,
                local: None,
                remote_base_name: Some((
                    "https://api.example.com/v1".to_string(),
                    "deepseek-chat".to_string(),
                )),
                remote_key_present: true,
            },
            &unreachable(),
        );

        assert_eq!(diagnosis.tier, "template");
        let online = diagnosis.checks.iter().find(|c| c.tier == "online").unwrap();
        assert!(!online.available);
        assert!(online.detail.contains("未授权"), "要说清是未授权：{}", online.detail);
    }

    /// 场景：在线已授权且配置齐全 → 在线档，provider 与报告同源。
    #[test]
    fn authorized_remote_wins_over_local() {
        let mut policy = AiPolicy { enabled: true, ..Default::default() };
        policy.remote_model_compose = true;
        policy.remote_model_name = Some("deepseek-chat".to_string());
        policy.remote_model_authorized_at = Some("2026-10-12T00:00:00+08:00".to_string());
        policy.local_model_compose = true;

        let diagnosis = evaluate(
            DiagnoseInputs {
                policy: &policy,
                local: Some(("http://127.0.0.1:11434".to_string(), "qwen2.5:3b".to_string())),
                remote_base_name: Some((
                    "https://api.example.com/v1".to_string(),
                    "deepseek-chat".to_string(),
                )),
                remote_key_present: true,
            },
            &reachable(),
        );

        assert_eq!(diagnosis.tier, "online");
        assert_eq!(diagnosis.provider_id, "remote.openai-compatible:deepseek-chat");
    }

    /// 总开关关掉时，任何档都不该被说成"可用"，且不应触发探测。
    #[test]
    fn disabled_ai_never_probes_and_reports_template() {
        struct PanicProbe;
        impl LocalModelProbe for PanicProbe {
            fn probe(&self, _endpoint: &str, _model: &str) -> LocalProbe {
                panic!("AI 关闭时不应探测本地模型");
            }
        }

        let mut policy = AiPolicy { enabled: false, ..Default::default() };
        policy.local_model_compose = true;

        let diagnosis = evaluate(
            DiagnoseInputs {
                policy: &policy,
                local: Some(("http://127.0.0.1:11434".to_string(), "qwen2.5:3b".to_string())),
                remote_base_name: None,
                remote_key_present: false,
            },
            &PanicProbe,
        );

        assert_eq!(diagnosis.tier, "template");
        assert!(!diagnosis.enabled);
        assert!(diagnosis.checks.iter().all(|check| check.tier == "template" || !check.available));
    }

    #[test]
    fn model_presence_accepts_implicit_latest_tag() {
        let names = vec!["qwen2.5:latest".to_string(), "llama3:8b".to_string()];
        assert!(model_present_in(&names, "qwen2.5"));
        assert!(!model_present_in(&names, "qwen2.5:3b"));
        assert!(model_present_in(&names, "llama3:8b"));
    }
}
