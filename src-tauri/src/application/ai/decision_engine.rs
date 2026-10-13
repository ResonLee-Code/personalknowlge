//! 决策层骨架（D2 / N6）：**Engine + 缓存 + Trace**（不接模型）。
//!
//! 边界与职责：
//! - **Engine** 只做三件事：查缓存 → 按候选链逐档试 → 写缓存 + 记 Trace；
//! - 「哪一档可用」由调用方（`research.rs`）用闸门算好后传进来——引擎自己不做外发判断；
//! - **每一次决策都留下可解释的 Trace**（试过谁、谁失败、为什么、命中缓存没有），
//!   但只记元数据，**不含正文**。

use std::sync::Arc;
use std::time::Instant;

use crate::application::ai::decision_cache::{DecisionCache, TTL_SECS};
use crate::application::ai::traits::{DecisionKind, DecisionModel, DecisionResult, DecisionState};
use crate::core::error::Result;
use crate::domain::ai::{DecisionAttempt, DecisionTrace};
use crate::infrastructure::db::Db;

/// 一次决策的结果 + 可解释 Trace。
#[derive(Debug, Clone)]
pub struct DecisionOutcome {
    pub results: Vec<DecisionResult>,
    /// 最终生效的档位（全军覆没时为空串）
    pub provider: String,
    pub trace: DecisionTrace,
}

pub struct DecisionEngine {
    cache: DecisionCache,
}

impl Default for DecisionEngine {
    fn default() -> Self {
        Self { cache: DecisionCache }
    }
}

