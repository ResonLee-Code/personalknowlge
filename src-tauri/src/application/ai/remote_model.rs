//! 在线模型接入（OpenAI 兼容端点；V1.2.1 阶段 D5 补充档）。
//!
//! 为什么要有这一档：**应用是本地软件、跨平台部署，但装它的机器不一定跑得动本地模型**
//! （没有独显、内存不够、公司机器不让装 Ollama）。三档可选的降级链由此成立：
//! **模板（永远可用）→ 本地模型（装了就用）→ 在线模型（用户显式配置并授权后才外发）**。
//!
//! 纪律（与本地模型同一套，另加两条）：
//! - **必须显式授权**：`policy::ensure_remote_model_allowed` 每次拒绝都记审计；换地址/模型名后
//!   授权作废。调用方（`research.rs`）在授权不过时**根本不构造**这个 provider；
//! - **Key 只在请求头里出现**：错误信息只带主机名与状态码，绝不回显 Key（审计、日志、报告、
//!   IPC 响应里都不会有 Key）；Key 存在应用数据目录，不进工作区、不进备份；
//! - **`allow_remote` 二次把关**：请求没有标记「允许外发」时直接拒绝，防止将来某条新调用路径
//!   忘了过闸门就把笔记片段发出去；
//! - **失败即降级**：任何错误都只让上层回退到下一档（本地模型 / 规则档 / 模板），研究流程不中断。

use std::io::{BufRead, BufReader};
use std::time::Duration;

use crate::core::error::{AppError, ErrorCode, Result};

use super::host::host_of;
use super::local_model::{
    build_conflict_prompt, build_decision_prompt, build_generation_prompt, parse_conflict_output,
    parse_decision_output,
};
use super::traits::{
    ConflictVerdict, DecisionCandidate, DecisionKind, DecisionModel, DecisionResult, DecisionState,
    GenerationRequest, GenerationResponse, GenerativeModel,
};

/// 单次调用超时（云端首 token 通常很快，给 60 秒足够）。
const TIMEOUT: Duration = Duration::from_secs(60);
/// 生成结果的字符上限（与本地模型同口径，防止超长输出整篇写进报告）。
const MAX_OUTPUT_CHARS: usize = 6000;
/// 流式解析时另存的原始响应上限（用于「对方忽略了 `stream:true`」时的兜底解析）。
const MAX_RAW_CHARS: usize = 1_000_000;

/// 从**非流式**响应里取 `choices[0].message.content`。
///
/// 存在的原因：有些"OpenAI 兼容"端点会忽略 `stream: true`，直接回一个普通 JSON。
/// 这种时候只是少了个逐字效果，**不该把一个本来能用的在线模型判成失败**。
fn content_from_non_streaming(raw: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
    let content = value
        .get("choices")?
        .as_array()?
        .first()?
        .get("message")?
        .get("content")?
        .as_str()?
        .trim()
        .to_string();
    (!content.is_empty()).then_some(content)
}

/// OpenAI 兼容端点客户端（生成 + 打分共用同一份连接配置）。
#[derive(Debug, Clone)]
pub struct RemoteModel {
    base_url: String,
    model: String,
    api_key: String,
    /// 出站代理（为空 = 跟随环境变量 / 系统代理；见 `core::net`）
    proxy: Option<String>,
}

