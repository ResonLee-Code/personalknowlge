//! 决策模型客户端（D4）：TypeSafe **Jev** 与 Perplexity **Decisions** 用的是**同一套协议**
//! （`POST {base}/v1/decisions` 或 `/v1/systemone`，请求 `{model, state, questions}`，
//! 响应 `{model, answers, usage}`，答案类型 `noul` / `choice` / `score`），
//! 所以这里**只写一个实现**，靠 `base_url` + `model` 切换。
//!
//! 纪律（与在线模型/嵌入同一套）：
//! - **Key 只出现在请求头**，任何错误信息只带主机名与状态码；
//! - 端点不合法、未授权时**根本不构造**这个 Provider（调用方用 `decision_config()` 把关）；
//! - 出错一律返回 `Err`，由决策链降级到规则档，绝不让研究流程卡住。
//!
//! 端点差异只有一处：路径。Perplexity 是 `/v1/decisions`，TypeSafe 是 `/v1/systemone`，
//! 按域名判断（与 `default_decision_model` 同一套判据）。

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::application::ai::policy::DecisionConfig;
use crate::application::ai::traits::{
    ConflictVerdict, DecisionCandidate, DecisionChoice, DecisionKind, DecisionModel, DecisionResult,
    DecisionState,
};
use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::net::agent_for;

/// 单次请求的超时。文档实测：几百 token <2s，接近 26 万 token 约 23s；这里给足。
const TIMEOUT: Duration = Duration::from_secs(60);
/// 单请求最多几个问题（文档上限 128；留一点余量）。
const MAX_QUESTIONS: usize = 120;
/// 相关度打分的档位数（`score` 会返回档位索引的加权平均，0..=2）。
const RELEVANCE_LEVELS: [&str; 3] = ["不相关", "与问题无关或只是字面重合", "直接相关，能回答该问题"];
/// 送进 state 的片段上限（控制 token 与隐私面）。
const MAX_SNIPPET_CHARS: usize = 800;

pub struct SystemOneDecisionModel {
    base_url: String,
    model: String,
    api_key: String,
    proxy: Option<String>,
}

impl SystemOneDecisionModel {
    pub fn new(config: &DecisionConfig) -> Self {
        Self {
            base_url: config.base_url.trim().trim_end_matches('/').to_string(),
            model: config.model.trim().to_string(),
            api_key: config.key.trim().to_string(),
            proxy: config.proxy.clone(),
        }
    }

    /// provider 标识（进 `DecisionResult`、审计与报告）。
    pub fn provider_id(&self) -> String {
        format!("system_one:{}", self.model)
    }

