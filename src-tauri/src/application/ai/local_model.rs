//! 本地模型接入（V1.2.1 阶段 D5 / AI_ARCHITECTURE_PLAN §5，Ollama HTTP）。
//!
//! 边界与纪律：
//! - **只连本机**：地址必须是 `localhost` / `127.0.0.1` / `::1`（`policy::normalize_local_endpoint` 把关），
//!   远程模型是另一档决策，需要单独授权，本版不做；
//! - **默认关闭**：没配地址或模型名时完全不发请求，报告走模板实现、打分走规则档（§9.6 降级路径）；
//! - **失败即降级**：任何错误（没装 Ollama、模型不存在、超时）都只记审计并回退到规则档/模板，
//!   绝不让研究流程卡住；
//! - **生成与决策分开**：`complete` 是生成（替换模板报告文字）；`decide` 是对「打分」的近似
//!   ——按 §9.6，真正做决策的是 Jev（类型化 + 校准置信度），本文件里的 `decide` 只是
//!   与规则档 A/B 用的同构实现，界面上必须如实标注「未校准」。

use std::time::Duration;

use crate::core::error::{AppError, ErrorCode, Result};

use super::traits::{
    DecisionChoice, DecisionKind, DecisionModel, DecisionResult, DecisionState, GenerationRequest,
    GenerationResponse, GenerativeModel,
};

/// 单次调用超时（本地模型首 token 可能较慢，这里给足）。
const TIMEOUT: Duration = Duration::from_secs(120);
/// 生成结果的字符上限（防止把超长输出整篇写进报告）。
const MAX_OUTPUT_CHARS: usize = 6000;

/// Ollama 客户端（生成 + 打分共用同一个连接配置）。
#[derive(Debug, Clone)]
pub struct OllamaModel {
    endpoint: String,
    model: String,
}

impl OllamaModel {
    pub fn new(endpoint: &str, model: &str) -> Self {
        Self {
            endpoint: endpoint.trim().trim_end_matches('/').to_string(),
            model: model.trim().to_string(),
        }
    }

    pub fn id(&self) -> String {
        format!("local.ollama:{}", self.model)
    }

    /// 一次非流式生成（`POST /api/generate`，`stream: false`）。
    fn generate(&self, prompt: &str, expect_json: bool) -> Result<String> {
        let agent = ureq::Agent::new_with_config(
            ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).build(),
        );
        let mut body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
        });
        if expect_json {
            body["format"] = serde_json::Value::String("json".to_string());
        }
        let response = agent
            .post(format!("{}/api/generate", self.endpoint))
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(|error| {
                AppError::new(
                    ErrorCode::InvalidInput,
                    format!(
                        "本地模型调用失败：{error}（检查 Ollama 是否在运行、模型 `{}` 是否已 pull）",
                        self.model
                    ),
                )
            })?;
        let text = response
            .into_body()
            .read_to_string()
            .map_err(|error| AppError::new(ErrorCode::InvalidInput, format!("读取模型输出失败：{error}")))?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|error| {
            AppError::new(ErrorCode::InvalidInput, format!("模型响应不是合法 JSON：{error}"))
        })?;
        let content = value
            .get("response")
            .and_then(|entry| entry.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        if content.is_empty() {
            return Err(AppError::new(ErrorCode::InvalidInput, "本地模型返回了空内容。"));
        }
        Ok(crate::core::text::truncate_chars(&content, MAX_OUTPUT_CHARS))
    }
}

impl GenerativeModel for OllamaModel {
    fn id(&self) -> &'static str {
        // trait 要求 `&'static str`；这里返回稳定的短标识，完整标识用 `OllamaModel::id()`
        "local.ollama"
    }

    fn is_local(&self) -> bool {
        true
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    fn complete(&self, request: &GenerationRequest) -> Result<GenerationResponse> {
        let started = std::time::Instant::now();
        let prompt = build_generation_prompt(request);
        let text = self.generate(&prompt, false)?;
        Ok(GenerationResponse {
            text,
            model: self.id(),
            duration_ms: started.elapsed().as_millis() as u64,
            usage: None,
        })
    }
}

