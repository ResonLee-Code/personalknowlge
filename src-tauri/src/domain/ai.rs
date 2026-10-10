//! AI 策略与审计的 DTO（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §6、§9.7）。
//!
//! 默认口径（2026-10-10 用户确认）：
//! - AI 总开关**默认关闭**；关闭后所有 `ai_*` 命令返回 `AI_DISABLED`；
//! - 联网档位**默认仅本地**（`localOnly`）；切到 `allowNetwork` 后**首次使用仍需单次确认**；
//! - 研究预算有上限（子问题 / 抓取页 / 时长），触顶即停。

use serde::{Deserialize, Serialize};

/// 联网档位（对标语雀式的「仅本地 / 允许联网检索」两档）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum NetworkMode {
    /// 只允许本地模型与本地检索（默认）
    #[default]
    LocalOnly,
    /// 允许联网检索（仍需单次确认后才真正发起请求）
    AllowNetwork,
}

impl NetworkMode {
    pub fn as_str(self) -> &'static str {
        match self {
            NetworkMode::LocalOnly => "localOnly",
            NetworkMode::AllowNetwork => "allowNetwork",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "localOnly" | "local_only" | "local" => Some(NetworkMode::LocalOnly),
            "allowNetwork" | "allow_network" | "network" => Some(NetworkMode::AllowNetwork),
            _ => None,
        }
    }
}

/// 研究/抓取的预算上限（防止一次研究失控烧掉时间与额度）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ResearchBudget {
    /// 子问题上限（1–10）
    pub max_subquestions: u32,
    /// 单次抓取页数上限（1–50）
    pub max_pages: u32,
    /// 单次时长上限（秒，30–1800）
    pub max_duration_secs: u32,
}

impl Default for ResearchBudget {
    fn default() -> Self {
        Self { max_subquestions: 5, max_pages: 12, max_duration_secs: 480 }
    }
}

impl ResearchBudget {
    /// 与应用层的硬上限对齐（用户可调小，但不能调大）。
    pub fn clamped(&self) -> Self {
        Self {
            max_subquestions: self.max_subquestions.clamp(1, 10),
            max_pages: self.max_pages.clamp(1, 50),
            max_duration_secs: self.max_duration_secs.clamp(30, 1800),
        }
    }
}

/// AI 策略（持久化在工作区设置里）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiPolicy {
    /// AI 能力总开关（默认关闭）
    pub enabled: bool,
    /// 联网档位（默认仅本地）
    pub network: NetworkMode,
    /// 联网档位已由用户确认过一次（用于「首次使用再确认」）
    pub network_confirmed_at: Option<String>,
    /// 是否允许抓取网页正文（默认关闭；仅「搜索摘要」时可不开）
    pub allow_fetch_body: bool,
    /// 允许的域名白名单（为空 = 不预置白名单，按内置分级表判定）
    pub domain_allowlist: Vec<String>,
    /// 强制丢弃的域名黑名单
    pub domain_denylist: Vec<String>,
    /// 联网检索源地址（URL 模板，必须含 `{query}`；可选含 `{limit}`）
    ///
    /// 约定：返回 JSON（SearxNG `format=json` 或 `{ "results": [...] }` / 裸数组）。
    /// **默认为空**——应用不预置任何第三方检索服务，由用户自行配置。
    pub search_endpoint: Option<String>,
    /// 研究预算
    pub budget: ResearchBudget,
    /// 审计日志条数上限（超出后截断最旧的记录）
    pub audit_max_entries: u32,
}

impl Default for AiPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            network: NetworkMode::LocalOnly,
            network_confirmed_at: None,
            allow_fetch_body: false,
            domain_allowlist: Vec::new(),
            domain_denylist: Vec::new(),
            search_endpoint: None,
            budget: ResearchBudget::default(),
            audit_max_entries: 500,
        }
    }
}

/// 更新 AI 策略的输入（全部可选，缺省表示不改）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateAiPolicyInput {
    pub enabled: Option<bool>,
    pub network: Option<String>,
    /// 用户确认联网档位时置 true（写入时间戳）
    pub confirm_network: Option<bool>,
    pub allow_fetch_body: Option<bool>,
    pub domain_allowlist: Option<Vec<String>>,
    pub domain_denylist: Option<Vec<String>>,
    /// 检索源地址模板；传空字符串表示清除
    pub search_endpoint: Option<String>,
    pub budget: Option<ResearchBudget>,
}

/// 一条审计记录（**不含正文**，只记行为与结果）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAuditEntry {
    /// 时间（ISO 8601）
    pub at: String,
    /// 能力 / 工具名，例如 `research_start`、`web_fetch`
    pub capability: String,
    /// 是否发起了外发（联网 / 远程模型）
    pub outbound: bool,
    /// 结果：`ok` / `denied` / `error`
    pub outcome: String,
    /// 耗时毫秒
    pub duration_ms: u64,
    /// 备注（错误码或简要说明，不含正文）
    pub note: Option<String>,
}
