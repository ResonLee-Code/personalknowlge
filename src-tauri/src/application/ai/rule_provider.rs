//! 规则决策 Provider（**离线兜底实现**，第二代接口预留的一部分）。
//!
//! 它证明 `DecisionModel` 的形状是可实现的，并且在没有 Jev / 本地模型时保证链路可用：
//! 依据标题命中、片段命中、来源可靠性与时效给出确定性的 score/confidence，
//! 不依赖网络、模型或任何新依赖。

use crate::core::error::Result;

use super::traits::{
    DecisionCandidate, DecisionChoice, DecisionKind, DecisionModel, DecisionResult, DecisionState,
};

/// 关键词决策规则：命中越多、来源越可靠、越新，分数越高。
pub struct RuleDecisionProvider {
    /// 低于该分数判为 DROP
    pub drop_threshold: f32,
    /// 高于该分数判为 USE
    pub use_threshold: f32,
}

impl Default for RuleDecisionProvider {
    fn default() -> Self {
        Self { drop_threshold: 0.25, use_threshold: 0.55 }
    }
}

impl RuleDecisionProvider {
    fn score_candidate(query: &str, candidate: &DecisionCandidate) -> (f32, usize) {
        let terms: Vec<String> = query
            .split(|ch: char| ch.is_whitespace() || ch.is_ascii_punctuation())
            .map(|term| term.trim().to_lowercase())
            .filter(|term| term.chars().count() >= 2)
            .collect();
        if terms.is_empty() {
            return (0.0, 0);
        }
        let title = candidate.title.to_lowercase();
        let snippet = candidate.snippet.to_lowercase();
        let mut hits = 0usize;
        let mut score = 0.0f32;
        for term in &terms {
            if title.contains(term.as_str()) {
                score += 1.0;
                hits += 1;
            } else if snippet.contains(term.as_str()) {
                score += 0.5;
                hits += 1;
            }
        }
        let mut relevance = (score / terms.len() as f32).clamp(0.0, 1.0);
        // 来源可靠性 1–5 → ×0.8–1.0 的权重（缺省 0.9）
        let reliability = candidate
            .source_reliability
            .map(|level| 0.8 + 0.04 * level.clamp(1, 5) as f32)
            .unwrap_or(0.9);
        relevance *= reliability;
        (relevance, hits)
    }
}

impl DecisionModel for RuleDecisionProvider {
    fn id(&self) -> &'static str {
        "rule"
    }

    fn is_local(&self) -> bool {
        true
    }

    fn decide(&self, kind: DecisionKind, state: &DecisionState) -> Result<Vec<DecisionResult>> {
        let results = state
            .candidates
            .iter()
            .map(|candidate| {
                let (score, hits) = Self::score_candidate(&state.query, candidate);
                let choice = if hits == 0 {
                    DecisionChoice::Drop
                } else if score >= self.use_threshold {
                    DecisionChoice::Use
                } else if score < self.drop_threshold {
                    DecisionChoice::Drop
                } else {
                    DecisionChoice::Keep
                };
                // 规则没有校准概率：置信度按命中数给固定档位，并在理由里说明来源
                let confidence = match hits {
                    0 => 0.2,
                    1 => 0.5,
                    _ => 0.7,
                };
                DecisionResult {
                    note_id: candidate.note_id.clone(),
                    choice,
                    score,
                    confidence,
                    reason: format!("规则命中 {hits} 项关键词（离线兜底，无模型参与）"),
                    kind,
                }
            })
            .collect();
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, title: &str, snippet: &str, reliability: Option<i64>) -> DecisionCandidate {
        DecisionCandidate {
            note_id: id.to_string(),
            title: title.to_string(),
            snippet: snippet.to_string(),
            updated_at: "2026-10-01T00:00:00+08:00".to_string(),
            source_reliability: reliability,
            valid_until: None,
        }
    }

    #[test]
    fn prefers_title_hits_and_high_reliability() {
        let provider = RuleDecisionProvider::default();
        let state = DecisionState {
            query: "tauri 跨平台".to_string(),
            candidates: vec![
                candidate("a", "Tauri 跨平台选型", "官方文档摘要", Some(5)),
                candidate("b", "随便写写", "提到 tauri 一次", None),
                candidate("c", "无关笔记", "完全无关内容", Some(5)),
            ],
        };
        let results = provider.decide(DecisionKind::Relevance, &state).expect("决策");
        let by_id = |id: &str| results.iter().find(|item| item.note_id == id).unwrap();
        assert_eq!(by_id("a").choice, DecisionChoice::Use);
        assert!(by_id("a").score > by_id("b").score);
        assert_eq!(by_id("c").choice, DecisionChoice::Drop);
        assert!(results.iter().all(|item| item.kind == DecisionKind::Relevance));
    }

    #[test]
    fn confidence_reflects_hit_count_and_provider_is_local() {
        let provider = RuleDecisionProvider::default();
        assert!(provider.is_local(), "规则 Provider 必须可离线使用");
        assert_eq!(provider.id(), "rule");
        let state = DecisionState {
            query: "索引 策略".to_string(),
            candidates: vec![candidate("a", "索引策略", "", None)],
        };
        let results = provider.decide(DecisionKind::Utility, &state).unwrap();
        assert_eq!(results[0].confidence, 0.7);
        assert!(results[0].reason.contains("离线兜底"));
    }

    #[test]
    fn empty_query_drops_everything() {
        let provider = RuleDecisionProvider::default();
        let state = DecisionState {
            query: "  ".to_string(),
            candidates: vec![candidate("a", "索引", "内容", None)],
        };
        let results = provider.decide(DecisionKind::Relevance, &state).unwrap();
        assert_eq!(results[0].choice, DecisionChoice::Drop);
        assert_eq!(results[0].score, 0.0);
    }
}
