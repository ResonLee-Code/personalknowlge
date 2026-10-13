//! AI 策略闸门与审计（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §6、§9.7）。
//!
//! 职责：
//! - 读写工作区设置里的 `AiPolicy`（默认最保守：总开关关闭、联网仅本地）；
//! - 提供**唯一的**闸门函数：`ensure_enabled` / `ensure_network_allowed` / `ensure_fetch_body_allowed`
//!   —— 任何 AI 能力与外发动作都必须先过这里，禁止在业务代码里散落判断；
//! - 审计：把每次动作追加到 `.knowledge/logs/ai-audit.jsonl`（**不含正文**），
//!   超过 `audit_max_entries` 后保留最新的若干条。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::time::now_iso;
use crate::domain::ai::{AiAuditEntry, AiPolicy, NetworkMode, UpdateAiPolicyInput};
use crate::infrastructure::credential_store;

use super::host::host_of;
use super::super::context::WorkspaceContext;

/// 审计文件名（放在既有 `.knowledge/logs/` 下）。
const AUDIT_FILE: &str = "ai-audit.jsonl";

/// 决策档的默认端点（Perplexity Decisions；Jev 同结构，只是换 base_url）。
pub const SYSTEM_ONE_DEFAULT_BASE: &str = "https://api.perplexity.ai";
/// Perplexity Decisions 的默认模型。
pub const PERPLEXITY_DECIDER_MODEL: &str = "pplx-decider-v1.1-27b";
/// TypeSafe Jev 的默认模型。
pub const JEV_DEFAULT_MODEL: &str = "jev-latest";

/// 按端点猜默认模型：TypeSafe 的域名 → `jev-latest`，其余 → Perplexity 的 decider。
///
/// 之所以"按域名猜"：两者的请求/响应结构完全一致，唯一的差别就是端点与模型名，
/// 所以只用一个 `system_one` 档 + 两个预设即可，不必再分两个 provider。
pub fn default_decision_model(base_url: &str) -> &'static str {
    let lowered = base_url.to_ascii_lowercase();
    if lowered.contains("typesafe") {
        JEV_DEFAULT_MODEL
    } else {
        PERPLEXITY_DECIDER_MODEL
    }
}

/// 决策档的生效配置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionConfig {
    pub base_url: String,
    pub model: String,
    pub key: String,
    /// 出站代理（为空 = 跟随系统/环境）
    pub proxy: Option<String>,
}

pub struct AiPolicyService {
    ctx: Arc<WorkspaceContext>,
}

/// 单个域名相对当前策略的裁决结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostVerdict {
    Allowed,
    /// 命中黑名单：强制丢弃
    DeniedByBlacklist,
    /// 白名单非空且不含该域名：丢弃
    NotInAllowlist,
}