    /// 端点路径：Perplexity 用 `/v1/decisions`，TypeSafe 用 `/v1/systemone`。
    pub fn path_for(base_url: &str) -> &'static str {
        if base_url.to_ascii_lowercase().contains("typesafe") {
            "/v1/systemone"
        } else {
            "/v1/decisions"
        }
    }

    fn path(&self) -> &'static str {
        Self::path_for(&self.base_url)
    }

    /// 只用于错误信息的主机名（**绝不回显 Key**）。
    fn host_label(&self) -> String {
        super::host::host_of(&self.base_url).unwrap_or_else(|| "（未知主机）".to_string())
    }

    /// 原始请求（给契约探针用）：返回解析后的 JSON 与输入 token 用量。
    ///
    /// 探针要看的字段与决策路径不完全一样（它只关心 `noul`），所以这里暴露"传输层"，
    /// 由探针自己做语义解析——避免把探针的临时需求塞进决策实现里。
    pub fn raw_request(
        &self,
        state: &serde_json::Value,
        questions: &serde_json::Value,
    ) -> Result<(serde_json::Value, i64)> {
        let response = self.request_raw(state, questions)?;
        Ok(response)
    }

    fn request(&self, state: &serde_json::Value, questions: &serde_json::Value) -> Result<Response> {
        let (value, _tokens) = self.request_raw(state, questions)?;
        serde_json::from_value::<Response>(value).map_err(|error| {
            AppError::new(
                ErrorCode::InvalidInput,
                format!("决策模型响应结构不符（主机 {}）：{error}", self.host_label()),
            )
        })
    }

    /// 发一次请求，返回原始 JSON 与输入 token 数。
    fn request_raw(
        &self,
        state: &serde_json::Value,
        questions: &serde_json::Value,
    ) -> Result<(serde_json::Value, i64)> {
        let body = serde_json::json!({
            "model": self.model,
            "state": state,
            "questions": questions,
        });
        let agent = agent_for(&self.base_url, TIMEOUT, self.proxy.as_deref());
        let response = agent
            .post(format!("{}{}", self.base_url, self.path()))
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send(body.to_string())
            .map_err(|error| {
                AppError::new(
                    ErrorCode::InvalidInput,
                    format!(
                        "决策模型调用失败：{error}（主机 {}，模型 `{}`）——若是网络问题，可到「设置 → AI → 出站代理」里指定代理",
                        self.host_label(),
                        self.model
                    ),
                )
            })?;

        let status = response.status().as_u16();
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let text = response.into_body().read_to_string().unwrap_or_default();

        if status >= 400 {
            return Err(self.status_error(status, &text, retry_after.as_deref(), request_id.as_deref()));
        }
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
            AppError::new(
                ErrorCode::InvalidInput,
                format!(
                    "决策模型响应无法解析（主机 {}）：{error}{}",
                    self.host_label(),
                    request_id
                        .map(|id| format!("，request-id {id}"))
                        .unwrap_or_default()
                ),
            )
        })?;
        let input_tokens = value
            .get("usage")
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        Ok((value, input_tokens))
    }

    /// 状态码 → 人能看懂的结论（**把文档里列的坑逐条说清**）。
    fn status_error(
        &self,
        status: u16,
        body: &str,
        retry_after: Option<&str>,
        request_id: Option<&str>,
    ) -> AppError {
        // 文档：多数错误体是 `{"error":{"message","type","code"}}`；404/405 空体；504 可能是 HTML
        let message = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(|message| message.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| crate::core::text::truncate_chars(body.trim(), 200));
        let hint = match status {
            400 => "请求不合法（模型名/字段不符，或超出上限）",
            401 => "API Key 缺失或无效（注意：Key 必须放在 Authorization: Bearer 里）",
            404 => "路径不对（端点不能带尾斜杠）",
            405 => "方法不对（必须是 POST）",
            413 => "请求体超过 32 MiB",
            429 => "触发限流（Decisions API 为 10 请求/秒）",
            500..=599 => "服务端未按时作答或故障（大输入可能需要更久，可减小批量）",
            _ => "未知错误",
        };
        let mut detail = format!(
            "决策模型返回 HTTP {status}（主机 {}，模型 `{}`）：{hint}",
            self.host_label(),
            self.model
        );
        if !message.is_empty() {
            detail.push_str(&format!("；服务端说明：{message}"));
        }
        if let Some(seconds) = retry_after {
            detail.push_str(&format!("；建议 {seconds} 秒后重试"));
        }
        if let Some(id) = request_id {
            detail.push_str(&format!("；request-id {id}"));
        }
        AppError::new(ErrorCode::InvalidInput, detail)
    }

    /// 把 `score` 答案（档位加权平均）归一化到 0..1，并映射成我们的选择。
    fn score_to_result(&self, id: &str, answer: &Answer, kind: DecisionKind) -> Option<DecisionResult> {
        let Answer::Score { score, confidence, .. } = answer else {
            return None;
        };
        let normalized = (score / (RELEVANCE_LEVELS.len() as f32 - 1.0)).clamp(0.0, 1.0);
        let choice = if normalized >= 0.66 {
            DecisionChoice::Use
        } else if normalized >= 0.33 {
            DecisionChoice::Keep
        } else {
            DecisionChoice::Drop
        };
        Some(DecisionResult {
            note_id: id.to_string(),
            choice,
            score: normalized,
            confidence: *confidence,
            // 决策模型不返回理由：这里如实标注这是**我们**按档位折算出来的，别冒充模型的话
            reason: format!(
                "{} 档位加权分 {score:.3}/{max:.0}（归一化 {normalized:.3}，confidence {confidence:.3}）",
                self.provider_id(),
                max = RELEVANCE_LEVELS.len() as f32 - 1.0
            ),
            kind,
        })
    }

    fn candidate_state(&self, state: &DecisionState) -> serde_json::Value {
        serde_json::json!({
            "query": state.query,
            "candidates": state.candidates.iter().map(|candidate| serde_json::json!({
                "id": candidate.note_id,
                "title": candidate.title,
                "snippet": crate::core::text::truncate_chars(&candidate.snippet, MAX_SNIPPET_CHARS),
                "updated_at": candidate.updated_at,
                "source_reliability": candidate.source_reliability,
            })).collect::<Vec<_>>(),
        })
    }

    /// 问题的 `criteria` 是**有序数组**（档位说明），索引即分数。
    fn relevance_questions(&self, candidates: &[DecisionCandidate]) -> serde_json::Value {
        let mut questions = serde_json::Map::new();
        for candidate in candidates {
            questions.insert(
                candidate.note_id.clone(),
                serde_json::json!({
                    "type": "score",
                    "instructions": format!(
                        "候选 `{}` 与查询的相关程度（只看它能不能帮助回答查询，不看语气）",
                        candidate.note_id
                    ),
                    "criteria": RELEVANCE_LEVELS,
                }),
            );
        }
        serde_json::Value::Object(questions)
    }
}