impl DecisionEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// 走一遍决策：先查缓存，未命中再按 `chain` 逐档试，并把过程记进 Trace。
    pub fn decide(
        &self,
        db: &Arc<Db>,
        kind: DecisionKind,
        state: &DecisionState,
        chain: &[Arc<dyn DecisionModel>],
    ) -> Result<DecisionOutcome> {
        let started = Instant::now();
        let chain_ids: Vec<String> = chain.iter().map(|model| model.id().to_string()).collect();
        let key = DecisionCache::key(kind, state, &chain_ids);

        if let Some(cached) = db.with_conn(|conn| self.cache.get(conn, &key))? {
            return Ok(DecisionOutcome {
                results: cached.results,
                provider: cached.provider.clone(),
                trace: DecisionTrace {
                    kind: kind.as_str().to_string(),
                    cache: "hit".to_string(),
                    provider: cached.provider,
                    candidates: state.candidates.len(),
                    // 命中缓存=什么都没试：`attempts` 空着是**如实**的
                    attempts: Vec::new(),
                    took_ms: started.elapsed().as_millis() as u64,
                },
            });
        }

        let mut attempts: Vec<DecisionAttempt> = Vec::new();
        let mut outcome: Option<(String, Vec<DecisionResult>)> = None;
        for model in chain {
            let provider = model.id().to_string();
            match model.decide(kind, state) {
                Ok(results) if !results.is_empty() => {
                    attempts.push(DecisionAttempt {
                        provider: provider.clone(),
                        ok: true,
                        detail: format!("返回 {} 条", results.len()),
                    });
                    outcome = Some((provider, results));
                    break;
                }
                Ok(_) => attempts.push(DecisionAttempt {
                    provider,
                    ok: false,
                    detail: "没有给出结果".to_string(),
                }),
                Err(error) => attempts.push(DecisionAttempt {
                    provider,
                    ok: false,
                    detail: error.message,
                }),
            }
        }

        let (provider, results) = outcome.unwrap_or_else(|| (String::new(), Vec::new()));
        if !results.is_empty() {
            // 写缓存失败不该影响这次决策（结果已经拿到了）
            if let Err(error) =
                db.with_conn(|conn| self.cache.put(conn, &key, kind, &provider, &results, TTL_SECS))
            {
                tracing::debug!(target: "ai", "写决策缓存失败：{}", error.message);
            }
        }
        Ok(DecisionOutcome {
            results,
            provider: provider.clone(),
            trace: DecisionTrace {
                kind: kind.as_str().to_string(),
                cache: "miss".to_string(),
                provider,
                candidates: state.candidates.len(),
                attempts,
                took_ms: started.elapsed().as_millis() as u64,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::traits::{
        DecisionCandidate, DecisionChoice, DecisionResult,
    };
    use crate::core::error::{AppError, ErrorCode};
    use crate::infrastructure::db::Db;

    struct FakeModel {
        id: &'static str,
        result: Result<Vec<DecisionResult>>,
    }

    impl DecisionModel for FakeModel {
        fn id(&self) -> &'static str {
            self.id
        }
        fn is_local(&self) -> bool {
            true
        }
        fn decide(
            &self,
            _kind: DecisionKind,
            _state: &DecisionState,
        ) -> Result<Vec<DecisionResult>> {
            match &self.result {
                Ok(results) => Ok(results.clone()),
                Err(error) => Err(AppError::new(ErrorCode::InvalidInput, error.message.clone())),
            }
        }
    }

    fn candidate(id: &str) -> DecisionCandidate {
        DecisionCandidate {
            note_id: id.to_string(),
            title: format!("标题 {id}"),
            snippet: format!("片段 {id}"),
            updated_at: "2026-10-12T10:00:00+08:00".to_string(),
            source_reliability: Some(4),
            valid_until: None,
        }
    }

    fn state() -> DecisionState {
        DecisionState { query: "调度".to_string(), candidates: vec![candidate("a")] }
    }

    fn scored(id: &str) -> DecisionResult {
        DecisionResult {
            note_id: id.to_string(),
            choice: DecisionChoice::Use,
            score: 0.8,
            confidence: 0.7,
            reason: "测试".to_string(),
            kind: DecisionKind::Relevance,
        }
    }

    fn db() -> Arc<Db> {
        Arc::new(Db::in_memory().expect("内存库"))
    }

    #[test]
    fn first_call_is_a_miss_then_the_same_input_hits_the_cache() {
        let db = db();
        let engine = DecisionEngine::new();
        let chain: Vec<Arc<dyn DecisionModel>> =
            vec![Arc::new(FakeModel { id: "rule", result: Ok(vec![scored("a")]) })];

        let first = engine.decide(&db, DecisionKind::Relevance, &state(), &chain).expect("首次");
        assert_eq!(first.trace.cache, "miss");
        assert_eq!(first.provider, "rule");
        assert_eq!(first.trace.attempts.len(), 1);
        assert!(first.trace.attempts[0].ok);

        // 同一个输入第二次：应当直接吃缓存（不再走候选链）
        let panicking: Vec<Arc<dyn DecisionModel>> =
            vec![Arc::new(FakeModel { id: "rule", result: Err(AppError::internal("不该被调用")) })];
        let second = engine.decide(&db, DecisionKind::Relevance, &state(), &panicking).expect("二次");
        assert_eq!(second.trace.cache, "hit", "同输入第二次应当命中缓存");
        assert_eq!(second.provider, "rule");
        assert!(second.trace.attempts.is_empty(), "命中缓存=什么都没试");
    }

    #[test]
    fn content_change_invalidates_the_cache_naturally() {
        let db = db();
        let engine = DecisionEngine::new();
        let chain: Vec<Arc<dyn DecisionModel>> =
            vec![Arc::new(FakeModel { id: "rule", result: Ok(vec![scored("a")]) })];
        engine.decide(&db, DecisionKind::Relevance, &state(), &chain).expect("首次");

        let mut changed = state();
        changed.candidates[0].snippet = "片段换了内容".to_string();
        let again = engine.decide(&db, DecisionKind::Relevance, &changed, &chain).expect("内容变更");
        assert_eq!(again.trace.cache, "miss", "内容变了必须重新算，不能吃旧缓存");
    }

    #[test]
    fn failures_are_recorded_in_order_and_the_next_tier_wins() {
        let db = db();
        let engine = DecisionEngine::new();
        let chain: Vec<Arc<dyn DecisionModel>> = vec![
            Arc::new(FakeModel {
                id: "local.ollama",
                result: Err(AppError::new(ErrorCode::InvalidInput, "连不上".to_string())),
            }),
            Arc::new(FakeModel { id: "rule", result: Ok(vec![scored("a")]) }),
        ];

        let outcome = engine.decide(&db, DecisionKind::Relevance, &state(), &chain).expect("决策");
        assert_eq!(outcome.provider, "rule");
        let attempts: Vec<(&str, bool)> = outcome
            .trace
            .attempts
            .iter()
            .map(|attempt| (attempt.provider.as_str(), attempt.ok))
            .collect();
        assert_eq!(attempts, vec![("local.ollama", false), ("rule", true)]);
        assert!(outcome.trace.attempts[0].detail.contains("连不上"), "失败原因要能解释");
    }

    #[test]
    fn all_tiers_failing_returns_empty_with_a_full_trace() {
        let db = db();
        let engine = DecisionEngine::new();
        let chain: Vec<Arc<dyn DecisionModel>> = vec![Arc::new(FakeModel {
            id: "local.ollama",
            result: Err(AppError::new(ErrorCode::InvalidInput, "连不上".to_string())),
        })];

        let outcome = engine.decide(&db, DecisionKind::Relevance, &state(), &chain).expect("决策");
        assert!(outcome.results.is_empty());
        assert_eq!(outcome.provider, "");
        assert_eq!(outcome.trace.attempts.len(), 1);
    }
}