impl AiPolicyService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    /// 当前策略。
    ///
    /// 回显时补上「是否已配置 API Key」——Key 本身存在应用数据目录（`credential_store`），
    /// **任何 IPC 响应、日志与审计里都不会出现 Key**。
    pub fn policy(&self) -> AiPolicy {
        let mut policy = self.ctx.settings().ai;
        policy.remote_model_key_set = credential_store::remote_model_key_set(&self.ctx.data_dir);
        policy.decision_key_set = credential_store::decision_key_set(&self.ctx.data_dir);
        policy
    }

    /// 更新策略；`confirmNetwork = true` 时记录确认时间（用于「首次使用再确认」）。
    pub fn update(&self, input: UpdateAiPolicyInput) -> Result<AiPolicy> {
        let updated = self.ctx.update_settings(|settings| {
            apply_policy(&mut settings.ai, &input);
        })?;
        let mut policy = updated.ai;
        policy.remote_model_key_set = credential_store::remote_model_key_set(&self.ctx.data_dir);
        policy.decision_key_set = credential_store::decision_key_set(&self.ctx.data_dir);
        Ok(policy)
    }

    /// 决策档的**生效配置**（D4）：provider 预设 + 用户覆盖。
    ///
    /// 只有「开了、配好、已授权」时才返回 `Some`——**不构造 Provider 就一个字节都不会发**，
    /// 与在线模型/向量档同一套结构性纪律。
    pub fn decision_config(&self) -> Option<DecisionConfig> {
        let policy = self.policy();
        if !policy.enabled || policy.decision_provider.as_deref() != Some("system_one") {
            return None;
        }
        if policy.decision_authorized_at.is_none() {
            return None;
        }
        let key = credential_store::decision_key(&self.ctx.data_dir)?;
        let base_url = policy
            .decision_base_url
            .clone()
            .unwrap_or_else(|| SYSTEM_ONE_DEFAULT_BASE.to_string());
        let base_url = normalize_remote_base_url(&base_url)?;
        let model = policy
            .decision_model
            .clone()
            .unwrap_or_else(|| default_decision_model(&base_url).to_string());
        if model.trim().is_empty() {
            return None;
        }
        Some(DecisionConfig {
            base_url,
            model,
            key,
            proxy: policy.outbound_proxy.clone(),
        })
    }

    /// 闸门一：AI 总开关（关闭时所有能力直接拒绝）。
    pub fn ensure_enabled(&self, capability: &str) -> Result<AiPolicy> {
        let policy = self.policy();
        if !policy.enabled {
            self.audit(capability, false, "denied", 0, Some("AI_DISABLED"));
            return Err(AppError::new(
                ErrorCode::AiDisabled,
                "AI 能力未开启：请到「设置 → AI」打开总开关（默认关闭，且不会自动联网）。",
            ));
        }
        Ok(policy)
    }

    /// 闸门二：联网动作（档位 + 本次确认 + 域名白/黑名单）。
    ///
    /// 检索结果里的**每一条候选**也要按同一套域名规则过一遍（见 `host_verdict`）：
    /// 抓正文时会被拦，但默认不抓正文（只用检索摘要），所以候选本身必须单独判定。
    ///
    /// `confirmed` 表示调用方已拿到用户的**单次确认**；档位为 `allowNetwork` 且已确认才放行。
    pub fn ensure_network_allowed(
        &self,
        capability: &str,
        url: &str,
        confirmed: bool,
    ) -> Result<AiPolicy> {
        let policy = self.ensure_enabled(capability)?;
        if policy.network != NetworkMode::AllowNetwork {
            self.audit(capability, false, "denied", 0, Some("NETWORK_DISABLED"));
            return Err(AppError::new(
                ErrorCode::NetworkDisabled,
                "联网检索当前是「仅本地」：需要到「设置 → AI」切到「允许联网检索」，并在首次使用时确认。",
            ));
        }
        if policy.network_confirmed_at.is_none() || !confirmed {
            self.audit(capability, false, "denied", 0, Some("NETWORK_UNCONFIRMED"));
            return Err(AppError::new(
                ErrorCode::NetworkDisabled,
                "本次联网尚未确认：请在弹出的确认里选择「仅此一次」或「记住」，本次不会发起任何请求。",
            ));
        }
        if let Some(host) = host_of(url) {
            if policy
                .domain_denylist
                .iter()
                .any(|rule| domain_matches(rule, &host))
            {
                self.audit(capability, false, "denied", 0, Some("DOMAIN_DENIED"));
                return Err(AppError::new(
                    ErrorCode::NetworkDisabled,
                    format!("域名 {host} 在黑名单里，已拒绝抓取。"),
                ));
            }
            // 白名单为空 = 不预置白名单（按内置分级表判定）；非空时只允许白名单内域名
            if !policy.domain_allowlist.is_empty()
                && !policy
                    .domain_allowlist
                    .iter()
                    .any(|rule| domain_matches(rule, &host))
            {
                self.audit(capability, false, "denied", 0, Some("DOMAIN_NOT_ALLOWED"));
                return Err(AppError::new(
                    ErrorCode::NetworkDisabled,
                    format!("域名 {host} 不在白名单里，已拒绝抓取（可在「设置 → AI」里调整）。"),
                ));
            }
        }
        Ok(policy)
    }

    /// 闸门三：抓取正文（默认关闭时只允许搜索摘要）。
    ///
    /// 与闸门二共用同一套域名规则（`domain_matches`），供**检索结果的逐条过滤**使用：
    /// 黑名单强制丢弃；白名单非空时只放行白名单内域名。
    pub fn host_verdict(&self, url: &str) -> HostVerdict {
        let policy = self.policy();
        let Some(host) = host_of(url) else {
            // 地址解析不出主机（本地文件、相对路径等）：不参与域名规则判定
            return HostVerdict::Allowed;
        };
        if policy
            .domain_denylist
            .iter()
            .any(|rule| domain_matches(rule, &host))
        {
            return HostVerdict::DeniedByBlacklist;
        }
        if !policy.domain_allowlist.is_empty()
            && !policy
                .domain_allowlist
                .iter()
                .any(|rule| domain_matches(rule, &host))
        {
            return HostVerdict::NotInAllowlist;
        }
        HostVerdict::Allowed
    }

    pub fn ensure_fetch_body_allowed(&self, capability: &str) -> Result<()> {
        let policy = self.policy();
        if !policy.allow_fetch_body {
            self.audit(capability, false, "denied", 0, Some("FETCH_BODY_DISABLED"));
            return Err(AppError::new(
                ErrorCode::NetworkDisabled,
                "抓取网页正文未开启：请在「设置 → AI」打开（默认关闭，可只用搜索摘要）。",
            ));
        }
        Ok(())
    }

    /// 可用的本地模型配置（地址 + 模型名）。
    ///
    /// **读的时候再校验一次地址**：写入路径（`apply_policy`）已经规范化过，
    /// 但设置是从工作区数据库读出来的——多一道防线，避免「某条未来的写入路径忘了规范化」
    /// 或「有人直接改了库」时，笔记内容被发到远程地址上去。
    pub fn local_model(&self) -> Option<(String, String)> {
        let policy = self.policy();
        let (endpoint, model) = policy.local_model()?;
        let normalized = normalize_local_endpoint(endpoint)?;
        Some((normalized, model.to_string()))
    }

    /// 可用的在线模型配置（基地址 + 模型名 + API Key）。
    ///
    /// **读的时候再校验一次地址**（纵深防御，口径与本地模型一致）：即使有人绕过写入路径
    /// 直接改了设置库，也不会把笔记片段发到一个没被规范化过的地址上。
    /// 三项缺一不可：缺地址/模型名 = 没配置，缺 Key = 调不动（不算「可用」）。
    pub fn remote_model(&self) -> Option<(String, String, String)> {
        let policy = self.ctx.settings().ai;
        let (base, model) = policy.remote_model()?;
        let normalized = normalize_remote_base_url(base)?;
        let key = credential_store::remote_model_key(&self.ctx.data_dir)?;
        Some((normalized, model.to_string(), key))
    }

    /// 自检用：在线模型的 `(基地址, 模型名)`（已规范化）与「API Key 是否在手」。
    ///
    /// 与 `remote_model()` 的区别：**不要求配置完整**——自检恰恰要能回答「缺哪一项」，
    /// 所以缺什么就如实回什么（缺地址/模型名 → `None`；缺 Key → `false`）。
    pub fn remote_diagnosis(&self) -> (Option<(String, String)>, bool) {
        let policy = self.ctx.settings().ai;
        let base_name = policy.remote_model().and_then(|(base, model)| {
            normalize_remote_base_url(base).map(|base| (base, model.to_string()))
        });
        let key_present = credential_store::remote_model_key(&self.ctx.data_dir).is_some();
        (base_name, key_present)
    }

    /// 逐次外发确认（Track C2）：`confirm_each_outbound` 打开时，**每一次**外发都要显式确认。
    ///
    /// 返回 `false` 时调用方**不得构造在线档**——"未确认 = 零请求"是结构保证，
    /// 而不是靠调用时记得判断。
    pub fn outbound_confirmed(&self, confirmed: bool) -> bool {
        !self.policy().confirm_each_outbound || confirmed
    }

    /// 设置里显式配置的出站代理（为空 = 交给环境变量 / 系统代理，见 `core::net`）。
    pub fn outbound_proxy(&self) -> Option<String> {
        self.ctx
            .settings()
            .ai
            .outbound_proxy
            .filter(|value| !value.trim().is_empty())
    }

    /// 闸门四：在线模型外发（总开关 + 配置完整 + 已授权）。
    ///
    /// **与「联网检索」档位是两件事**：档位管的是「去外面抓东西」（搜索与抓正文、按域名白黑名单
    /// 判定）；这一条管的是「把笔记片段交给外部模型」。所以它有自己的首次授权，
    /// 而且**基地址或模型名一变就作废**（见 `apply_policy`）——换了收件人就要重新确认。
    ///
    /// 每次拒绝都记 `outbound=false` 的审计：拒绝本身也必须是可追溯的。
    pub fn ensure_remote_model_allowed(&self, capability: &str) -> Result<AiPolicy> {
        let policy = self.ensure_enabled(capability)?;
        if self.remote_model().is_none() {
            self.audit(capability, false, "denied", 0, Some("REMOTE_MODEL_UNCONFIGURED"));
            return Err(AppError::new(
                ErrorCode::AiDisabled,
                "在线模型未配置完整：请到「设置 → AI → 在线模型」填写基地址、模型名与 API Key。",
            ));
        }
        if policy.remote_model_authorized_at.is_none() {
            self.audit(capability, false, "denied", 0, Some("REMOTE_MODEL_UNCONFIRMED"));
            return Err(AppError::new(
                ErrorCode::NetworkDisabled,
                "在线模型尚未授权：内容会离开本机，需在「设置 → AI → 在线模型」里确认一次；\
                 换地址或模型名后要重新确认。",
            ));
        }
        Ok(policy)
    }

    /// 追加一条审计（失败不影响主流程，只记 debug 日志）。
    pub fn audit(
        &self,
        capability: &str,
        outbound: bool,
        outcome: &str,
        duration_ms: u64,
        note: Option<&str>,
    ) {
        let entry = AiAuditEntry {
            at: now_iso(),
            capability: capability.to_string(),
            outbound,
            outcome: outcome.to_string(),
            duration_ms,
            note: note.map(|value| value.to_string()),
        };
        if let Err(error) = self.append(&entry) {
            tracing::debug!(target: "ai", "写审计失败：{}", error.message);
        }
    }

    /// 读取最近 N 条审计（新的在前）。
    pub fn audit_tail(&self, limit: usize) -> Result<Vec<AiAuditEntry>> {
        let path = self.audit_path();
        let Ok(raw) = fs::read_to_string(&path) else {
            return Ok(Vec::new());
        };
        let mut entries: Vec<AiAuditEntry> = raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        entries.reverse();
        entries.truncate(limit.max(1));
        Ok(entries)
    }

    fn append(&self, entry: &AiAuditEntry) -> Result<()> {
        let path = self.audit_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut lines: Vec<String> = match fs::read_to_string(&path) {
            Ok(raw) => raw
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(str::to_string)
                .collect(),
            Err(_) => Vec::new(),
        };
        lines.push(serde_json::to_string(entry).unwrap_or_else(|_| "{}".to_string()));

        let limit = self.policy().audit_max_entries.max(50) as usize;
        if lines.len() > limit {
            let drop = lines.len() - limit;
            lines.drain(0..drop);
        }
        let text = lines.join("\n") + "\n";
        self.ctx
            .storage
            .write_atomic_absolute(&path, text.as_bytes())
    }

    fn audit_path(&self) -> PathBuf {
        self.ctx.paths.log_dir().join(AUDIT_FILE)
    }
}

