//! 决策模型契约探针（D4/第 2 项）。
//!
//! 目的：**用我们自己的中文题目**回答"这个决策模型在我们场景上到底行不行"，
//! 而不是引用厂商的宣称。指标就是那份公开研究用的那几个：
//! - **准确率**（有标准答案的题目）
//! - **自洽性**：同一件事的正问与反问 P(是) 之和应 ≈1
//! - **确定性**：同一请求跑两次，答案是否逐位相同（官方说"通常相同"）
//! - **矛盾判别**：给一对真矛盾/一对不矛盾的material，看 noul 能不能分开
//! - **耗时与用量**（据此估成本）
//!
//! 只读：不碰知识库，不改设置，只发这一小批探针请求。

use std::sync::Arc;
use std::time::Instant;

use crate::application::ai::policy::AiPolicyService;
use crate::application::ai::system_one::SystemOneDecisionModel;
use crate::core::error::{AppError, ErrorCode, Result};

/// Perplexity Decisions 单价（美元 / 输入 token）：$0.02 / 百万。
const PERPLEXITY_USD_PER_TOKEN: f64 = 0.02 / 1_000_000.0;
/// Jev 单价（美元 / 输入 token）：$42 / B（据公开评测报告的报价口径）。
const JEV_USD_PER_TOKEN: f64 = 42.0 / 1_000_000_000.0;

/// 一条探针题。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionProbeItem {
    pub name: String,
    pub question: String,
    /// 标准答案（`None` = 只观察，不判对错，例如自洽性/矛盾判别）
    pub expected_yes: Option<bool>,
    pub p_yes: f32,
}

/// 探针报告。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionProbeReport {
    pub provider: String,
    pub model: String,
    pub host: String,
    pub items: Vec<DecisionProbeItem>,
    /// 有标准答案的题目上的准确率（0–1）
    pub accuracy: Option<f32>,
    pub scored_items: usize,
    /// **期望校准误差（ECE）**：越小越好；公开研究用的线是 0.05
    pub calibration: crate::application::ai::calibration::Calibration,
    /// 只算"材料里能推出来"那一族（这些题答错就是错，答对应当高置信）
    pub calibration_determinable: crate::application::ai::calibration::Calibration,
    /// 只算"**材料里没有答案**"那一族：校准的模型应当给 ~0.5，凭空的自信会在这里现形
    pub calibration_underdetermined: crate::application::ai::calibration::Calibration,
    /// "材料里没有答案"那族的**平均陈述概率**（理想 ≈0.5；明显偏离就是不懂装懂）
    pub mean_stated_underdetermined: Option<f32>,
    /// **折外**校准误差：一半拟合、一半评估（两边互换取平均）——这是"拟合有没有用"的诚实答案
    pub out_of_sample_ece: Option<f32>,
    /// 自洽性：P(甲更好) + P(甲并不更好)，理想 ≈1
    pub coherence: Option<f32>,
    /// 同一请求跑两次是否逐位相同
    pub deterministic: bool,
    /// 真矛盾那一对判出来的 P(矛盾)
    pub conflict_pair_yes: Option<f32>,
    /// 不矛盾那一对判出来的 P(矛盾)
    pub non_conflict_pair_yes: Option<f32>,
    pub calls: usize,
    pub input_tokens: i64,
    pub latency_ms: u64,
    /// 按公开单价估算的美元成本（估算值，不是账单）
    pub estimated_usd: f64,
    /// 人读结论（含"这只是小样本冒烟，不等于校准"的口径）
    pub note: String,
    /// **A/B 对比（D4 收尾）**：同一批「查询 + 候选」打分任务上，规则档与决策档各自的表现。
    /// 只看"是非题"是比不出规则的——规则档根本答不了是非题；打分才是两者共同的任务。
    pub ab: AbReport,
}

/// A/B：一个档位在同一批打分任务上的表现。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbOutcome {
    pub provider: String,
    /// 判定为"该用"的准确率（阈值 0.5，看 `choice == Use`）
    pub accuracy: f32,
    /// 相关候选的平均分
    pub mean_relevant: f32,
    /// 不相关候选的平均分
    pub mean_irrelevant: f32,
    /// 区分度：相关均分 − 不相关均分（越大越能分开）
    pub separation: f32,
    pub correct: usize,
    pub total: usize,
}

/// A/B 报告：两个档位 + 一句结论。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbReport {
    pub cases: usize,
    pub rule: AbOutcome,
    pub decision: AbOutcome,
    /// 决策档是否**严格更好**（准确率更高，或准确率持平但区分度更大）
    pub decision_better: bool,
    pub note: String,
}

/// 一条 A/B 打分用例（我们自己的中文材料，`relevant` 是人工标注的标准答案）。
pub struct AbCase {
    pub query: &'static str,
    pub note_id: &'static str,
    pub title: &'static str,
    pub snippet: &'static str,
    pub relevant: bool,
}