impl DecisionModel for OllamaModel {
    fn id(&self) -> &'static str {
        "local.ollama"
    }

    fn is_local(&self) -> bool {
        true
    }

    fn decide(&self, kind: DecisionKind, state: &DecisionState) -> Result<Vec<DecisionResult>> {
        if state.candidates.is_empty() {
            return Ok(Vec::new());
        }
        let prompt = build_decision_prompt(kind, state);
        let raw = self.generate(&prompt, true)?;
        parse_decision_output(&raw, kind, state)
    }
}

/// 生成侧提示词：沿用 §9.6 的分工（模型只写文字，不做决策），并要求可引用的写法。
///
/// 本地与在线模型**共用同一份提示词**：否则「换模型后报告口径变了」会变成
/// 一件说不清的事（同一份材料，两个模型两套规则）。
pub(crate) fn build_generation_prompt(request: &GenerationRequest) -> String {
    let mut prompt = String::from(
        "你在一个「本地优先」的个人知识库里撰写调研材料。要求：\n\
         1) 只根据给定材料说话，不要编造事实或来源；\n\
         2) 每条事实后面用 [n] 标注它来自第几条材料；\n\
         3) 材料之间矛盾时并列呈现、不要替读者选边；\n\
         4) 用简体中文、Markdown，控制在 400 字以内。\n\n",
    );
    prompt.push_str(&format!("任务：{}\n\n", request.prompt));
    if request.evidence.is_empty() {
        prompt.push_str("材料：（无）\n");
    } else {
        prompt.push_str("材料：\n");
        for (index, chunk) in request.evidence.iter().enumerate() {
            let heading = if chunk.heading_path.is_empty() {
                String::new()
            } else {
                format!("{} · ", chunk.heading_path.join(" / "))
            };
            prompt.push_str(&format!(
                "[{}] {}{} —— {}\n",
                index + 1,
                heading,
                chunk.snippet,
                chunk.note_id
            ));
        }
    }
    prompt
}

/// 决策侧提示词（近似 Jev 的类型化决策，未校准——界面必须如实标注）。
pub(crate) fn build_decision_prompt(kind: DecisionKind, state: &DecisionState) -> String {
    let mut prompt = format!(
        "你是检索结果的判定器。判定类型：{kind:?}。\n\
         用户问题：{}\n\n候选：\n",
        state.query
    );
    for candidate in &state.candidates {
        prompt.push_str(&format!(
            "- id={} title={} 可靠性={:?} 片段={}\n",
            candidate.note_id,
            candidate.title,
            candidate.source_reliability,
            crate::core::text::truncate_chars(&candidate.snippet, 200)
        ));
    }
    prompt.push_str(
        "\n只输出 JSON，不要解释：{\"scores\":[{\"id\":\"<原样回填 id>\",\"score\":0.0-1.0,\"reason\":\"一句话\"}]}\n\
         score 表示「这条候选对回答该问题的价值」，越高越有用。",
    );
    prompt
}