/// 把更新输入应用到策略上（纯函数，便于单测）。
pub fn apply_policy(policy: &mut AiPolicy, input: &UpdateAiPolicyInput) {
    if let Some(enabled) = input.enabled {
        policy.enabled = enabled;
    }
    if let Some(network) = input.network.as_deref().and_then(NetworkMode::parse) {
        // 档位变化时清掉旧的确认：切到联网必须重新确认一次
        if policy.network != network {
            policy.network_confirmed_at = None;
        }
        policy.network = network;
    }
    if input.confirm_network == Some(true) && policy.network == NetworkMode::AllowNetwork {
        policy.network_confirmed_at = Some(now_iso());
    }
    if let Some(allow) = input.allow_fetch_body {
        policy.allow_fetch_body = allow;
    }
    if let Some(list) = &input.domain_allowlist {
        policy.domain_allowlist = normalize_domains(list);
    }
    if let Some(list) = &input.domain_denylist {
        policy.domain_denylist = normalize_domains(list);
    }
    if let Some(endpoint) = &input.search_endpoint {
        policy.search_endpoint = normalize_search_endpoint(endpoint);
    }
    if let Some(endpoint) = &input.local_model_endpoint {
        policy.local_model_endpoint = normalize_local_endpoint(endpoint);
    }
    if let Some(name) = &input.local_model_name {
        let trimmed = name.trim();
        policy.local_model_name = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    if let Some(enabled) = input.local_model_compose {
        policy.local_model_compose = enabled;
    }
    if let Some(enabled) = input.local_model_score {
        policy.local_model_score = enabled;
    }
    if let Some(base) = &input.remote_model_base_url {
        let next = normalize_remote_base_url(base);
        // 地址变更 = 换了个「内容会发给谁」的收件人：旧授权立即作废
        if policy.remote_model_base_url != next {
            policy.remote_model_authorized_at = None;
            // 向量档与在线模型共用地址：换了收件人，向量授权同样作废
            policy.vector_authorized_at = None;
        }
        policy.remote_model_base_url = next;
    }
    if let Some(name) = &input.remote_model_name {
        let trimmed = name.trim();
        let next = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        if policy.remote_model_name != next {
            policy.remote_model_authorized_at = None;
        }
        policy.remote_model_name = next;
    }
    if let Some(enabled) = input.remote_model_compose {
        policy.remote_model_compose = enabled;
    }
    if let Some(enabled) = input.remote_model_score {
        policy.remote_model_score = enabled;
    }
    if input.confirm_remote_model == Some(true) && policy.remote_model().is_some() {
        // 首次（或换地址后）授权：地址与模型名都填好才允许记时间戳，
        // 否则会出现「授权了一个空配置」的假状态。
        policy.remote_model_authorized_at = Some(now_iso());
    }
    if let Some(enabled) = input.confirm_each_outbound {
        // 逐次外发确认（Track C2）：与「首次授权」是两件事，互不影响。
        policy.confirm_each_outbound = enabled;
    }
    if let Some(enabled) = input.vector_enabled {
        policy.vector_enabled = enabled;
    }
    if let Some(model) = &input.remote_embedding_model {
        let trimmed = model.trim();
        let next = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        // 换嵌入模型 = 换了「谁来算向量」：旧授权与旧向量都作废（向量由 vector_index 清）
        if policy.remote_embedding_model != next {
            policy.vector_authorized_at = None;
        }
        policy.remote_embedding_model = next;
    }
    if input.confirm_vector == Some(true)
        && policy.remote_model().is_some()
        && policy.remote_embedding_model.is_some()
    {
        // 授权也要"配置齐全才记时间戳"，避免出现「授权了一个空配置」的假状态
        policy.vector_authorized_at = Some(now_iso());
    }
    if let Some(provider) = &input.decision_provider {
        // 只认 `rule` / `system_one`：写错一律退回规则档（不猜）
        let next = match provider.trim().to_ascii_lowercase().as_str() {
            "system_one" => Some("system_one".to_string()),
            _ => None,
        };
        if policy.decision_provider != next {
            // 换档位 = 换了「材料发给谁」：旧授权作废
            policy.decision_authorized_at = None;
        }
        policy.decision_provider = next;
    }
    if let Some(base) = &input.decision_base_url {
        let trimmed = base.trim();
        let next = if trimmed.is_empty() {
            None
        } else {
            normalize_remote_base_url(trimmed)
        };
        if policy.decision_base_url != next {
            policy.decision_authorized_at = None;
        }
        policy.decision_base_url = next;
    }
    if let Some(model) = &input.decision_model {
        let trimmed = model.trim();
        let next = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        if policy.decision_model != next {
            policy.decision_authorized_at = None;
        }
        policy.decision_model = next;
    }
    if input.confirm_decision == Some(true) && policy.decision_provider.as_deref() == Some("system_one") {
        // 端点与模型名都能解析出来才允许记时间戳（同上：不制造"授权了空配置"）
        let base = policy
            .decision_base_url
            .clone()
            .unwrap_or_else(|| SYSTEM_ONE_DEFAULT_BASE.to_string());
        if normalize_remote_base_url(&base).is_some() {
            policy.decision_authorized_at = Some(now_iso());
        }
    }
    if let Some(proxy) = &input.outbound_proxy {
        let trimmed = proxy.trim();
        policy.outbound_proxy = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    if let Some(budget) = &input.budget {
        policy.budget = budget.clamped();
    }
}

/// 本地模型地址规范化：只接受 http(s) 且去掉结尾的 `/`；不合法视为「未配置」。
///
/// 只允许本机地址（localhost / 127.0.0.1 / ::1）：本地模型不应该是一个远程服务；
/// 远程模型属于另一档决策（需要单独授权），本版不做。
pub fn normalize_local_endpoint(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let lowered = trimmed.to_ascii_lowercase();
    if !(lowered.starts_with("http://") || lowered.starts_with("https://")) {
        return None;
    }
    // 主机名解析与闸门/分级共用一份实现（IPv6 字面量会去掉方括号）
    let host = host_of(trimmed)?;
    let local = matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1");
    if !local {
        return None;
    }
    Some(trimmed.to_string())
}

/// 检索源地址规范化：只接受 http(s) 且必须含 `{query}`；不合法一律视为「未配置」。
pub fn normalize_search_endpoint(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lowered = trimmed.to_ascii_lowercase();
    if !(lowered.starts_with("http://") || lowered.starts_with("https://")) {
        return None;
    }
    if !trimmed.contains("{query}") {
        return None;
    }
    Some(trimmed.to_string())
}

/// 在线模型基地址规范化（OpenAI 兼容端点，例如 `https://api.example.com/v1`）。
///
/// 接受：
/// - `https://` 的公网域名（含子域、可选端口与路径）；
/// - 本机地址放宽到 `http://`（`localhost` / `127.0.0.1` / `::1`）——本地起的兼容网关（LM Studio 等）。
///
/// 拒绝（每一条都对应一种真实的外发风险）：
/// - 非 http(s) 协议（`file://` 会读本机文件、`ftp://` 不是我们支持的形状）；
/// - **裸 IP 直连**（`https://10.0.0.5/v1`、`http://127.0.0.1:8080`）：配置里写 IP 就等于
///   把「笔记片段发给谁」交给一个容易写错、也难以在界面上复核的地址；要用本机请写 `localhost`；
/// - **数字写法的 IP**（`https://2130706433/v1`、`https://0x7f000001/v1`、`https://foo.2130706433/v1`）：
///   十进制 / 十六进制写法的回环地址同样指向本机或内网，而这些形状在界面上完全看不出问题；
/// - **单标签主机名**（`https://intranet-api/v1`）：公网不可解析，实际只会指向内网；
/// - 带 userinfo 的地址（`https://user:pass@host`）：口令会随调试输出、错误信息泄漏；
/// - 非 https 的远程主机：默认加密，避免中间人读到笔记片段。
pub fn normalize_remote_base_url(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let lowered = trimmed.to_ascii_lowercase();
    let scheme = if lowered.starts_with("https://") {
        "https"
    } else if lowered.starts_with("http://") {
        "http"
    } else {
        return None;
    };
    // 先看 authority 段：`user:pass@host` 这类带凭据的地址一律不要
    let rest = &lowered[scheme.len() + 3..];
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    if rest[..authority_end].contains('@') {
        return None;
    }
    // 主机名解析与闸门/分级共用一份实现（IPv6 字面量会去掉方括号）
    let host = host_of(trimmed)?;
    let labels: Vec<&str> = host.split('.').collect();
    let loopback = host == "localhost";
    let numeric_only = host.chars().all(|ch| ch.is_ascii_digit() || ch == '.');
    let numeric_tld = labels
        .last()
        .map(|label| label.chars().all(|ch| ch.is_ascii_digit()))
        .unwrap_or(false);
    if host.parse::<std::net::IpAddr>().is_ok() || numeric_only || numeric_tld {
        // 裸 IP（点分 / IPv6 字面量）与各种数字写法一律拒绝：本机地址请写 localhost
        return None;
    }
    if labels.len() < 2 && !loopback {
        // 单标签主机名在公网上解析不了，实际只会落到内网（或某个编码写法）
        return None;
    }
    if scheme == "http" && !loopback {
        return None;
    }
    Some(trimmed.to_string())
}

/// 域名列表规范化：去空白、小写、去重、去掉协议与路径。
fn normalize_domains(list: &[String]) -> Vec<String> {
    let mut seen = Vec::new();
    for item in list {
        let host = host_of(item).unwrap_or_else(|| item.trim().to_string());
        let host = host.trim().trim_start_matches("www.").to_ascii_lowercase();
        if !host.is_empty() && !seen.contains(&host) {
            seen.push(host);
        }
    }
    seen
}

/// 域名匹配：支持精确与子域（规则 `example.com` 命中 `a.example.com`）。
fn domain_matches(rule: &str, host: &str) -> bool {
    let rule = rule.trim().trim_start_matches("www.").to_ascii_lowercase();
    if rule.is_empty() {
        return false;
    }
    host == rule || host.ends_with(&format!(".{rule}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ai::ResearchBudget;

    #[test]
    fn policy_defaults_are_conservative() {
        let policy = AiPolicy::default();
        assert!(!policy.enabled, "AI 总开关默认关闭");
        assert_eq!(policy.network, NetworkMode::LocalOnly, "联网档位默认仅本地");
        assert!(!policy.allow_fetch_body, "抓取正文默认关闭");
        assert!(policy.domain_allowlist.is_empty(), "默认不预置白名单");
        assert_eq!(policy.budget.max_pages, 12);
    }

    #[test]
    fn budget_is_clamped_to_hard_limits() {
        let loose = ResearchBudget {
            max_subquestions: 99,
            max_pages: 999,
            max_duration_secs: 99_999,
        };
        let clamped = loose.clamped();
        assert_eq!(clamped.max_subquestions, 10);
        assert_eq!(clamped.max_pages, 50);
        assert_eq!(clamped.max_duration_secs, 1800);
    }

    #[test]
    fn switching_network_mode_clears_previous_confirmation() {
        let mut policy = AiPolicy::default();
        apply_policy(
            &mut policy,
            &UpdateAiPolicyInput {
                enabled: Some(true),
                network: Some("allowNetwork".into()),
                confirm_network: Some(true),
                ..Default::default()
            },
        );
        assert_eq!(policy.network, NetworkMode::AllowNetwork);
        assert!(policy.network_confirmed_at.is_some());

        // 切回仅本地：确认状态必须清空（下次切到联网要重新确认）
        apply_policy(
            &mut policy,
            &UpdateAiPolicyInput { network: Some("localOnly".into()), ..Default::default() },
        );
        assert!(policy.network_confirmed_at.is_none());
    }

    /// 本地模型是**唯一的外发通道之一**，地址校验必须真的只放行本机：
    /// 这条规则以前没有任何测试，而且 `::1` 分支因为解析方式写错而永远不会命中。
    #[test]
    fn local_model_endpoint_must_be_a_loopback_host() {
        for accepted in [
            "http://localhost:11434",
            "http://127.0.0.1:11434",
            "http://[::1]:11434",
            "http://LOCALHOST:11434/",
            "https://localhost",
            "http://localhost:11434/api",
        ] {
            assert!(
                normalize_local_endpoint(accepted).is_some(),
                "应当接受本机地址：{accepted}"
            );
        }
        for rejected in [
            "",
            "   ",
            "localhost:11434",
            "ftp://localhost",
            "http://evil.com:11434",
            "http://localhost.evil.com:11434",
            "http://127.0.0.1.evil.com:11434",
            "http://evil.com#@localhost",
            "http://user@evil.com:11434",
            // 十进制写法指向 127.0.0.1，但不是我们声明的本机写法：一律拒绝（保守）
            "http://2130706433:11434",
            "http://0.0.0.0:11434",
        ] {
            assert!(
                normalize_local_endpoint(rejected).is_none(),
                "必须拒绝非本机地址：{rejected}"
            );
        }
    }

    /// IPv6 字面量在「域名规则」与「主机解析」两侧的口径要一致，否则黑名单写了也拦不住。
    #[test]
    fn ipv6_literals_normalize_the_same_way_in_rules_and_hosts() {
        assert_eq!(normalize_domains(&["[::1]".to_string()]), vec!["::1".to_string()]);
        assert_eq!(
            normalize_domains(&["https://[2001:db8::1]/x".to_string()]),
            vec!["2001:db8::1".to_string()]
        );
        assert!(domain_matches("::1", "::1"));
        assert!(domain_matches("2001:db8::1", "2001:db8::1"));
    }

    #[test]
    fn domain_rules_match_subdomains_and_normalize_input() {
        assert!(domain_matches("example.com", "example.com"));
        assert!(domain_matches("example.com", "docs.example.com"));
        assert!(!domain_matches("example.com", "notexample.com"));
        let normalized = normalize_domains(&[
            " https://WWW.Example.com/path ".into(),
            "example.com".into(),
            "cdn.test.io:443".into(),
        ]);
        assert_eq!(
            normalized,
            vec!["example.com".to_string(), "cdn.test.io".to_string()]
        );
        assert_eq!(
            host_of("https://docs.example.com/a/b?c=1").as_deref(),
            Some("docs.example.com")
        );
    }

    /// 在线模型的基地址是**第二个外发通道**，放行清单必须真的只放行「https 的域名」。
    ///
    /// 之前本地模型那条测试的教训：只断言「是/不是 IP」这种弱条件，写错了也看不出来；
    /// 这里逐条列出接受与拒绝的具体地址。
    #[test]
    fn remote_base_url_accepts_https_domains_and_rejects_everything_risky() {
        for accepted in [
            "https://api.openai.com/v1",
            "https://api.deepseek.com/v1/",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "https://proxy.corp.example.com:8443/v1",
            "HTTPS://API.Example.COM/v1",
            // 本机起的兼容网关（LM Studio / vLLM）：http 只在本机这一档放宽
            "http://localhost:1234/v1",
            "https://localhost:1234/v1",
        ] {
            assert!(
                normalize_remote_base_url(accepted).is_some(),
                "应当接受：{accepted}"
            );
        }
        for rejected in [
            "",
            "   ",
            "api.openai.com/v1",
            "ftp://api.example.com/v1",
            "file:///etc/passwd",
            // 裸 IP 一律拒绝（含回环：要用本机写 localhost）
            "https://10.0.0.5/v1",
            "https://192.168.1.20:8080/v1",
            "http://127.0.0.1:11434/v1",
            "http://[::1]:1234/v1",
            // 十进制 / 十六进制写法与单标签主机名：都是「看起来像域名、其实指向内网」的形状
            "https://2130706433/v1",
            "https://foo.2130706433/v1",
            "https://0x7f000001/v1",
            "https://intranet-api/v1",
            // 非 https 的远程主机：默认必须加密
            "http://api.example.com/v1",
            // 带 userinfo：口令会随错误信息与调试输出泄漏
            "https://user:pass@api.example.com/v1",
            "https://token@api.example.com/v1",
        ] {
            assert!(
                normalize_remote_base_url(rejected).is_none(),
                "必须拒绝：{rejected}"
            );
        }
        // 规范化：尾部斜杠去掉，其余原样保留（大小写不影响调用）
        assert_eq!(
            normalize_remote_base_url(" https://api.example.com/v1/// ").as_deref(),
            Some("https://api.example.com/v1")
        );
    }

    /// 换了地址或模型名 = 换了收件人：旧授权必须立刻作废，否则「换个地址继续外发」就是静默的。
    #[test]
    fn changing_remote_endpoint_or_model_revokes_the_authorization() {
        let mut policy = AiPolicy::default();
        apply_policy(
            &mut policy,
            &UpdateAiPolicyInput {
                remote_model_base_url: Some("https://api.example.com/v1".into()),
                remote_model_name: Some("chat-model".into()),
                confirm_remote_model: Some(true),
                ..Default::default()
            },
        );
        assert!(policy.remote_model_authorized_at.is_some(), "配置完整 + 确认才记时间戳");
        let authorized_at = policy.remote_model_authorized_at.clone();

        // 只是重复保存同一个地址/模型名：授权不受影响（否则每次保存都要重新确认）
        apply_policy(
            &mut policy,
            &UpdateAiPolicyInput {
                remote_model_base_url: Some("https://api.example.com/v1".into()),
                remote_model_name: Some("chat-model".into()),
                ..Default::default()
            },
        );
        assert_eq!(policy.remote_model_authorized_at, authorized_at);

        // 换地址 → 作废
        apply_policy(
            &mut policy,
            &UpdateAiPolicyInput {
                remote_model_base_url: Some("https://other.example.com/v1".into()),
                ..Default::default()
            },
        );
        assert!(
            policy.remote_model_authorized_at.is_none(),
            "换地址后必须重新确认"
        );

        // 重新授权后，换模型名同样作废
        apply_policy(&mut policy, &UpdateAiPolicyInput { confirm_remote_model: Some(true), ..Default::default() });
        assert!(policy.remote_model_authorized_at.is_some());
        apply_policy(
            &mut policy,
            &UpdateAiPolicyInput { remote_model_name: Some("another-model".into()), ..Default::default() },
        );
        assert!(policy.remote_model_authorized_at.is_none(), "换模型名后必须重新确认");

        // 非法地址写进去只会变成「未配置」，不会留下半截状态
        apply_policy(
            &mut policy,
            &UpdateAiPolicyInput { remote_model_base_url: Some("http://evil.example.com/v1".into()), ..Default::default() },
        );
        assert!(policy.remote_model_base_url.is_none(), "非 https 的远程地址不得被保存");
        assert!(policy.remote_model().is_none());
    }

    /// 确认一个**空配置**不能留下「已授权」的假状态（否则界面会显示已授权但实际没配好）。
    #[test]
    fn confirming_an_incomplete_remote_configuration_does_not_authorize() {
        let mut policy = AiPolicy::default();
        apply_policy(
            &mut policy,
            &UpdateAiPolicyInput { confirm_remote_model: Some(true), ..Default::default() },
        );
        assert!(policy.remote_model_authorized_at.is_none());

        // 只有地址、没有模型名 → 仍不算配置完整
        apply_policy(
            &mut policy,
            &UpdateAiPolicyInput {
                remote_model_base_url: Some("https://api.example.com/v1".into()),
                confirm_remote_model: Some(true),
                ..Default::default()
            },
        );
        assert!(policy.remote_model_authorized_at.is_none(), "缺模型名就不算配置完整");
        assert!(policy.remote_model().is_none());
    }
}
