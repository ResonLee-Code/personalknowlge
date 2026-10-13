//! 证据充分性（D5 / N9 第二步）：**"这些材料够不够回答这个问题"**。
//!
//! 为什么必须有这一层：研究助手的默认失败方式不是"答错"，而是**硬答**——
//! 材料只覆盖了 5 个子问题里的 1 个，却仍然生成一份读起来很完整的报告。
//! 用户没有别的线索能看出来，于是把"没材料"误读成"查过了，就是这样"。
//!
//! 所以这里的判定只有一条原则：**任何一条不满足，就如实说"不够"**，并说清差在哪：
//!
//! | 判定 | 含义 | 用户该做什么 |
//! | --- | --- | --- |
//! | `NoMaterial` | 一条材料都没有 | 先攒材料，或打开联网档位 |
//! | `PartialCoverage` | 有子问题完全没有材料 | 针对缺口补材料（报告里点名是哪几个） |
//! | `NoCrossCheck` | 没有任何"≥2 个独立来源" | 再找一条独立来源，否则只能进「待验证」 |
//! | `ThinEvidence` | 可用来源太少 | 材料太少时结论段天然为空，这是**故意的** |
//! | `WeakOnly` | 只有 E 级（不可用）线索 | 换更可靠的来源 |
//! | `StaleOnly` | 全部可能已过时 | 核对时效 |
//!
//! 判定口径与「结论段的交叉验证」**共用同一个独立性定义**（见 `source_identity`），
//! 不允许出现"评估说够、结论段却一条都进不去"这种自相矛盾。

use crate::application::ai::source_grade;
use crate::domain::research::{
    EvidenceAssessment, EvidenceGap, EvidenceGapKind, ResearchSource, SourceLevel, SubQuestion,
};

/// 可用来源（排除 E 级）少于这个数就算"材料太薄"。
pub const MIN_USABLE_SOURCES: usize = 3;
/// 「独立来源」至少要这么多，才可能形成交叉验证过的结论。
pub const MIN_INDEPENDENT_SOURCES: usize = 2;

/// 一个来源的**身份**：用来数"到底有几个独立来源"。
///
/// 口径（与结论段的 `cross_verified` 完全一致，不允许两套）：
/// - 本地笔记 → `note:<id>`；
/// - 外部网页 → `host:<域名>`（**同一站点多篇 = 一个独立来源**，这是保守方向）；
/// - 都没有 → `ref:<编号>`（兜底，至少不会互相顶替）。
pub fn source_identity(source: &ResearchSource) -> String {
    if let Some(note_id) = source.note_id.as_deref().filter(|value| !value.is_empty()) {
        return format!("note:{note_id}");
    }
    if let Some(host) = source_grade::host_of(source.url.as_deref().unwrap_or("")) {
        return format!("host:{host}");
    }
    format!("ref:{}", source.reference)
}