/// A/B 用例：4 个查询 × 4 条候选 = 16 条，相关/不相关各 8 条（平衡）。
///
/// 设计要点：不相关的那几条**与查询有字面重合但不该采用**（例如都提到"调度"却讲的是别的），
/// 这样才测得出"是按关键词命中，还是真读懂了"。规则档的强项是字面命中，这里正是它的边界。
pub const AB_CASES: &[AbCase] = &[
    // 查询 1：Rust 异步调度
    AbCase { query: "Rust 异步调度", note_id: "q1a", title: "Rust 异步调度实现要点", snippet: "实现要点在于任务唤醒与批量窃取。", relevant: true },
    AbCase { query: "Rust 异步调度", note_id: "q1b", title: "任务窃取与唤醒队列", snippet: "工作窃取如何降低锁竞争，唤醒队列如何避免惊群。", relevant: true },
    AbCase { query: "Rust 异步调度", note_id: "q1c", title: "周末爬山记录", snippet: "山上风很大，带了热水和干粮。", relevant: false },
    AbCase { query: "Rust 异步调度", note_id: "q1d", title: "Rust 异步调度的线下聚会通知", snippet: "本周五晚七点，地点待定，请接龙报名。", relevant: false },
    // 查询 2：SQLite 全文检索
    AbCase { query: "SQLite 全文检索", note_id: "q2a", title: "倒排索引与召回率", snippet: "倒排索引影响召回，长尾词需要更好的分词策略。", relevant: true },
    AbCase { query: "SQLite 全文检索", note_id: "q2b", title: "SQLite 全文检索的中文分词", snippet: "unicode61 会把连续中文当成整词，需要额外做扩展。", relevant: true },
    AbCase { query: "SQLite 全文检索", note_id: "q2c", title: "今天的天气", snippet: "多云转晴，最高 22 度。", relevant: false },
    AbCase { query: "SQLite 全文检索", note_id: "q2d", title: "SQLite 全文检索插件的安装包下载", snippet: "官网提供各平台预编译二进制，解压即可使用。", relevant: false },
    // 查询 3：备份与恢复
    AbCase { query: "备份与恢复", note_id: "q3a", title: "备份包结构与逐项校验", snippet: "zip 内含清单、数据库与全部笔记附件，支持逐项校验。", relevant: true },
    AbCase { query: "备份与恢复", note_id: "q3b", title: "恢复到空目录后能否直接打开", snippet: "恢复出来的知识库应当能直接打开，列表与计数正常。", relevant: true },
    AbCase { query: "备份与恢复", note_id: "q3c", title: "CSS 弹性布局笔记", snippet: "flex 与 grid 的取舍，以及常见的塌陷问题。", relevant: false },
    AbCase { query: "备份与恢复", note_id: "q3d", title: "备份与恢复功能的验收排班表", snippet: "本周值班：张三、李四，请按时到场。", relevant: false },
    // 查询 4：窗口尺寸记忆
    AbCase { query: "窗口尺寸记忆", note_id: "q4a", title: "窗口位置写进本地存储", snippet: "关掉再开要回到上次的尺寸与位置，越界时交给系统摆放。", relevant: true },
    AbCase { query: "窗口尺寸记忆", note_id: "q4b", title: "窗口尺寸记忆在多显示器下的处理", snippet: "拔掉外接屏后上次位置可能已在屏幕外，应只恢复尺寸。", relevant: true },
    AbCase { query: "窗口尺寸记忆", note_id: "q4c", title: "蛋糕配方", snippet: "低筋面粉 100 克，鸡蛋两个。", relevant: false },
    AbCase { query: "窗口尺寸记忆", note_id: "q4d", title: "窗口尺寸记忆功能的命名投票", snippet: "候选名称三个，投票截止到周五下班前。", relevant: false },
];

/// 用某个档位跑一遍 A/B 用例，算出准确率与区分度。
pub fn evaluate_ab(
    model: &dyn crate::application::ai::traits::DecisionModel,
    cases: &[AbCase],
) -> AbOutcome {
    use crate::application::ai::traits::{
        DecisionCandidate, DecisionChoice, DecisionKind, DecisionState,
    };

    let mut correct = 0usize;
    let mut total = 0usize;
    let mut relevant_scores: Vec<f32> = Vec::new();
    let mut irrelevant_scores: Vec<f32> = Vec::new();

    // 按查询分组：一次 `decide` 传一批候选（与真实用法一致）
    let mut queries: Vec<&str> = Vec::new();
    for case in cases {
        if !queries.contains(&case.query) {
            queries.push(case.query);
        }
    }
    for query in queries {
        let group: Vec<&AbCase> = cases.iter().filter(|case| case.query == query).collect();
        let state = DecisionState {
            query: query.to_string(),
            candidates: group
                .iter()
                .map(|case| DecisionCandidate {
                    note_id: case.note_id.to_string(),
                    title: case.title.to_string(),
                    snippet: case.snippet.to_string(),
                    updated_at: "2026-10-13T10:00:00+08:00".to_string(),
                    source_reliability: Some(4),
                    valid_until: None,
                })
                .collect(),
        };
        let Ok(results) = model.decide(DecisionKind::Relevance, &state) else {
            // 这一档这一组失败：只记"没答"，不影响另一档
            total += group.len();
            continue;
        };
        for case in group {
            let Some(result) = results.iter().find(|item| item.note_id == case.note_id) else {
                total += 1;
                continue;
            };
            total += 1;
            let predicted = result.choice == DecisionChoice::Use;
            if predicted == case.relevant {
                correct += 1;
            }
            if case.relevant {
                relevant_scores.push(result.score);
            } else {
                irrelevant_scores.push(result.score);
            }
        }
    }

    let mean = |values: &[f32]| {
        if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f32>() / values.len() as f32
        }
    };
    let mean_relevant = mean(&relevant_scores);
    let mean_irrelevant = mean(&irrelevant_scores);
    AbOutcome {
        provider: model.id().to_string(),
        accuracy: if total == 0 { 0.0 } else { correct as f32 / total as f32 },
        mean_relevant,
        mean_irrelevant,
        separation: mean_relevant - mean_irrelevant,
        correct,
        total,
    }
}

pub struct DecisionProbeService {
    ctx: Arc<crate::application::context::WorkspaceContext>,
}

