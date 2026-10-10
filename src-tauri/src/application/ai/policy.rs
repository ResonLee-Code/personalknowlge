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

use super::super::context::WorkspaceContext;

/// 审计文件名（放在既有 `.knowledge/logs/` 下）。
const AUDIT_FILE: &str = "ai-audit.jsonl";

pub struct AiPolicyService {
    ctx: Arc<WorkspaceContext>,
}

impl AiPolicyService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx }
    }

    /// 当前策略。
    pub fn policy(&self) -> AiPolicy {
        self.ctx.settings().ai
    }

    /// 更新策略；`confirmNetwork = true` 时记录确认时间（用于「首次使用再确认」）。
    pub fn update(&self, input: UpdateAiPolicyInput) -> Result<AiPolicy> {
        let updated = self.ctx.update_settings(|settings| {
            apply_policy(&mut settings.ai, &input);
        })?;
        Ok(updated.ai)
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
    let rest = trimmed.split_once("://").map(|(_, rest)| rest).unwrap_or(trimmed);
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    let local = host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]";
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

/// 从 URL 或裸域名里取 host（小写、去 www、去端口）。
fn host_of(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let rest = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim()
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
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
}