impl DecisionModel for SystemOneDecisionModel {
    fn id(&self) -> &'static str {
        "system_one"
    }

    fn is_local(&self) -> bool {
        false
    }

    fn decide(&self, kind: DecisionKind, state: &DecisionState) -> Result<Vec<DecisionResult>> {
        if state.candidates.is_empty() {
            return Ok(Vec::new());
        }
        let candidate_state = self.candidate_state(state);
        let mut results = Vec::with_capacity(state.candidates.len());
        // 文档上限 128 个问题/请求；超出就分批（每批都带上同一份 state）
        for batch in state.candidates.chunks(MAX_QUESTIONS) {
            let questions = self.relevance_questions(batch);
            let response = self.request(&candidate_state, &questions)?;
            for candidate in batch {
                let Some(answer) = response.answers.get(&candidate.note_id) else {
                    continue; // 少答某一条：按"没结果"处理，由上层用规则档兜
                };
                if let Some(result) = self.score_to_result(&candidate.note_id, answer, kind) {
                    results.push(result);
                }
            }
        }
        Ok(results)
    }

    /// 语义冲突判定：一个 `noul` 问题（"这些材料是否互相矛盾"）。
    fn judge_conflict(
        &self,
        query: &str,
        candidates: &[DecisionCandidate],
    ) -> Result<Option<ConflictVerdict>> {
        if candidates.len() < 2 {
            return Ok(None); // 一条材料谈不上"互相矛盾"
        }
        let state = serde_json::json!({
            "query": query,
            "materials": candidates.iter().map(|candidate| serde_json::json!({
                "id": candidate.note_id,
                "title": candidate.title,
                "snippet": crate::core::text::truncate_chars(&candidate.snippet, MAX_SNIPPET_CHARS),
            })).collect::<Vec<_>>(),
        });
        let questions = serde_json::json!({
            "conflict": {
                "type": "noul",
                "instructions": "这些材料之间是否**互相矛盾**（对同一问题给出对立结论）？只是侧重不同、详略不同、时间不同，都不算矛盾。",
                "criteria": {
                    "true": "结论对立（一个支持、一个明确反对，或给出互斥的数值/事实）",
                    "false": "只是侧重不同，或互不相关"
                }
            }
        });
        let response = self.request(&state, &questions)?;
        let Some(Answer::Noul { noul }) = response.answers.get("conflict") else {
            return Ok(None);
        };
        Ok(Some(ConflictVerdict {
            conflict: *noul >= 0.5,
            // 决策模型不给理由：如实标注这是模型自己给的 yes 概率
            reason: format!("{} 给出 P(矛盾)={noul:.3}", self.provider_id()),
        }))
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Answer {
    Noul {
        noul: f32,
    },
    Choice {
        #[allow(dead_code)]
        choice: String,
        #[allow(dead_code)]
        confidence: f32,
        #[allow(dead_code)]
        #[serde(default)]
        probabilities: HashMap<String, f32>,
    },
    Score {
        score: f32,
        confidence: f32,
        #[allow(dead_code)]
        #[serde(default)]
        legend: HashMap<String, String>,
        #[allow(dead_code)]
        #[serde(default)]
        probabilities: HashMap<String, f32>,
    },
}

#[derive(Debug, Deserialize)]
struct Response {
    #[allow(dead_code)]
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    answers: HashMap<String, Answer>,
    #[allow(dead_code)]
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy, Default)]
struct Usage {
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
}