impl DecisionProbeService {
    pub fn new(ctx: Arc<crate::application::context::WorkspaceContext>) -> Self {
        Self { ctx }
    }

    pub fn run(&self) -> Result<DecisionProbeReport> {
        let policy = self.ctx.settings().ai;
        if !policy.enabled {
            return Err(AppError::new(
                ErrorCode::AiDisabled,
                "AI 能力未开启：请先到「设置 → AI」打开总开关。",
            ));
        }
        let service = AiPolicyService::new(Arc::clone(&self.ctx));
        let Some(config) = service.decision_config() else {
            return Err(AppError::new(
                ErrorCode::AiDisabled,
                "决策模型未就绪：请在「设置 → AI → 决策模型」里选择 System One、填好地址与模型名、填写 Key，并确认一次授权。",
            ));
        };
        let model = SystemOneDecisionModel::new(&config);
        let provider = model.provider_id();
        let host = super::host::host_of(&config.base_url).unwrap_or_default();

        let state = probe_state();
        let questions = probe_questions();
        let started = Instant::now();

        let (first_value, first_tokens) = model.raw_request(&state, &questions)?;
        let (second_value, second_tokens) = model.raw_request(&state, &questions)?;
        let first = parse_probe_raw(&first_value, first_tokens);
        let second = parse_probe_raw(&second_value, second_tokens);
        let latency_ms = started.elapsed().as_millis() as u64;

        let mut items = Vec::new();
        let mut scored = 0usize;
        let mut correct = 0usize;
        for spec in all_items() {
            let Some(p_yes) = first.p_yes(spec.name) else {
                continue;
            };
            items.push(DecisionProbeItem {
                name: spec.name.to_string(),
                question: spec.question.to_string(),
                expected_yes: spec.expected_yes,
                p_yes,
            });
            if let Some(expected) = spec.expected_yes {
                scored += 1;
                if (p_yes > 0.5) == expected {
                    correct += 1;
                }
            }
        }
        let accuracy = (scored > 0).then(|| correct as f32 / scored as f32);
        // 校准：只用手上有标准答案的题，且**分成两族**——
        // 混在一起算会被"题太清晰"那一族拉平，看不出"材料里没有答案时会不会装懂"。
        let to_sample = |item: &DecisionProbeItem| item.expected_yes.map(|truth| (item.p_yes, truth));
        let samples: Vec<crate::application::ai::calibration::Sample> =
            items.iter().filter_map(to_sample).collect();
        let determinable: Vec<crate::application::ai::calibration::Sample> = items
            .iter()
            .filter(|item| !is_underdetermined(&item.name))
            .filter_map(to_sample)
            .collect();
        let underdetermined: Vec<crate::application::ai::calibration::Sample> = items
            .iter()
            .filter(|item| is_underdetermined(&item.name))
            .filter_map(to_sample)
            .collect();
        let calibration = crate::application::ai::calibration::evaluate(&samples);
        let calibration_determinable =
            crate::application::ai::calibration::evaluate(&determinable);
        let calibration_underdetermined =
            crate::application::ai::calibration::evaluate(&underdetermined);
        let mean_stated_underdetermined = if underdetermined.is_empty() {
            None
        } else {
            Some(
                underdetermined.iter().map(|(p, _)| *p).sum::<f32>()
                    / underdetermined.len() as f32,
            )
        };
        let out_of_sample_ece = crate::application::ai::calibration::out_of_sample_ece(&samples);
        let coherence = match (first.p_yes("coherence_pos"), first.p_yes("coherence_neg")) {
            (Some(left), Some(right)) => Some(left + right),
            _ => None,
        };
        // 确定性：把两次的答案序列化成同样的形状再比
        let deterministic = first.canonical() == second.canonical();
        let input_tokens = first.input_tokens + second.input_tokens;
        let unit = if config.base_url.to_ascii_lowercase().contains("typesafe") {
            JEV_USD_PER_TOKEN
        } else {
            PERPLEXITY_USD_PER_TOKEN
        };

        let note = format!(
            "ECE 是**期望校准误差**（公开研究用的线是 0.05），按 10 等宽箱算；\
             折外（outOfSampleEce）是「一半拟合、一半评估」——同一批数据既拟合又评估会得出虚假的好数字。\
             题库分两族：**材料里能推出来**的（determinable）与**材料里没有答案、只有我们知道的私事**的\
             （underdetermined）。后者才是测校准的关键——理想情况下模型应当给 ~0.5；\
             它若明显偏离 0.5，就是「不懂装懂」。共 {scored} 题仍属**小样本**\
             （公开研究用了 799 题），足够发现明显问题、不足以定校准结论。"
        );
        // —— A/B：同一批打分任务上，规则档 vs 决策档（D4 收尾）——
        let rule = crate::application::ai::rule_provider::RuleDecisionProvider::default();
        let rule_outcome = evaluate_ab(&rule, AB_CASES);
        let decision_outcome = evaluate_ab(&model, AB_CASES);
        let decision_better = decision_outcome.accuracy > rule_outcome.accuracy
            || (decision_outcome.accuracy == rule_outcome.accuracy
                && decision_outcome.separation > rule_outcome.separation);
        let ab_note = format!(
            "同一批 {} 条「查询 + 候选」打分任务（相关/不相关各半，人工标注）：\
             规则档 {}/{}（区分度 {:.3}）、决策档 {}/{}（区分度 {:.3}）→ {}。\
             注意：这是**打分**任务的对比（是非题规则档答不了）；\
             决策档要联网、按输入 token 计费，规则档零成本、永远可用。",
            AB_CASES.len(),
            rule_outcome.correct,
            rule_outcome.total,
            rule_outcome.separation,
            decision_outcome.correct,
            decision_outcome.total,
            decision_outcome.separation,
            if decision_better { "决策档更好" } else { "规则档不落下风" }
        );
        let ab = AbReport {
            cases: AB_CASES.len(),
            rule: rule_outcome,
            decision: decision_outcome,
            decision_better,
            note: ab_note,
        };
        Ok(DecisionProbeReport {
            provider,
            model: config.model.clone(),
            host,
            items,
            accuracy,
            scored_items: scored,
            calibration,
            calibration_determinable,
            calibration_underdetermined,
            mean_stated_underdetermined,
            out_of_sample_ece,
            coherence,
            deterministic,
            conflict_pair_yes: first.p_yes("conflict_pair"),
            non_conflict_pair_yes: first.p_yes("non_conflict_pair"),
            calls: 2,
            input_tokens,
            latency_ms,
            estimated_usd: input_tokens as f64 * unit,
            note,
            ab,
        })
    }
}