impl RemoteModel {
    pub fn new(base_url: &str, model: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim().trim_end_matches('/').to_string(),
            model: model.trim().to_string(),
            api_key: api_key.trim().to_string(),
            proxy: None,
        }
    }

    /// 指定出站代理（设置里显式配置的那一个）。
    pub fn with_proxy(mut self, proxy: Option<String>) -> Self {
        self.proxy = proxy.filter(|value| !value.trim().is_empty());
        self
    }

    /// 完整标识（报告与审计里用；含模型名，便于事后核对到底是哪个模型写的）。
    pub fn id(&self) -> String {
        Self::provider_id(&self.model)
    }

    /// provider 标识（含模型名）。自检等场景**不持有 Key** 也能用它，
    /// 保证「自检说的」与「报告 composer 写的」是同一个字符串。
    pub fn provider_id(model: &str) -> String {
        format!("remote.openai-compatible:{}", model.trim())
    }

    /// 只用于错误信息的主机名：**永远不要把 Key 或完整 URL 写进错误**。
    fn host_label(&self) -> String {
        host_of(&self.base_url).unwrap_or_else(|| "（未知主机）".to_string())
    }

    /// 一次非流式调用（`POST {base}/chat/completions`）。
    fn chat(&self, prompt: &str, expect_json: bool) -> Result<String> {
        let agent = crate::core::net::agent_for(&self.base_url, TIMEOUT, self.proxy.as_deref());
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": prompt }],
            "stream": false,
        });
        if expect_json {
            // OpenAI 兼容端点普遍支持这个字段；不支持的服务会忽略它，
            // 此时 `parse_decision_output` 仍会严格校验输出（不合法就回退，不猜）。
            body["response_format"] = serde_json::json!({ "type": "json_object" });
        }
        let response = agent
            .post(format!("{}/chat/completions", self.base_url))
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send(body.to_string())
            .map_err(|error| {
                // `ureq` 的错误里可能带 URL，但不带请求头 —— 这里只再补主机名，方便定位
                AppError::new(
                    ErrorCode::InvalidInput,
                    format!(
                        "在线模型调用失败：{error}（主机 {}，模型 `{}`）",
                        self.host_label(),
                        self.model
                    ),
                )
            })?;
        let status = response.status().as_u16();
        let text = response.into_body().read_to_string().map_err(|error| {
            AppError::new(
                ErrorCode::InvalidInput,
                format!("读取在线模型响应失败（主机 {}）：{error}", self.host_label()),
            )
        })?;
        if status >= 400 {
            // 只报状态码与主机：响应体里可能有服务商的提示，但**不能**把请求头带出去
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!(
                    "在线模型返回 HTTP {status}（主机 {}，模型 `{}`）：检查 API Key、模型名与账户额度",
                    self.host_label(),
                    self.model
                ),
            ));
        }
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
            AppError::new(
                ErrorCode::InvalidInput,
                format!("在线模型响应不是合法 JSON（主机 {}）：{error}", self.host_label()),
            )
        })?;
        let content = value
            .get("choices")
            .and_then(|entry| entry.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|message| message.get("content"))
            .and_then(|entry| entry.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if content.is_empty() {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!("在线模型返回了空内容（主机 {}）。", self.host_label()),
            ));
        }
        Ok(crate::core::text::truncate_chars(&content, MAX_OUTPUT_CHARS))
    }
}