/// 评估一批材料够不够回答这组子问题。
pub fn assess(topic: &str, sub_questions: &[SubQuestion], sources: &[ResearchSource]) -> EvidenceAssessment {
    let total = sub_questions.len();
    let usable: Vec<&ResearchSource> = sources
        .iter()
        .filter(|source| source.level != SourceLevel::E)
        .collect();
    let independent = {
        let mut keys: Vec<String> = Vec::new();
        for source in &usable {
            let key = source_identity(source);
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys.len()
    };

    // 每个子问题各有多少材料（按子问题 id 统计；无归属的不算覆盖）
    let mut uncovered: Vec<String> = Vec::new();
    let mut covered = 0usize;
    for question in sub_questions {
        let count = sources
            .iter()
            .filter(|source| source.sub_question_id == question.id && source.level != SourceLevel::E)
            .count();
        if count == 0 {
            uncovered.push(question.id.clone());
        } else {
            covered += 1;
        }
    }

    let mut gaps: Vec<EvidenceGap> = Vec::new();
    if sources.is_empty() {
        gaps.push(EvidenceGap {
            kind: EvidenceGapKind::NoMaterial,
            message: "本地知识库里没有找到任何与课题相关的材料。".to_string(),
            sub_question_ids: sub_questions.iter().map(|item| item.id.clone()).collect(),
            references: Vec::new(),
        });
    } else if usable.is_empty() {
        gaps.push(EvidenceGap {
            kind: EvidenceGapKind::WeakOnly,
            message: format!(
                "找到 {} 条材料，但**全部是 E 级**（论坛贴 / 社交平台 / AI 生成内容）：只能当线索，撑不起结论。",
                sources.len()
            ),
            sub_question_ids: Vec::new(),
            references: sources.iter().map(|source| source.reference).collect(),
        });
    } else {
        if !uncovered.is_empty() {
            gaps.push(EvidenceGap {
                kind: EvidenceGapKind::PartialCoverage,
                message: format!(
                    "{} 个子问题里有 {} 个**一条材料都没有**：这部分不是「查过没有」，而是「没查」。",
                    total,
                    uncovered.len()
                ),
                sub_question_ids: uncovered.clone(),
                references: Vec::new(),
            });
        }
        if independent < MIN_INDEPENDENT_SOURCES && !usable.is_empty() {
            gaps.push(EvidenceGap {
                kind: EvidenceGapKind::NoCrossCheck,
                message: format!(
                    "可用来源只有 {} 个**独立**来源（同一站点/同一篇笔记算一个）：没有 ≥{MIN_INDEPENDENT_SOURCES} 个独立来源，任何结论都进不了结论段。",
                    independent
                ),
                sub_question_ids: Vec::new(),
                references: usable.iter().map(|source| source.reference).collect(),
            });
        }
        if usable.len() < MIN_USABLE_SOURCES {
            gaps.push(EvidenceGap {
                kind: EvidenceGapKind::ThinEvidence,
                message: format!(
                    "可用来源只有 {} 条（少于 {MIN_USABLE_SOURCES} 条）：材料太薄，报告里会出现大段空白。",
                    usable.len()
                ),
                sub_question_ids: Vec::new(),
                references: usable.iter().map(|source| source.reference).collect(),
            });
        }
        if usable.iter().all(|source| source.stale) {
            gaps.push(EvidenceGap {
                kind: EvidenceGapKind::StaleOnly,
                message: "可用来源**全部**可能已过时（超出时间窗或 valid_until 已过）：结论不能只靠它们。"
                    .to_string(),
                sub_question_ids: Vec::new(),
                references: usable.iter().map(|source| source.reference).collect(),
            });
        }
    }

    let sufficient = gaps.is_empty();
    let coverage = if total == 0 {
        1.0
    } else {
        covered as f32 / total as f32
    };
    let cross_check = (independent as f32 / MIN_INDEPENDENT_SOURCES as f32).min(1.0);
    let freshness = if usable.is_empty() {
        0.0
    } else {
        usable.iter().filter(|source| !source.stale).count() as f32 / usable.len() as f32
    };
    let score = if sources.is_empty() {
        0.0
    } else {
        0.5 * coverage + 0.3 * cross_check + 0.2 * freshness
    };

    let verdict = if sufficient {
        format!(
            "证据充分：{}/{} 个子问题有材料，{} 个独立来源，足以支撑结论段。",
            covered, total, independent
        )
    } else {
        format!(
            "**证据不足**：{}/{} 个子问题有材料，{} 个独立来源 —— 本次**不给结论**，缺口见下。",
            covered, total, independent
        )
    };
    let recommendation = if sources.is_empty() {
        format!(
            "课题「{topic}」在本库里没有材料。可以：① 先在库中记录几篇相关材料再来调研；② 在「设置 → AI」打开联网档位（默认仅本地），让本次调研去外部找。"
        )
    } else if !sufficient {
        "缺口就是上面每条 `gaps`：按子问题补材料、补一条独立来源、或核对时效后重跑收集（`research_collect`）。"
            .to_string()
    } else {
        String::new()
    };

    EvidenceAssessment {
        sufficient,
        score,
        covered_sub_questions: covered,
        total_sub_questions: total,
        independent_sources: independent,
        usable_sources: usable.len(),
        gaps,
        verdict,
        recommendation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::research::{SourceOrigin, ResearchSource};

    fn question(id: &str) -> SubQuestion {
        SubQuestion {
            id: id.to_string(),
            question: format!("「{id}」是什么？"),
            keywords: vec!["测试".to_string()],
            local_hits: 0,
        }
    }

    fn source(reference: usize, question_id: &str, level: SourceLevel, stale: bool) -> ResearchSource {
        ResearchSource {
            reference,
            title: format!("材料 {reference}"),
            url: None,
            origin: SourceOrigin::Local,
            note_id: Some(format!("note_{reference}")),
            sub_question_id: question_id.to_string(),
            level,
            reliability: level.reliability(),
            grade_reason: String::new(),
            published_at: None,
            fetched_at: "2026-10-14T00:00:00+08:00".to_string(),
            applicable_version: None,
            snippet: "片段".to_string(),
            char_start: None,
            char_end: None,
            heading: None,
            stale,
            credibility: 0.5,
        }
    }

    #[test]
    fn no_material_is_reported_as_no_material_not_as_a_conclusion() {
        let questions = vec![question("q1"), question("q2")];
        let assessment = assess("课题", &questions, &[]);
        assert!(!assessment.sufficient);
        assert_eq!(assessment.score, 0.0);
        assert_eq!(assessment.gaps.len(), 1);
        assert_eq!(assessment.gaps[0].kind, EvidenceGapKind::NoMaterial);
        assert_eq!(assessment.gaps[0].sub_question_ids, vec!["q1", "q2"]);
        assert!(assessment.verdict.contains("证据不足"));
        assert!(assessment.recommendation.contains("联网"));
    }

    #[test]
    fn a_single_source_is_not_enough_for_a_conclusion() {
        let questions = vec![question("q1")];
        let sources = vec![source(1, "q1", SourceLevel::C, false)];
        let assessment = assess("课题", &questions, &sources);
        assert!(!assessment.sufficient);
        let kinds: Vec<EvidenceGapKind> = assessment.gaps.iter().map(|gap| gap.kind).collect();
        assert!(kinds.contains(&EvidenceGapKind::NoCrossCheck), "{kinds:?}");
        assert!(kinds.contains(&EvidenceGapKind::ThinEvidence), "{kinds:?}");
    }

    #[test]
    fn gaps_name_the_sub_questions_that_have_nothing() {
        let questions = vec![question("q1"), question("q2"), question("q3")];
        let sources = vec![
            source(1, "q1", SourceLevel::B, false),
            source(2, "q1", SourceLevel::B, false),
            source(3, "q1", SourceLevel::B, false),
        ];
        let assessment = assess("课题", &questions, &sources);
        let gap = assessment
            .gaps
            .iter()
            .find(|gap| gap.kind == EvidenceGapKind::PartialCoverage)
            .expect("应当指出覆盖缺口");
        assert_eq!(gap.sub_question_ids, vec!["q2", "q3"]);
        assert!(!assessment.sufficient);
    }

    #[test]
    fn same_host_different_pages_count_as_one_independent_source() {
        let questions = vec![question("q1")];
        let mut first = source(1, "q1", SourceLevel::A, false);
        first.note_id = None;
        first.origin = SourceOrigin::Web;
        first.url = Some("https://example.com/a".to_string());
        let mut second = source(2, "q1", SourceLevel::A, false);
        second.note_id = None;
        second.origin = SourceOrigin::Web;
        second.url = Some("https://example.com/b".to_string());
        let assessment = assess("课题", &questions, &[first, second]);
        assert_eq!(
            assessment.independent_sources, 1,
            "同一站点的两页只能算一个独立来源（保守口径）"
        );
        assert!(!assessment.sufficient);
    }

    #[test]
    fn everything_stale_is_its_own_gap() {
        let questions = vec![question("q1")];
        let sources: Vec<ResearchSource> = (1..=3)
            .map(|index| source(index, "q1", SourceLevel::A, true))
            .collect();
        let assessment = assess("课题", &questions, &sources);
        assert!(!assessment.sufficient);
        assert!(
            assessment
                .gaps
                .iter()
                .any(|gap| gap.kind == EvidenceGapKind::StaleOnly),
            "{:?}",
            assessment.gaps
        );
    }

    #[test]
    fn enough_independent_fresh_sources_covering_every_question_is_sufficient() {
        let questions = vec![question("q1"), question("q2")];
        let sources = vec![
            source(1, "q1", SourceLevel::A, false),
            source(2, "q1", SourceLevel::B, false),
            source(3, "q2", SourceLevel::A, false),
            source(4, "q2", SourceLevel::B, false),
        ];
        let assessment = assess("课题", &questions, &sources);
        assert!(assessment.sufficient, "{:?}", assessment.gaps);
        assert_eq!(assessment.covered_sub_questions, 2);
        assert_eq!(assessment.independent_sources, 4);
        assert!(assessment.recommendation.is_empty());
        assert!(assessment.verdict.contains("证据充分"));
    }
}