/// 探针题目（**我们自己的中文题**，覆盖公开研究点名的弱项）。
///
/// 公开给测试用：真机评测要能拿同一份题库去跑，否则"服务里跑的题"和"评测跑的题"
/// 会变成两套，数字就没法互相印证。
pub struct ProbeItemSpec {
    pub name: &'static str,
    pub question: &'static str,
    pub expected_yes: Option<bool>,
}


pub const DETERMINABLE_ITEMS: &[ProbeItemSpec] = &[
    // —— 算术 / 浮点（研究点名的弱项）——
    ProbeItemSpec { name: "math_sum", question: "2 + 2 = 4 是否成立？", expected_yes: Some(true) },
    ProbeItemSpec { name: "math_mul", question: "3 × 4 = 12 是否成立？", expected_yes: Some(true) },
    ProbeItemSpec { name: "math_sub", question: "10 − 3 = 7 是否成立？", expected_yes: Some(true) },
    ProbeItemSpec {
        name: "float_compare",
        question: "在数学上，9.11 大于 9.9，这个说法是否正确？",
        expected_yes: Some(false),
    },
    ProbeItemSpec {
        name: "float_compare_2",
        question: "在数学上，9.9 大于 9.11，这个说法是否正确？",
        expected_yes: Some(true),
    },
    // —— 计数 ——
    ProbeItemSpec {
        name: "count_material",
        question: "state 的 material 数组是否恰好包含 5 个元素？",
        expected_yes: Some(true),
    },
    ProbeItemSpec {
        name: "count_priced",
        question: "material 里是否恰好有 3 个元素给出了价格？",
        expected_yes: Some(true),
    },
    ProbeItemSpec {
        name: "count_images",
        question: "material 里是否有 0 个元素带图片？",
        expected_yes: Some(true),
    },
    ProbeItemSpec {
        name: "count_two_weeks",
        question: "material 中提到「两周前上线」的材料是否恰好有 1 条？",
        expected_yes: Some(true),
    },
    // —— 否定陷阱 ——
    ProbeItemSpec {
        name: "negation_price",
        question: "「材料 A 没有提到价格」这句话是否正确？",
        expected_yes: Some(false),
    },
    ProbeItemSpec {
        name: "negation_price_e",
        question: "材料 E 是否提供了价格信息？",
        expected_yes: Some(false),
    },
    ProbeItemSpec {
        name: "negation_double",
        question: "「材料 C 并没有明确反对照搬方案乙」这个说法是否成立？",
        expected_yes: Some(false),
    },
    ProbeItemSpec {
        name: "negation_docs",
        question: "材料 N 是否提到该方案的文档还需要补充？",
        expected_yes: Some(true),
    },
    // —— 时序与比较 ——
    ProbeItemSpec {
        name: "date_recall",
        question: "材料是否说明方案甲是两周前上线的？",
        expected_yes: Some(true),
    },
    ProbeItemSpec {
        name: "date_older",
        question: "方案乙是否比方案甲更早上线？",
        expected_yes: Some(true),
    },
    ProbeItemSpec {
        name: "price_higher",
        question: "方案乙的价格是否高于方案甲？",
        expected_yes: Some(true),
    },
    ProbeItemSpec {
        name: "price_cheapest",
        question: "在给出价格的方案里，方案丙是否是价格最低的？",
        expected_yes: Some(true),
    },
    // —— 立场识别 ——
    ProbeItemSpec {
        name: "stance_c",
        question: "材料 C 是否明确反对把方案乙照搬到生产环境？",
        expected_yes: Some(true),
    },
    ProbeItemSpec {
        name: "stance_x",
        question: "材料 X 是否推荐使用工作窃取队列？",
        expected_yes: Some(true),
    },
    ProbeItemSpec {
        name: "stance_y",
        question: "材料 Y 是否建议在生产环境采用工作窃取队列？",
        expected_yes: Some(false),
    },
    // —— 矛盾判别（真 / 假各两对）——
    ProbeItemSpec {
        name: "conflict_pair",
        question: "conflict_pair 里的两条材料是否互相矛盾（一个推荐、一个明确反对）？",
        expected_yes: Some(true),
    },
    ProbeItemSpec {
        name: "conflict_pair_2",
        question: "conflict_pair_2 里的两条材料是否互相矛盾？",
        expected_yes: Some(true),
    },
    ProbeItemSpec {
        name: "non_conflict_pair",
        question: "non_conflict_pair 里的两条材料是否互相矛盾？",
        expected_yes: Some(false),
    },
    ProbeItemSpec {
        name: "non_conflict_pair_2",
        question: "non_conflict_pair_2 里的两条材料是否互相矛盾？",
        expected_yes: Some(false),
    },
    // —— 信息不足时要如实说"不能确定" ——
    ProbeItemSpec {
        name: "unknown_price_d",
        question: "仅凭给定材料，能确定方案丁的价格吗？",
        expected_yes: Some(false),
    },
    ProbeItemSpec {
        name: "unknown_mars",
        question: "仅凭给定材料，能确定火星基地的选址吗？",
        expected_yes: Some(false),
    },
    ProbeItemSpec {
        name: "unknown_conclusion",
        question: "关于「哪个方案更好」，材料是否给出了明确结论？",
        expected_yes: Some(false),
    },
    // —— 第二批（2026-10-13 扩容）：同一个 state 上再问 13 道，把可判分题补到 40 ——
    ProbeItemSpec { name: "math_percent", question: "310 的 10% 等于 31，这个说法是否正确？", expected_yes: Some(true) },
    ProbeItemSpec { name: "math_compare", question: "90 小于 100，这个说法是否正确？", expected_yes: Some(true) },
    ProbeItemSpec { name: "math_even", question: "120 是偶数，这个说法是否正确？", expected_yes: Some(true) },
    ProbeItemSpec { name: "count_month", question: "material 中提到「一个月前」的材料是否恰好有 1 条？", expected_yes: Some(true) },
    ProbeItemSpec { name: "count_e_no_price", question: "material 里是否有一条 id 为 E、且没有给出价格的材料？", expected_yes: Some(true) },
    ProbeItemSpec { name: "date_newest", question: "在给出上线时间的方案里，方案丙是否是最晚上线的？", expected_yes: Some(true) },
    ProbeItemSpec { name: "price_sum", question: "三个给出价格的方案，价格之和是否大于 300 元？", expected_yes: Some(true) },
    ProbeItemSpec { name: "price_gap", question: "给出价格的方案里，最高价与最低价之差是否大于 20 元？", expected_yes: Some(true) },
    ProbeItemSpec { name: "stance_m", question: "材料 M 是否在描述实现要点？", expected_yes: Some(true) },
    ProbeItemSpec { name: "stance_n", question: "材料 N 是否提到了误用风险？", expected_yes: Some(true) },
    ProbeItemSpec { name: "conflict_opposite", question: "X 与 Y 是否对「是否推荐工作窃取队列」给出了相反结论？", expected_yes: Some(true) },
    ProbeItemSpec { name: "non_conflict_same", question: "M 与 N 是否在讲同一件事？", expected_yes: Some(false) },
    ProbeItemSpec { name: "negation_e_has_price", question: "材料 E 是否给出了价格？", expected_yes: Some(false) },
    // —— 自洽对（只观察，不判对错）——
    ProbeItemSpec {
        name: "coherence_pos",
        question: "根据材料判断：方案甲比方案乙更好吗？",
        expected_yes: None,
    },
    ProbeItemSpec {
        name: "coherence_neg",
        question: "根据材料判断：方案甲并不比方案乙更好吗？",
        expected_yes: None,
    },
];