impl GenerativeModel for RemoteModel {
    fn id(&self) -> &'static str {
        // trait 要求 `&'static str`；完整标识（含模型名）用 `RemoteModel::id()`
        "remote.openai-compatible"
    }

    fn is_local(&self) -> bool {
        false
    }

    fn supports_streaming(&self) -> bool {
        // Track C1：`chat/completions` 的 `stream: true`（SSE）已实现
        true
    }

    fn complete(&self, request: &GenerationRequest) -> Result<GenerationResponse> {
        if !request.allow_remote {
            // 二次把关：调用方没声明「这次允许外发」就不发（防的是将来新加的调用路径漏过闸门）
            return Err(AppError::new(
                ErrorCode::NetworkDisabled,
                "本次生成没有标记为「允许外发」，已拒绝调用在线模型。",
            ));
        }
        let started = std::time::Instant::now();
        let prompt = build_generation_prompt(request);
        let text = self.chat(&prompt, false)?;
        Ok(GenerationResponse {
            text,
            model: self.id(),
            duration_ms: started.elapsed().as_millis() as u64,
            usage: None,
        })
    }

    /// 流式生成（Track C1）：`stream: true` 的 SSE，逐块回调 `choices[0].delta.content`。
    ///
    /// 中途出错**不返回半截内容**：返回 `Err`，由上层清掉预览并降级到下一档。
    fn complete_streaming(
        &self,
        request: &GenerationRequest,
        on_delta: &mut dyn FnMut(&str),
    ) -> Result<GenerationResponse> {
        if !request.allow_remote {
            return Err(AppError::new(
                ErrorCode::NetworkDisabled,
                "本次生成没有标记为「允许外发」，已拒绝调用在线模型。",
            ));
        }
        let started = std::time::Instant::now();
        let prompt = build_generation_prompt(request);
        let agent = crate::core::net::agent_for(&self.base_url, TIMEOUT, self.proxy.as_deref());
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": prompt }],
            "stream": true,
        });
        let response = agent
            .post(format!("{}/chat/completions", self.base_url))
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send(body.to_string())
            .map_err(|error| {
                AppError::new(
                    ErrorCode::InvalidInput,
                    format!(
                        "在线模型调用失败：{error}（主机 {}，模型 `{}`）",
                        self.host_label(),
                        self.model
                    ),
                )
            })?;
        let status = response.status().as_u16();
        if status >= 400 {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!(
                    "在线模型返回 HTTP {status}（主机 {}，模型 `{}`）：检查 API Key、模型名与账户额度",
                    self.host_label(),
                    self.model
                ),
            ));
        }

        let mut reader = BufReader::new(response.into_body().into_reader());
        let mut accumulated = String::new();
        let mut raw = String::new();
        let mut saw_sse = false;
        let mut line = String::new();
        loop {
            line.clear();
            let read = reader.read_line(&mut line).map_err(|error| {
                AppError::new(
                    ErrorCode::InvalidInput,
                    format!("读取在线模型流式响应失败（主机 {}）：{error}", self.host_label()),
                )
            })?;
            if read == 0 {
                break;
            }
            if raw.len() < MAX_RAW_CHARS {
                raw.push_str(&line);
            }
            // SSE：只认 `data:` 行；`[DONE]` 与空行跳过
            let Some(data) = line.trim().strip_prefix("data:") else {
                continue;
            };
            let data = data.trim();
            if data.is_empty() || data == "[DONE]" {
                continue;
            }
            saw_sse = true;
            let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
                // 单行坏数据不该毁掉整段：跳过（与"严格校验最终产物"同一口径）
                continue;
            };
            let delta = value
                .get("choices")
                .and_then(|choices| choices.as_array())
                .and_then(|choices| choices.first())
                .and_then(|choice| choice.get("delta"))
                .and_then(|delta| delta.get("content"))
                .and_then(|content| content.as_str())
                .unwrap_or_default();
            if delta.is_empty() {
                continue;
            }
            accumulated.push_str(delta);
            on_delta(delta);
            if accumulated.chars().count() >= MAX_OUTPUT_CHARS {
                break;
            }
        }

        // 对方忽略了 `stream: true`（回的是普通 JSON）→ 按非流式再解析一次
        if accumulated.is_empty() && !saw_sse {
            if let Some(content) = content_from_non_streaming(&raw) {
                on_delta(&content);
                accumulated = content;
            }
        }

        let text = crate::core::text::truncate_chars(accumulated.trim(), MAX_OUTPUT_CHARS);
        if text.is_empty() {
            return Err(AppError::new(
                ErrorCode::InvalidInput,
                format!("在线模型返回了空内容（主机 {}）。", self.host_label()),
            ));
        }
        Ok(GenerationResponse {
            text,
            model: self.id(),
            duration_ms: started.elapsed().as_millis() as u64,
            usage: None,
        })
    }
}