/// 解析决策输出（严格校验：id 必须是候选之一，score 必须是 0–1 的数字）。
pub fn parse_decision_output(
    raw: &str,
    kind: DecisionKind,
    state: &DecisionState,
) -> Result<Vec<DecisionResult>> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|error| {
        AppError::new(
            ErrorCode::InvalidInput,
            format!("模型打分输出不是合法 JSON：{error}"),
        )
    })?;
    let scores = value
        .get("scores")
        .and_then(|entry| entry.as_array())
        .ok_or_else(|| AppError::new(ErrorCode::InvalidInput, "模型打分输出缺少 scores 数组。"))?;
    let mut results = Vec::new();
    for item in scores {
        let Some(id) = item.get("id").and_then(|entry| entry.as_str()) else {
            continue;
        };
        if !state.candidates.iter().any(|candidate| candidate.note_id == id) {
            continue; // 模型编造的 id 一律丢弃
        }
        let Some(score) = item.get("score").and_then(serde_json::Value::as_f64) else {
            continue;
        };
        let score = (score as f32).clamp(0.0, 1.0);
        let reason = item
            .get("reason")
            .and_then(|entry| entry.as_str())
            .unwrap_or("模型未给出理由")
            .trim()
            .to_string();
        results.push(DecisionResult {
            note_id: id.to_string(),
            choice: if score >= 0.5 {
                DecisionChoice::Use
            } else {
                DecisionChoice::Drop
            },
            score,
            confidence: score,
            reason,
            kind,
        });
    }
    if results.is_empty() {
        return Err(AppError::new(
            ErrorCode::InvalidInput,
            "模型打分输出里没有可用的条目（id 与候选不匹配或分数缺失）。",
        ));
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::traits::DecisionCandidate;

    fn state() -> DecisionState {
        DecisionState {
            query: "Rust 异步调度".to_string(),
            candidates: vec![
                DecisionCandidate {
                    note_id: "note_a".to_string(),
                    title: "方案对比".to_string(),
                    snippet: "调度实现".to_string(),
                    updated_at: "2026-10-11T10:00:00+08:00".to_string(),
                    source_reliability: Some(4),
                    valid_until: None,
                },
                DecisionCandidate {
                    note_id: "note_b".to_string(),
                    title: "闲聊".to_string(),
                    snippet: "无关内容".to_string(),
                    updated_at: "2026-10-11T10:00:00+08:00".to_string(),
                    source_reliability: None,
                    valid_until: None,
                },
            ],
        }
    }

    #[test]
    fn model_id_is_stable_and_local() {
        let model = OllamaModel::new("http://127.0.0.1:11434/", "qwen2.5:7b");
        assert_eq!(model.id(), "local.ollama:qwen2.5:7b");
        assert_eq!(<OllamaModel as GenerativeModel>::id(&model), "local.ollama");
        assert_eq!(<OllamaModel as DecisionModel>::id(&model), "local.ollama");
        assert!(<OllamaModel as GenerativeModel>::is_local(&model));
        assert!(<OllamaModel as DecisionModel>::is_local(&model));
        assert!(!<OllamaModel as GenerativeModel>::supports_streaming(&model));
    }

    #[test]
    fn decision_output_is_validated_against_candidates() {
        let raw = r#"{"scores":[
            {"id":"note_a","score":0.82,"reason":"直接回答"},
            {"id":"编造的 id","score":0.99,"reason":"幻觉"},
            {"id":"note_b","score":2.5,"reason":"越界"}
        ]}"#;
        let results = parse_decision_output(raw, DecisionKind::Relevance, &state()).expect("可解析");
        assert_eq!(results.len(), 2, "编造的 id 必须被丢弃");
        assert_eq!(results[0].note_id, "note_a");
        assert!((results[0].score - 0.82).abs() < 1e-6);
        assert_eq!(results[0].choice, DecisionChoice::Use);
        assert!((results[1].score - 1.0).abs() < 1e-6, "越界分数夹取到 1.0");
    }

    #[test]
    fn invalid_decision_output_is_rejected_not_guessed() {
        assert!(parse_decision_output("不是 JSON", DecisionKind::Relevance, &state()).is_err());
        assert!(parse_decision_output(r#"{"foo":1}"#, DecisionKind::Relevance, &state()).is_err());
        assert!(
            parse_decision_output(r#"{"scores":[]}"#, DecisionKind::Relevance, &state()).is_err(),
            "空结果必须报错，让上层回退到规则档"
        );
        assert!(
            parse_decision_output(
                r#"{"scores":[{"id":"编造","score":0.9}]}"#,
                DecisionKind::Relevance,
                &state()
            )
            .is_err()
        );
    }

    #[test]
    fn generation_prompt_lists_evidence_with_reference_numbers() {
        let request = GenerationRequest {
            prompt: "写推测段".to_string(),
            evidence: vec![crate::application::ai::traits::RetrievedChunk {
                note_id: "note_a".to_string(),
                chunk_index: 0,
                heading_path: vec!["调度".to_string()],
                snippet: "工作窃取".to_string(),
                char_start: 0,
                char_end: 4,
                score: 1.0,
                origin: crate::application::ai::traits::RetrievalOrigin::Bm25,
            }],
            max_tokens: None,
            allow_remote: false,
        };
        let prompt = build_generation_prompt(&request);
        assert!(prompt.contains("[1] 调度 · 工作窃取"));
        assert!(prompt.contains("不要编造"));
        assert!(prompt.contains("不要替读者选边"));
    }
}