/// **材料里没有答案**的题（只有提问者知道真相）。
///
/// 为什么必须单独一族：上面那些题材料里都能推出来，模型当然给极端概率——那样算出来的
/// ECE 只能证明"题太清晰"，**测不了校准**。这一族的真相来自评测环境（我们自己的私事），
/// 材料里没有、预训练里也没有；**校准的模型应当给 ~0.5，而不是凭空自信**。
/// 每件事写正反两问，因此真实正例率恰好是 50%（平衡），ECE 才有意义。
pub const UNDERDETERMINED_ITEMS: &[ProbeItemSpec] = &[
    ProbeItemSpec { name: "hidden_python_yes", question: "材料之外：这台评测机上 python3 的主版本号是 3.14 —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_python_no", question: "材料之外：这台评测机上 python3 的主版本号是 3.12 —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_proxy_yes", question: "材料之外：这台机器上的 HTTP 代理监听在 8118 端口 —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_proxy_no", question: "材料之外：这台机器上的 HTTP 代理监听在 9999 端口 —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_repo_yes", question: "材料之外：这个项目的根目录名是 PersonalKnowlge —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_repo_no", question: "材料之外：这个项目的根目录名是 PersonalKnowledgeBase —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_plan_yes", question: "材料之外：仓库的 docs 目录下有 PLAN_V1.3.0.md —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_plan_no", question: "材料之外：仓库的 docs 目录下有 PLAN_V9.9.md —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_pyobjc_yes", question: "材料之外：这台评测机上装了 Python 的 pyobjc（可用 Quartz）—— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_pyobjc_no", question: "材料之外：这台评测机上没有装 pyobjc（Quartz 不可用）—— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_version_yes", question: "材料之外：仓库 package.json 里的 version 是 1.3.6 —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_version_no", question: "材料之外：仓库 package.json 里的 version 是 0.9.0 —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_schema_yes", question: "材料之外：数据库迁移到的最新 schema 版本是 13 —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_schema_no", question: "材料之外：数据库迁移到的最新 schema 版本是 8 —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_battery_yes", question: "材料之外：这个决策探针的可判分题库有 100 道题 —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_battery_no", question: "材料之外：这个决策探针的可判分题库有 30 道题 —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_default_base_no", question: "材料之外：决策模型的默认端点是 api.typesafe.ai —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_default_base_yes", question: "材料之外：决策模型的默认端点是 api.perplexity.ai —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_migration_yes", question: "材料之外：仓库里存在 src-tauri/migrations/012_decision_cache.sql —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_migration_no", question: "材料之外：仓库里存在 src-tauri/migrations/099_missing.sql —— 对吗？", expected_yes: Some(false) },
    // —— 第二批（2026-10-13 扩容）：20 组"仓库里有没有这个东西"的事实，正反各一问，
    //    把材料外题补到 60 道（每箱从 1–5 条提到 6 条上下，ECE 才有精度）。
    //    每条真值都由 `underdetermined_truths_still_hold` 守卫测试对着仓库复核。
    ProbeItemSpec { name: "hidden_cargo_toml_yes", question: "材料之外：仓库里存在 src-tauri/Cargo.toml —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_cargo_toml_no", question: "材料之外：仓库里存在 src-tauri/Cargo.yml —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_app_tsx_yes", question: "材料之外：仓库里存在 src/app/App.tsx —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_app_tsx_no", question: "材料之外：仓库里存在 src/app/Main.tsx —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_readme_yes", question: "材料之外：仓库根目录存在 README.md —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_readme_no", question: "材料之外：仓库根目录存在 INSTALL.md —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_init_migration_yes", question: "材料之外：仓库里存在 src-tauri/migrations/001_init.sql —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_init_migration_no", question: "材料之外：仓库里存在 src-tauri/migrations/000_init.sql —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_run_sh_yes", question: "材料之外：仓库里存在 scripts/acceptance/run.sh —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_run_sh_no", question: "材料之外：仓库里存在 scripts/acceptance/start.sh —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_chunk_rs_yes", question: "材料之外：仓库里存在 src-tauri/src/core/chunk.rs —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_chunk_rs_no", question: "材料之外：仓库里存在 src-tauri/src/core/split.rs —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_ai_panel_yes", question: "材料之外：仓库里存在 src/features/settings/AiSettingsPanel.tsx —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_ai_panel_no", question: "材料之外：仓库里存在 src/features/settings/AiPanel.tsx —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_cases_js_yes", question: "材料之外：仓库里存在 scripts/acceptance/cases.js —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_cases_js_no", question: "材料之外：仓库里存在 scripts/acceptance/cases.json —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_release_yes", question: "材料之外：仓库里存在 .github/workflows/release.yml —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_release_no", question: "材料之外：仓库里存在 .github/workflows/deploy.yml —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_system_one_yes", question: "材料之外：仓库里存在 src-tauri/src/application/ai/system_one.rs —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_system_one_no", question: "材料之外：仓库里存在 src-tauri/src/application/ai/jev.rs —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_jev_doc_yes", question: "材料之外：仓库里存在 docs/JEV_CONTRACT.md —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_jev_doc_no", question: "材料之外：仓库里存在 docs/JEV_API.md —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_applog_yes", question: "材料之外：仓库里存在 src/lib/appLog.ts —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_applog_no", question: "材料之外：仓库里存在 src/lib/logger.ts —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_bounds_yes", question: "材料之外：仓库里存在 src/hooks/useWindowBounds.ts —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_bounds_no", question: "材料之外：仓库里存在 src/hooks/useWindowState.ts —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_prettier_yes", question: "材料之外：仓库根目录存在 .prettierrc.json —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_prettier_no", question: "材料之外：仓库根目录存在 .eslintrc —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_calibration_yes", question: "材料之外：仓库里存在 src-tauri/src/application/ai/calibration.rs —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_calibration_no", question: "材料之外：仓库里存在 src-tauri/src/application/ai/metrics.rs —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_provider_flow_yes", question: "材料之外：仓库里存在 src-tauri/tests/decision_provider_flow.rs —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_provider_flow_no", question: "材料之外：仓库里存在 src-tauri/tests/jev_flow.rs —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_screenshots_yes", question: "材料之外：仓库里存在 docs/screenshots 目录 —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_screenshots_no", question: "材料之外：仓库里存在 docs/images 目录 —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_ureq_yes", question: "材料之外：Cargo.toml 里 ureq 的版本是 3.4.2 —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_ureq_no", question: "材料之外：Cargo.toml 里 ureq 的版本是 2.10.0 —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_crate_yes", question: "材料之外：Cargo.toml 里这个 crate 的名字是 personal-knowledge-os —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_crate_no", question: "材料之外：Cargo.toml 里这个 crate 的名字是 personal-knowledge-ai —— 对吗？", expected_yes: Some(false) },
    ProbeItemSpec { name: "hidden_max_migration_yes", question: "材料之外：迁移目录里最大的编号是 013 —— 对吗？", expected_yes: Some(true) },
    ProbeItemSpec { name: "hidden_max_migration_no", question: "材料之外：迁移目录里最大的编号是 020 —— 对吗？", expected_yes: Some(false) },
];