impl DecisionModel for RemoteModel {
    fn id(&self) -> &'static str {
        "remote.openai-compatible"
    }

    fn is_local(&self) -> bool {
        false
    }

    fn decide(&self, kind: DecisionKind, state: &DecisionState) -> Result<Vec<DecisionResult>> {
        if state.candidates.is_empty() {
            return Ok(Vec::new());
        }
        let prompt = build_decision_prompt(kind, state);
        let raw = self.chat(&prompt, true)?;
        parse_decision_output(&raw, kind, state)
    }

    /// 语义冲突判定（Track C5）：在线模型直接回答"是否互相矛盾"。
    ///
    /// 这一档只有在「已授权」且「逐次确认通过」时才会被构造出来，
    /// 所以本身不需要再判一次外发闸门（与 `decide` 同口径）。
    fn judge_conflict(
        &self,
        query: &str,
        candidates: &[DecisionCandidate],
    ) -> Result<Option<ConflictVerdict>> {
        if candidates.len() < 2 {
            return Ok(None);
        }
        let prompt = build_conflict_prompt(query, candidates);
        let raw = self.chat(&prompt, true)?;
        Ok(Some(parse_conflict_output(&raw)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::traits::{DecisionCandidate, RetrievedChunk, RetrievalOrigin};

    fn request(allow_remote: bool) -> GenerationRequest {
        GenerationRequest {
            prompt: "写推测段".to_string(),
            evidence: vec![RetrievedChunk {
                note_id: "note_a".to_string(),
                chunk_index: 0,
                heading_path: vec!["调度".to_string()],
                snippet: "工作窃取".to_string(),
                char_start: 0,
                char_end: 4,
                score: 1.0,
                origin: RetrievalOrigin::Bm25,
            }],
            max_tokens: None,
            allow_remote,
        }
    }

    #[test]
    fn model_id_is_stable_and_not_local() {
        let model = RemoteModel::new("https://api.example.com/v1/", "chat-model", "sk-x");
        assert_eq!(model.id(), "remote.openai-compatible:chat-model");
        assert_eq!(<RemoteModel as GenerativeModel>::id(&model), "remote.openai-compatible");
        assert_eq!(<RemoteModel as DecisionModel>::id(&model), "remote.openai-compatible");
        assert!(!<RemoteModel as GenerativeModel>::is_local(&model), "在线模型绝不是本地模型");
        assert!(!<RemoteModel as DecisionModel>::is_local(&model));
    }

    /// 没标记「允许外发」时，一个请求都不能发出去（这里连网络都不用碰）。
    #[test]
    fn complete_refuses_when_remote_is_not_allowed() {
        let model = RemoteModel::new("https://api.example.com/v1", "chat-model", "sk-x");
        let error = model.complete(&request(false)).expect_err("必须拒绝");
        assert!(error.message.contains("允许外发"), "错误要说清楚原因：{}", error.message);
    }

    /// 错误信息里绝不能出现 Key —— 它会进界面、进日志、进问题反馈。
    #[test]
    fn error_messages_never_leak_the_api_key() {
        // 指向一个必然连不上的端口，构造真实的连接错误
        let model = RemoteModel::new("http://localhost:9/v1", "chat-model", "sk-secret-value");
        let error = model.complete(&request(true)).expect_err("连不上应报错");
        assert!(
            !error.message.contains("sk-secret-value"),
            "错误信息不得包含 API Key：{}",
            error.message
        );
        assert!(error.message.contains("localhost"), "错误信息要能定位主机：{}", error.message);

        let state = DecisionState {
            query: "课题".to_string(),
            candidates: vec![DecisionCandidate {
                note_id: "note_a".to_string(),
                title: "标题".to_string(),
                snippet: "片段".to_string(),
                updated_at: "2026-10-11T10:00:00+08:00".to_string(),
                source_reliability: Some(4),
                valid_until: None,
            }],
        };
        let error = model.decide(DecisionKind::Relevance, &state).expect_err("连不上应报错");
        assert!(!error.message.contains("sk-secret-value"));
    }
}