/// 两族合起来（探针一次请求把全部题目发出去）。
pub fn all_items() -> Vec<&'static ProbeItemSpec> {
    DETERMINABLE_ITEMS.iter().chain(UNDERDETERMINED_ITEMS.iter()).collect()
}

/// 这道题是不是"材料里没有答案"那一族。
pub fn is_underdetermined(name: &str) -> bool {
    UNDERDETERMINED_ITEMS.iter().any(|item| item.name == name)
}

pub fn probe_state() -> serde_json::Value {
    serde_json::json!({
        "material": [
            { "id": "A", "text": "方案甲：价格 100 元，两周前上线，日志无异常。" },
            { "id": "B", "text": "方案乙：价格 120 元，一个月前上线，日志无异常。" },
            { "id": "C", "text": "运维建议：不建议把方案乙直接照搬到生产环境。" },
            { "id": "D", "text": "方案丙：价格 90 元，三天前上线。" },
            { "id": "E", "text": "方案丁：还在评估中，暂时没有定论。" }
        ],
        "conflict_pair": [
            { "id": "X", "text": "推荐使用工作窃取队列：它在高并发下表现优异，建议生产环境采用。" },
            { "id": "Y", "text": "不建议使用工作窃取队列：它在高并发下会拖垮运行时，生产环境不要照搬。" }
        ],
        "conflict_pair_2": [
            { "id": "P", "text": "Redis 缓存在我们的负载下把吞吐提升了一倍。" },
            { "id": "Q", "text": "Redis 缓存在我们的负载下把吞吐拉低了一半。" }
        ],
        "non_conflict_pair": [
            { "id": "M", "text": "实现要点在于任务唤醒与批量窃取。" },
            { "id": "N", "text": "这种方案存在误用风险，文档还需要补充。" }
        ],
        "non_conflict_pair_2": [
            { "id": "M2", "text": "该方案两周前上线。" },
            { "id": "N2", "text": "该方案的监控面板还没有做。" }
        ]
    })
}

pub fn probe_questions() -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for spec in all_items() {
        map.insert(
            spec.name.to_string(),
            serde_json::json!({
                "type": "noul",
                "instructions": spec.question,
            }),
        );
    }
    serde_json::Value::Object(map)
}

/// 探针只用到 `noul`：把原始答案抽成"P(是)"与用量，便于汇总。
pub(crate) struct ProbeRaw {
    answers: std::collections::HashMap<String, f32>,
    pub input_tokens: i64,
}

impl ProbeRaw {
    pub fn p_yes(&self, name: &str) -> Option<f32> {
        self.answers.get(name).copied()
    }

    /// 用于确定性比较的规范化字符串。
    pub fn canonical(&self) -> String {
        let mut keys: Vec<&String> = self.answers.keys().collect();
        keys.sort();
        keys.iter()
            .map(|key| format!("{key}={:.9}", self.answers[*key]))
            .collect::<Vec<_>>()
            .join("|")
    }
}

pub(crate) fn parse_probe_raw(value: &serde_json::Value, input_tokens: i64) -> ProbeRaw {
    let mut answers = std::collections::HashMap::new();
    if let Some(map) = value.get("answers").and_then(|answers| answers.as_object()) {
        for (name, answer) in map {
            if let Some(noul) = answer.get("noul").and_then(|value| value.as_f64()) {
                answers.insert(name.clone(), noul as f32);
            }
        }
    }
    ProbeRaw { answers, input_tokens }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ai::traits::{
        DecisionChoice, DecisionKind, DecisionModel, DecisionResult, DecisionState,
    };
    use crate::core::error::Result;

    /// 假模型：按"标题里有没有查询词"决定打分——用来钉住 A/B 的算法本身。
    struct KeywordModel;

    impl DecisionModel for KeywordModel {
        fn id(&self) -> &'static str {
            "fake.keyword"
        }
        fn is_local(&self) -> bool {
            true
        }
        fn decide(&self, kind: DecisionKind, state: &DecisionState) -> Result<Vec<DecisionResult>> {
            Ok(state
                .candidates
                .iter()
                .map(|candidate| {
                    let hit = candidate.title.contains(&state.query);
                    DecisionResult {
                        note_id: candidate.note_id.clone(),
                        choice: if hit { DecisionChoice::Use } else { DecisionChoice::Drop },
                        score: if hit { 0.9 } else { 0.1 },
                        confidence: 0.9,
                        reason: "假模型".to_string(),
                        kind,
                    }
                })
                .collect())
        }
    }

    #[test]
    fn ab_counts_accuracy_and_separation_on_our_own_cases() {
        let outcome = evaluate_ab(&KeywordModel, AB_CASES);
        assert_eq!(outcome.total, AB_CASES.len());
        assert_eq!(outcome.provider, "fake.keyword");
        // 假模型只看字面命中：改写题（同义但无原词）必然漏、陷阱题（含原词但无知识）必然中。
        // 于是它的准确率低于基线、区分度**为负**——这正是这套用例的设计目的：
        // 能区分"字面命中"与"真相关"。真机实测里规则档也是 7/16、区分度 −0.12。
        assert!(
            outcome.accuracy < 0.6,
            "只看字面命中的模型不该在这套用例上及格：{outcome:?}"
        );
        assert!(
            outcome.separation < 0.0,
            "字面匹配会被陷阱题带偏，区分度应当为负：{outcome:?}"
        );
    }

    #[test]
    fn ab_case_set_is_balanced_and_covers_both_traps() {
        let relevant = AB_CASES.iter().filter(|case| case.relevant).count();
        assert_eq!(relevant * 2, AB_CASES.len(), "相关/不相关必须各半（否则准确率没有基线）");
        // 每个查询都有 4 条候选（1 字面相关 + 1 改写相关 + 1 明显无关 + 1 同词陷阱）
        let mut queries: Vec<&str> = AB_CASES.iter().map(|case| case.query).collect();
        queries.sort_unstable();
        queries.dedup();
        assert_eq!(queries.len(), 4);
        for query in queries {
            assert_eq!(
                AB_CASES.iter().filter(|case| case.query == query).count(),
                4,
                "{query} 的候选数不是 4"
            );
        }
    }

    /// 守卫：**材料外题的真值必须仍然成立**。
    ///
    /// 这些题说的是仓库/环境的现状；仓库一变（改名、加迁移、换依赖版本）它们就会变成"错题"，
    /// 而错题会悄悄污染校准统计。所以这里对着仓库逐条复核，漂移就当场失败。
    ///
    /// 注意：有几条是**本机环境**事实（python 主版本 3.14、代理端口 8118、未装 pyobjc），
    /// 换机器需要重新核对——这一条写在注释里，不放进自动守卫。
    #[test]
    fn underdetermined_truths_still_hold() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("仓库根")
            .to_path_buf();
        let exists = |relative: &str| root.join(relative).exists();

        // 必须存在 / 必须不存在（与题库里的正反两问一一对应）
        let file_facts: &[(&str, bool)] = &[
            ("src-tauri/Cargo.toml", true),
            ("src-tauri/Cargo.yml", false),
            ("src/app/App.tsx", true),
            ("src/app/Main.tsx", false),
            ("README.md", true),
            ("INSTALL.md", false),
            ("src-tauri/migrations/001_init.sql", true),
            ("src-tauri/migrations/000_init.sql", false),
            ("scripts/acceptance/run.sh", true),
            ("scripts/acceptance/start.sh", false),
            ("src-tauri/src/core/chunk.rs", true),
            ("src-tauri/src/core/split.rs", false),
            ("src/features/settings/AiSettingsPanel.tsx", true),
            ("src/features/settings/AiPanel.tsx", false),
            ("scripts/acceptance/cases.js", true),
            ("scripts/acceptance/cases.json", false),
            (".github/workflows/release.yml", true),
            (".github/workflows/deploy.yml", false),
            ("src-tauri/src/application/ai/system_one.rs", true),
            ("src-tauri/src/application/ai/jev.rs", false),
            ("docs/JEV_CONTRACT.md", true),
            ("docs/JEV_API.md", false),
            ("src/lib/appLog.ts", true),
            ("src/lib/logger.ts", false),
            ("src/hooks/useWindowBounds.ts", true),
            ("src/hooks/useWindowState.ts", false),
            (".prettierrc.json", true),
            (".eslintrc", false),
            ("src-tauri/src/application/ai/calibration.rs", true),
            ("src-tauri/src/application/ai/metrics.rs", false),
            ("src-tauri/tests/decision_provider_flow.rs", true),
            ("src-tauri/tests/jev_flow.rs", false),
            ("docs/screenshots", true),
            ("docs/images", false),
            ("docs/PLAN_V1.3.0.md", true),
            ("src-tauri/migrations/012_decision_cache.sql", true),
        ];
        for (relative, expected) in file_facts {
            assert_eq!(exists(relative), *expected, "材料外题的真值已漂移：{relative}");
        }

        // 配置类事实
        let cargo = std::fs::read_to_string(root.join("src-tauri/Cargo.toml")).expect("读 Cargo.toml");
        assert!(cargo.contains("name = \"personal-knowledge-os\""), "crate 名变了");
        assert!(cargo.contains("ureq = { version = \"3.4.2\""), "ureq 版本变了");
        // 版本号是"会随发布变"的事实：这里不写死某个值，而是**拿题库里的题面去对**——
        // 升版之后如果没人改题，模型答对反而会被记成答错（校准数据就脏了）。
        // 这条断言会直接点名该改哪两条题（`hidden_version_yes` / `hidden_version_no`）。
        let package = std::fs::read_to_string(root.join("package.json")).expect("读 package.json");
        let version = package
            .lines()
            .find_map(|line| line.trim().strip_prefix("\"version\":"))
            .map(|rest| rest.trim().trim_matches([',', '"']).to_string())
            .expect("package.json 里没有 version");
        let version_yes = UNDERDETERMINED_ITEMS
            .iter()
            .find(|item| item.name == "hidden_version_yes")
            .expect("题库里应有 hidden_version_yes");
        assert!(
            version_yes.question.contains(&version),
            "package.json 的版本是 {version}，但题库题面写的不是它（改 hidden_version_yes 的题面）：{}",
            version_yes.question
        );
        let version_no = UNDERDETERMINED_ITEMS
            .iter()
            .find(|item| item.name == "hidden_version_no")
            .expect("题库里应有 hidden_version_no");
        assert!(
            !version_no.question.contains(&version),
            "hidden_version_no 的题面不该出现**真实**版本号（否则两个问题都成了「对」）"
        );
        let migrations = std::fs::read_dir(root.join("src-tauri/migrations")).expect("读迁移目录");
        let max = migrations
            .flatten()
            .filter_map(|entry| {
                entry.file_name().to_string_lossy().chars().take(3).collect::<String>().parse::<u32>().ok()
            })
            .max()
            .unwrap_or(0);
        assert_eq!(max, 13, "迁移目录的最大编号变了");
        let db = std::fs::read_to_string(
            root.join("src-tauri/src/infrastructure/db/migrations.rs"),
        )
        .expect("读 migrations.rs");
        assert!(db.contains("SCHEMA_VERSION: i64 = 13"), "SCHEMA_VERSION 变了");
        let policy =
            std::fs::read_to_string(root.join("src-tauri/src/application/ai/policy.rs")).expect("读 policy.rs");
        assert!(policy.contains("api.perplexity.ai"), "决策档默认端点变了");

        // 自洽：题库规模的那条事实必须与数组实际长度一致
        let scored = DETERMINABLE_ITEMS.iter().filter(|item| item.expected_yes.is_some()).count()
            + UNDERDETERMINED_ITEMS.iter().filter(|item| item.expected_yes.is_some()).count();
        assert_eq!(scored, 100, "「可判分题库有 100 道题」这条事实与题库规模不一致");
    }
}

