//! 研究助手服务（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §9.1–§9.6、§9.9）。
//!
//! 五步固定流程（每步可单独调用、可中断、可审计）：
//! ① `start`   立项：把课题拆成 3–5 个子问题，先查本地知识库作为「已知基线」
//! ② `collect` 收集：本地优先（FTS5）；外部检索在 D3 接入前**如实告知未接入**，不伪造材料
//! ③ `verify`  验证：按子问题汇总，≥2 个**独立**来源才算交叉验证；单一来源只进「待验证清单」
//! ④ `compose` 汇总：固定六段报告（结论先行 / 关键事实 / 争议 / 与本库关系 / 待验证 / 参考来源）
//! ⑤ `proposal` 写入提案：默认落 Inbox、同标题不覆盖、写前 diff；**落库要用户确认**（D2 的 write）
//!
//! 降级路径（§9.6）：没有 Jev 用规则表（`source_grade`），没有 LLM 用模板实现
//! （`TemplateGenerativeModel`）——推测段永远标注「未接模型」，不冒充模型结论。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use crate::application::ai::policy::{AiPolicyService, HostVerdict};
use crate::application::ai::source_grade::{self, grade_local_note, Grade};
use crate::application::ai::template_provider::TemplateGenerativeModel;
use crate::application::ai::tools::AiTools;
use crate::application::ai::rule_provider::RuleDecisionProvider;
use crate::application::ai::local_model::OllamaModel;
use crate::application::ai::decision_engine::DecisionEngine;
use crate::application::ai::remote_model::RemoteModel;
use crate::application::ai::system_one::SystemOneDecisionModel;
use crate::application::ai::traits::{
    DecisionCandidate, DecisionKind, DecisionModel, DecisionState,
    GenerationRequest, GenerativeModel, RetrievalOrigin, RetrievedChunk,
};
use super::web_search;
use crate::application::context::WorkspaceContext;
use crate::application::note_service::NoteService;
use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::events::events as event_names;
use crate::core::ids::ulid;
use crate::core::text::first_sentence;
use crate::core::time::{date_offset_days, now_iso, today_date};
use crate::domain::ai::{DecisionTrace, NetworkMode};
use crate::domain::note::{CreateNoteInput, NoteListQuery, SaveNoteInput, SetSourceInput};
use crate::domain::research::{
    DuplicateNote, KbBaseline, KbRelation, ResearchClaim, ResearchPlan, ResearchProposal,
    ResearchReport, ResearchRequest, ResearchSession, ResearchSessionSummary, ResearchSource,
    ResearchStatus, ResearchStepLog, ResearchWarning, SourceLevel, SourceOrigin, SubQuestion,
    WarningKind, ResearchWriteInput, ResearchWriteResult, WRITE_MODE_ABANDON, WRITE_MODE_APPEND,
    WRITE_MODE_CREATE, DroppedSource, ResearchOutboundPreview, ResearchOutboundSnippet,
};
use super::host::host_of;

/// 每个子问题最多采纳的本地材料条数。
const HITS_PER_SUB_QUESTION: usize = 6;
/// 一次会话最多采纳的来源总数。
const MAX_SOURCES: usize = 24;
/// 默认时间窗（天）：约两年。
const DEFAULT_WINDOW_DAYS: u32 = 730;
/// 报告的复核建议周期（天）→ `valid_until`。
const REVIEW_PERIOD_DAYS: i64 = 180;
/// 结论段最多几条。
const MAX_CONCLUSIONS: usize = 5;
/// 关键事实最多几条。
const MAX_KEY_FACTS: usize = 8;

/// 子问题的「意图标记」：用于把检索命中的材料**按规则**归属到子问题。
///
/// 这是 D1 的降级实现（没有模型做语义归并）；顺序与 `decompose` 的模板一一对应。
const INTENT_MARKERS: [&[&str]; 5] = [
    &["定义", "概念", "是什么", "概述", "介绍", "原理"],
    &["方案", "做法", "实现", "对比", "比较", "选型", "用法", "实践"],
    &["场景", "适用", "限制", "注意", "前提", "边界", "约束", "条件"],
    &["风险", "争议", "问题", "失败", "坑", "缺陷", "缺点", "事故"],
    &["进展", "新版", "更新", "发布", "最近", "趋势", "未来"],
];

/// 过于宽泛、必须先追问范围的课题。
const BROAD_TOPICS: &[&str] = &[
    "ai",
    "人工智能",
    "机器学习",
    "深度学习",
    "大模型",
    "编程",
    "技术",
    "笔记",
    "知识库",
    "知识管理",
    "工作",
    "生活",
    "学习",
    "投资",
    "写作",
];

pub struct ResearchService {
    ctx: Arc<WorkspaceContext>,
    /// 打分用的决策模型（§9.6：默认规则档；Jev / 本地模型接入时只换这一处）
    decision: Arc<dyn DecisionModel>,
}

impl ResearchService {
    pub fn new(ctx: Arc<WorkspaceContext>) -> Self {
        Self { ctx, decision: Arc::new(RuleDecisionProvider::default()) }
    }

    /// 注入自定义决策模型（用于 A/B：规则档 vs Jev / 本地模型）。
    pub fn with_decision_model(
        ctx: Arc<WorkspaceContext>,
        decision: Arc<dyn DecisionModel>,
    ) -> Self {
        Self { ctx, decision }
    }

    fn policy(&self) -> AiPolicyService {
        AiPolicyService::new(Arc::clone(&self.ctx))
    }

    fn tools(&self) -> AiTools {
        AiTools::new(Arc::clone(&self.ctx))
    }

    /// 推送研究进度（§9.2：每一步、每个抓取页都要让界面看到）。
    ///
    /// **只发元数据**：会话 id / 步骤 / 人读说明 / 已抓页数 / 已采纳来源数——
    /// 不含笔记正文、不含网页内容（与审计同一条隐私口径）。
    fn emit_progress(&self, session_id: &str, step: &str, detail: &str, pages: usize, sources: usize) {
        self.ctx.events.emit(
            event_names::RESEARCH_PROGRESS,
            serde_json::json!({
                "sessionId": session_id,
                "step": step,
                "detail": detail,
                "pagesFetched": pages,
                "sources": sources,
            }),
        );
    }

    /// 推送「推测段」的生成增量（Track C1 流式）。
    ///
    /// `text` 传的是**当前这一档的累计文本**（不是增量），界面直接替换即可：
    /// 新的一档从空串开始，于是上一档的半截内容会被自然清掉；这一档失败时
    /// 显式发一次空串 + `failed`，保证**界面上不留下半截内容**。
    /// 这些只是预览——最终以命令返回的报告（`report.speculation`）为准。
    fn emit_compose_delta(&self, session_id: &str, provider_id: &str, text: &str, state: &str) {
        self.ctx.events.emit(
            event_names::RESEARCH_COMPOSE_DELTA,
            serde_json::json!({
                "sessionId": session_id,
                "providerId": provider_id,
                "text": text,
                "state": state,
            }),
        );
    }

    /// 用户是否已经**在长步骤运行期间**点了「中止」。
    ///
    /// 中止与正在跑的步骤是并发的（中止按钮不因忙碌而禁用），因此每一步都要以
    /// **磁盘上的状态**为准：否则运行中的步骤会用自己那份内存快照把中止覆盖回去，
    /// 用户看到会话又变回「已收集」，联网抓取也照旧跑完预算。
    fn aborted_snapshot(&self, session_id: &str) -> Result<Option<ResearchSession>> {
        let persisted = self.load(session_id)?;
        Ok((persisted.status == ResearchStatus::Aborted).then_some(persisted))
    }

    /// 收集中途被中止：把**已经拿到的材料**并回那份「已中止」的会话（草稿不丢），
    /// 状态与中止原因保持磁盘上的值。
    fn finish_aborted_collect(
        &self,
        session_id: &str,
        sources: Vec<ResearchSource>,
        decision_provider: Option<String>,
        web_fetched: usize,
    ) -> Result<ResearchSession> {
        let mut persisted = self.load(session_id)?;
        persisted.sources = sources;
        if persisted.decision_provider.is_none() {
            persisted.decision_provider = decision_provider;
        }
        if let Some(plan) = persisted.plan.as_mut() {
            let counts: std::collections::HashMap<String, usize> =
                persisted.sources.iter().fold(std::collections::HashMap::new(), |mut acc, source| {
                    *acc.entry(source.sub_question_id.clone()).or_insert(0) += 1;
                    acc
                });
            for question in plan.sub_questions.iter_mut() {
                question.local_hits = counts.get(&question.id).copied().unwrap_or(0);
            }
        }
        persisted.updated_at = now_iso();
        persisted.steps.push(ResearchStepLog {
            step: "collect".to_string(),
            at: now_iso(),
            detail: format!(
                "收集中途被用户中止：已得材料 {} 条保留为草稿",
                persisted.sources.len()
            ),
        });
        self.save(&persisted)?;
        self.emit_progress(
            session_id,
            "collect",
            "收集中途被中止：已得材料保留为草稿",
            web_fetched,
            persisted.sources.len(),
        );
        Ok(persisted)
    }

    /// 长步骤收尾时，若用户在这期间点了中止：把**这一步的成果**并回那份「已中止」的会话。
    ///
    /// 状态与中止原因保持磁盘上的值（中止优先），成果按 `apply` 合并——这样既不会
    /// 「中止被覆盖」，也不会白跑一步（例如汇总出的报告仍留作草稿）。
    fn finish_aborted_step(
        &self,
        session_id: &str,
        detail: &str,
        apply: impl FnOnce(&mut ResearchSession),
    ) -> Result<ResearchSession> {
        let mut persisted = self.load(session_id)?;
        apply(&mut persisted);
        persisted.updated_at = now_iso();
        persisted.steps.push(ResearchStepLog {
            step: "abort".to_string(),
            at: now_iso(),
            detail: detail.to_string(),
        });
        self.save(&persisted)?;
        self.emit_progress(session_id, "abort", detail, 0, persisted.sources.len());
        Ok(persisted)
    }

    /// 本次打分可用的决策模型，**按优先级排队**（§9.6 的三档降级链）：
    ///
    /// ① 在线模型——前提是这一侧开关打开、配置完整、并且**过了授权闸门**
    ///    （没授权就根本不构造它，笔记片段也就永远不会离开本机）；
    /// ② 本地模型——装了 Ollama 且开关打开；
    /// ③ 注入的默认模型（规则档，永远可用）。
    ///
    /// 「本地机器跑不动模型」是常态（应用是本地软件、跨平台部署），所以在线这一档必须存在；
    /// 但它**永远排在闸门之后**，不是自动生效的。
    fn model_decision_candidates(&self, confirm_outbound: bool) -> Vec<Arc<dyn DecisionModel>> {
        let policy = self.ctx.settings().ai;
        let service = self.policy();
        let mut candidates: Vec<Arc<dyn DecisionModel>> = Vec::new();
        // ① 决策档（D4：Jev / Perplexity Decisions）——**排在最前**：
        //    它是专用决策模型（类型化输出 + 校准置信度），配好了就该它先判。
        //    与其它出站档同一套闸门：没配好/没授权就不构造；开了逐次确认则每次都要确认。
        if let Some(config) = service.decision_config() {
            if service.outbound_confirmed(confirm_outbound) {
                candidates.push(Arc::new(SystemOneDecisionModel::new(&config)));
            } else {
                service.audit("research_score", false, "denied", 0, Some("OUTBOUND_UNCONFIRMED"));
            }
        }
        if policy.remote_model_score && service.ensure_remote_model_allowed("research_score").is_ok() {
            if service.outbound_confirmed(confirm_outbound) {
                if let Some((base, model, key)) = service.remote_model() {
                    candidates.push(Arc::new(
                        RemoteModel::new(&base, &model, &key).with_proxy(service.outbound_proxy()),
                    ));
                }
            } else {
                // Track C2：逐次确认未通过 —— 不构造在线档，**一个请求都不会发**
                service.audit("research_score", false, "denied", 0, Some("OUTBOUND_UNCONFIRMED"));
            }
        }
        if policy.local_model_score {
            // 走服务层：读取时再校验一次地址（只允许本机），不只依赖写入时的规范化
            if let Some((endpoint, model)) = service.local_model() {
                candidates.push(Arc::new(OllamaModel::new(&endpoint, &model)));
            }
        }
        candidates
    }

    /// 打分用的候选链：模型档（在线 / 本地）+ **规则档兜底**（永远可用）。
    fn decision_candidates(&self, confirm_outbound: bool) -> Vec<Arc<dyn DecisionModel>> {
        let mut candidates = self.model_decision_candidates(confirm_outbound);
        candidates.push(Arc::clone(&self.decision));
        candidates
    }

    /// 语义档判冲突（Track C5）：先试模型档（在线 / 本地），全部不可用就**回退规则档**（极性词）。
    ///
    /// 返回 `(是否存在冲突, 实际生效的档位)`——档位会写进报告，便于事后核对到底是"模型判的"
    /// 还是"规则判的"。
    fn judge_conflict(
        &self,
        question: &SubQuestion,
        evidence: &[&ResearchSource],
        confirm_outbound: bool,
    ) -> (bool, String) {
        // 一条材料谈不上"互相矛盾"：直接走规则档（它同样要求 ≥2 条），省一次调用
        if evidence.len() >= 2 {
            let candidates: Vec<DecisionCandidate> = evidence
                .iter()
                .enumerate()
                .map(|(index, source)| DecisionCandidate {
                    note_id: format!("s{}", index + 1),
                    title: source.title.clone(),
                    snippet: source.snippet.clone(),
                    updated_at: source.published_at.clone().unwrap_or_default(),
                    source_reliability: Some(source.reliability),
                    // 来源的时效是"可能过时"标记，不是有效期末尾：这里不臆造，留空
                    valid_until: None,
                })
                .collect();
            for model in self.model_decision_candidates(confirm_outbound) {
                let provider = model.id().to_string();
                match model.judge_conflict(&question.question, &candidates) {
                    Ok(Some(verdict)) => {
                        tracing::debug!(
                            target: "ai",
                            "冲突判定（{provider}）：{} — {}",
                            verdict.conflict,
                            verdict.reason
                        );
                        return (verdict.conflict, provider);
                    }
                    // 这一档不做语义判断（例如规则实现）：换下一档
                    Ok(None) => continue,
                    Err(error) => {
                        tracing::debug!(target: "ai", "冲突判定失败（{provider}）：{}", error.message);
                        continue;
                    }
                }
            }
        }
        (has_polarity_conflict(evidence), "rule".to_string())
    }

    /// 生成侧可用的模型，同样**按优先级排队**：在线（已授权）→ 本地 → 模板实现（调用方兜底）。
    ///
    /// 与打分侧同一套闸门口径：在线这一档没配好或没授权时**根本不构造**，
    /// 所以「没有授权 → 一个字节都不会外发」是靠结构保证的，不是靠调用时记得判断。
    /// 生成档候选链：`(完整 provider 标识, 实现)`。
    ///
    /// 携带完整标识（含模型名）是刻意的：流式事件、降级提示、报告 `composer` 必须
    /// 说同一个名字，否则界面会出现"本地写一半变成 local.ollama，最后又变成 local.ollama:xxx"。
    fn composer_candidates(&self, confirm_outbound: bool) -> Vec<(String, Arc<dyn GenerativeModel>)> {
        let policy = self.ctx.settings().ai;
        let service = self.policy();
        let mut candidates: Vec<(String, Arc<dyn GenerativeModel>)> = Vec::new();
        if policy.remote_model_compose
            && service.ensure_remote_model_allowed("research_compose").is_ok()
        {
            if service.outbound_confirmed(confirm_outbound) {
                if let Some((base, model, key)) = service.remote_model() {
                    candidates.push((
                        RemoteModel::provider_id(&model),
                        Arc::new(
                            RemoteModel::new(&base, &model, &key)
                                .with_proxy(service.outbound_proxy()),
                        ),
                    ));
                }
            } else {
                // Track C2：逐次确认未通过 —— 不构造在线档，**一个请求都不会发**
                service.audit("research_compose", false, "denied", 0, Some("OUTBOUND_UNCONFIRMED"));
            }
        }
        if policy.local_model_compose {
            if let Some((endpoint, model)) = service.local_model() {
                candidates.push((
                    OllamaModel::provider_id(&model),
                    Arc::new(OllamaModel::new(&endpoint, &model)),
                ));
            }
        }
        candidates
    }

    /// 用决策层给来源打可信度分（一次调用，批量打分）。
    ///
    /// 返回**实际生效的** provider 标识：排在前面的一档出错（没装模型 / 超时 / 输出不合规）就
    /// 自动降到下一档，并把降级原因写进审计与报告，绝不让研究流程卡住。
    fn score_sources(
        &self,
        topic: &str,
        sources: &mut [ResearchSource],
        confirm_outbound: bool,
    ) -> (String, Option<String>, Option<DecisionTrace>) {
        if sources.is_empty() {
            return (self.decision.id().to_string(), None, None);
        }
        let key_of = |source: &ResearchSource| -> String {
            source
                .note_id
                .clone()
                .or_else(|| source.url.clone())
                .unwrap_or_else(|| format!("ref{}", source.reference))
        };
        let state = DecisionState {
            query: topic.to_string(),
            candidates: sources
                .iter()
                .map(|source| DecisionCandidate {
                    note_id: key_of(source),
                    title: source.title.clone(),
                    snippet: source.snippet.clone(),
                    updated_at: source.fetched_at.clone(),
                    source_reliability: Some(source.reliability),
                    valid_until: None,
                })
                .collect(),
        };
        let candidates = self.decision_candidates(confirm_outbound);
        // D2/N6：打分统一走**决策引擎** —— 先查缓存、再逐档试，并留下可解释 Trace
        let engine = DecisionEngine::new();
        let outcome = engine.decide(&self.ctx.db, DecisionKind::Relevance, &state, &candidates);
        let (provider, scores, trace) = match outcome {
            Ok(outcome) => {
                let provider = if outcome.provider.is_empty() {
                    self.decision.id().to_string()
                } else {
                    outcome.provider.clone()
                };
                let scores = if outcome.results.is_empty() {
                    None
                } else {
                    Some(outcome.results)
                };
                (provider, scores, Some(outcome.trace))
            }
            Err(error) => {
                tracing::debug!(target: "ai", "决策引擎失败，按「没有模型分」处理：{}", error.message);
                (self.decision.id().to_string(), None, None)
            }
        };
        // 降级提示：从 Trace 里取第一条失败（文案与之前保持一致）
        let fallback_note = trace.as_ref().and_then(|trace| {
            trace
                .attempts
                .iter()
                .find(|attempt| !attempt.ok)
                .map(|failed| {
                    format!("{} 打分失败（{}），已退回 {provider}", failed.provider, failed.detail)
                })
        });
        for source in sources.iter_mut() {
            let key = key_of(source);
            let model_score = scores.as_ref().and_then(|results| {
                results
                    .iter()
                    .find(|result| result.note_id == key)
                    .map(|result| result.score)
            });
            source.credibility = blended_credibility(model_score, source.reliability);
        }
        (provider, fallback_note, trace)
    }

    // ---------------------------------------------------------------- ① 立项

    /// 立项：校验课题 → 拆子问题 → 查本地基线 → 落盘。
    pub fn start(&self, request: ResearchRequest) -> Result<ResearchSession> {
        let started = Instant::now();
        let policy = self.policy().ensure_enabled("research_start")?;
        let topic = validate_topic(&request.topic)?;
        let budget = policy.budget.clamped();
        let time_window_days = request
            .time_window_days
            .unwrap_or(DEFAULT_WINDOW_DAYS)
            .clamp(7, 3650);

        let sub_questions = decompose(&topic, budget.max_subquestions as usize);
        let baseline = self.tools().kb_search(&topic, 3)?;
        let local_baseline: Vec<KbBaseline> = baseline
            .into_iter()
            .map(|hit| KbBaseline {
                note_id: hit.note_id,
                title: hit.title,
                snippet: hit.snippet,
            })
            .collect();

        // D5/N9：一次检索只发一条查询太容易漏——把课题扩成一组查询（第一条仍是课题原文）
        let expanded = crate::application::ai::query_expansion::expand(
            &topic,
            &sub_questions,
            crate::application::ai::query_expansion::MAX_QUERIES,
        );
        let queries: Vec<String> = expanded.iter().map(|item| item.text.clone()).collect();

        let mut notes = vec![
            "本次只用本地知识库材料（外部检索属于阶段 D3，接入前不会发起任何请求）。".to_string(),
            format!(
                "子问题 {max} 个 · 时间窗 {days} 天 · 抓取页上限 {}（外部检索启用后生效）。",
                budget.max_pages,
                max = sub_questions.len(),
                days = time_window_days
            ),
            format!(
                "检索查询 {} 条（课题原文 + 规则扩展）：{}。扩展只做「括号短语 / 连接词切分 / 拉丁词」，不发明新词。",
                queries.len(),
                queries
                    .iter()
                    .map(|query| format!("`{query}`"))
                    .collect::<Vec<_>>()
                    .join("、")
            ),
            "子问题由模板生成；语义化的查询改写要等模型接入（N5/D4）。".to_string(),
            "来源按 §9.3 规则分级：域名内置表 → 内容自证（标题/作者/日期）→ 兜底「弱」。"
                .to_string(),
        ];
        if let Some(scope) = request.scope.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            notes.push(format!("用户给定的范围：{scope}（当前仅作为计划说明记录）。"));
        }
        if local_baseline.is_empty() {
            notes.push("本地知识库暂无相关笔记：本次没有「已知基线」可复用。".to_string());
        }

        let now = now_iso();
        let session = ResearchSession {
            id: format!("research_{}", ulid()),
            request: ResearchRequest {
                topic,
                scope: request.scope.clone(),
                time_window_days: Some(time_window_days),
                write_back: request.write_back,
                target_category_id: request.target_category_id.clone(),
            },
            status: ResearchStatus::Planned,
            created_at: now.clone(),
            updated_at: now,
            plan: Some(ResearchPlan {
                topic: request.topic.trim().to_string(),
                scope: request.scope.clone(),
                sub_questions,
                queries,
                local_baseline,
                time_window_days,
                budget,
                notes,
            }),
            sources: Vec::new(),
            report: None,
            proposal: None,
            steps: vec![ResearchStepLog {
                step: "plan".to_string(),
                at: now_iso(),
                detail: "已立项：生成研究计划与本地基线".to_string(),
            }],
            elapsed_ms: started.elapsed().as_millis() as u64,
            abort_reason: None,
            written_note_id: None,
            decision_provider: None,
            decision_trace: None,
            dropped_sources: Vec::new(),
        };
        self.save(&session)?;
        self.emit_progress(&session.id, "plan", "已生成研究计划与本地基线", 0, session.sources.len());
        self.policy().audit(
            "research_start",
            false,
            "ok",
            started.elapsed().as_millis() as u64,
            None,
        );
        Ok(session)
    }

    // ---------------------------------------------------------------- ② 收集

    /// 收集材料：本地 FTS5 → 规则归属到子问题 → 分级。
    pub fn collect(&self, session_id: &str, confirmed: bool) -> Result<ResearchSession> {
        let started = Instant::now();
        let policy = self.policy().ensure_enabled("research_collect")?;
        let mut session = self.load(session_id)?;
        if session.status != ResearchStatus::Planned {
            return Err(AppError::invalid_input(format!(
                "会话当前状态是 {}，只有「已立项」的会话可以收集材料。",
                session.status.as_str()
            )));
        }
        let plan = session
            .plan
            .clone()
            .ok_or_else(|| AppError::internal("会话缺少研究计划。"))?;

        // 联网档位已打开且本次已确认时，才走外部检索（D3）；否则只用本地材料
        let network_allowed = policy.network == NetworkMode::AllowNetwork;
        let web_enabled = network_allowed && confirmed;
        if network_allowed && !confirmed {
            tracing::debug!(target: "ai", "研究收集未带单次确认：本次只使用本地材料");
        }

        let tools = self.tools();
        let notes = NoteService::new(Arc::clone(&self.ctx));
        // D5/N9：按扩展出来的一组查询逐条检索，再按笔记去重合并。
        // 顺序有意义：**课题原文的结果排在前面**（它是用户真正问的），扩展查询只做补充。
        let queries: Vec<String> = if plan.queries.is_empty() {
            vec![plan.topic.clone()]
        } else {
            plan.queries.clone()
        };
        self.emit_progress(
            session_id,
            "collect",
            &format!("正在检索本地知识库（{} 条查询）…", queries.len()),
            0,
            0,
        );
        let mut hits: Vec<crate::application::ai::tools::KbHit> = Vec::new();
        let mut matched_queries = 0usize;
        for query in &queries {
            let found = match tools.kb_search(query, MAX_SOURCES * 2) {
                Ok(found) => found,
                Err(error) => {
                    // 单条查询失败不该让整步崩掉（例如扩展出来的片段被 FTS 判为非法）
                    tracing::debug!(target: "ai", "查询「{}」检索失败，跳过：{}", query, error.message);
                    continue;
                }
            };
            matched_queries += 1;
            for hit in found {
                if hits.iter().any(|existing| existing.note_id == hit.note_id) {
                    continue;
                }
                hits.push(hit);
            }
            if hits.len() >= MAX_SOURCES * 2 {
                break;
            }
        }
        tracing::debug!(
            target: "ai",
            "本地检索：{} 条查询成功 {} 条、去重后命中 {} 篇笔记",
            queries.len(),
            matched_queries,
            hits.len()
        );

        let mut sources: Vec<ResearchSource> = Vec::new();
        let mut per_question: Vec<usize> = vec![0; plan.sub_questions.len()];
        // 长步骤开始前先看一次：用户可能在上一步刚结束时点了中止
        if let Some(persisted) = self.aborted_snapshot(session_id)? {
            return Ok(persisted);
        }
        let mut seen: Vec<(String, String)> = Vec::new(); // (子问题 id, 笔记 id)
        let mut notes_read = 0usize;
        for hit in hits {
            if sources.len() >= MAX_SOURCES {
                break;
            }
            let index = assign_sub_question(&hit.title, &hit.snippet, &plan.sub_questions);
            let question_id = plan.sub_questions[index].id.clone();
            if per_question[index] >= HITS_PER_SUB_QUESTION {
                continue;
            }
            if seen.contains(&(question_id.clone(), hit.note_id.clone())) {
                continue;
            }
            let detail = match notes.get(&hit.note_id) {
                Ok(detail) => detail,
                Err(error) => {
                    tracing::debug!(target: "ai", "跳过无法读取的笔记 {}：{}", hit.note_id, error.message);
                    continue;
                }
            };
            notes_read += 1;
            let grade: Grade = grade_local_note(
                detail.note.source_reliability,
                detail.note.source_type.as_deref(),
            );
            let fetched_at = detail.note.updated_at.clone();
            let stale = match detail.note.valid_until.as_deref() {
                Some(valid_until) => source_grade::is_stale(Some(valid_until), 0).unwrap_or(false),
                None => source_grade::is_stale(Some(&fetched_at), plan.time_window_days)
                    .unwrap_or(false),
            };
            let snippet = if hit.snippet.trim().is_empty() {
                first_sentence(&detail.content)
            } else {
                hit.snippet.clone()
            };
            seen.push((question_id.clone(), hit.note_id.clone()));
            per_question[index] += 1;
            sources.push(ResearchSource {
                reference: sources.len() + 1,
                title: detail.note.title.clone(),
                url: detail.note.source_ref.clone().filter(|value| value.starts_with("http")),
                origin: SourceOrigin::Local,
                note_id: Some(detail.note.id.clone()),
                sub_question_id: question_id,
                level: grade.level,
                reliability: grade.level.reliability(),
                grade_reason: grade.reason,
                published_at: None,
                fetched_at,
                applicable_version: None,
                snippet,
                char_start: hit.char_start,
                char_end: hit.char_end,
                heading: hit.heading.clone(),
                stale,
                // 真正入库前由 `score_sources` 用决策层统一打分
                credibility: grade.level.reliability() as f32 / 5.0,
            });
        }

        // ---- 外部检索（需要在「设置 → AI」开启联网档位 + 配置检索源 + 本次确认）----
        // 本地部分已经跑完：联网之前再确认一次用户没有中止
        if self.aborted_snapshot(session_id)?.is_some() {
            return self.finish_aborted_collect(session_id, sources, None, 0);
        }
        let mut web_collected = 0usize;
        let mut web_fetched = 0usize;
        let mut web_injected = 0usize;
        let mut web_note: String = if network_allowed {
            "联网档位已打开但本次未确认，未发起任何请求".to_string()
        } else {
            "未开启（联网档位为「仅本地」）".to_string()
        };
        if web_enabled && policy.search_endpoint.is_none() {
            web_note = "已允许联网，但尚未配置检索源地址：未发起任何请求".to_string();
        } else if web_enabled {
            let budget = plan.budget.clone();
            let deadline = std::time::Duration::from_secs(u64::from(budget.max_duration_secs));
            match self.tools().web_search(&plan.topic, budget.max_pages as usize, true) {
                Err(error) => {
                    web_note = format!("联网检索未成功：{}", error.message);
                }
                Ok(hits) => {
                    let mut web_seen: Vec<String> = Vec::new();
                    let mut dropped: Vec<DroppedSource> = Vec::new();
                    for hit in hits {
                        // 每个候选前都看一次：联网抓取可能跑好几分钟，用户要能随时叫停
                        if self.aborted_snapshot(session_id)?.is_some() {
                            return self.finish_aborted_collect(session_id, sources, None, web_fetched);
                        }
                        if sources.len() >= MAX_SOURCES || started.elapsed() >= deadline {
                            break;
                        }
                        // 域名策略逐条生效（§9.3「黑名单强制」）：抓正文默认关闭时
                        // 走的不是 web_fetch，因此候选本身必须在这里判定，
                        // 否则黑名单域名会以「检索摘要」的形式混进来源列表。
                        match self.policy().host_verdict(&hit.url) {
                            HostVerdict::Allowed => {}
                            verdict => {
                                let host = source_grade::host_of(&hit.url)
                                    .unwrap_or_else(|| hit.url.clone());
                                dropped.push(DroppedSource {
                                    url: hit.url.clone(),
                                    host,
                                    reason: match verdict {
                                        HostVerdict::DeniedByBlacklist => "denylist".to_string(),
                                        _ => "notAllowed".to_string(),
                                    },
                                });
                                continue;
                            }
                        }
                        let index = assign_sub_question(&hit.title, &hit.snippet, &plan.sub_questions);
                        let question_id = plan.sub_questions[index].id.clone();
                        if per_question[index] >= HITS_PER_SUB_QUESTION {
                            continue;
                        }
                        if web_seen.contains(&hit.url) {
                            continue;
                        }
                        web_seen.push(hit.url.clone());

                        // 抓正文（可关闭）：失败就退回检索摘要，不把整页丢给用户
                        let mut snippet = hit.snippet.clone();
                        if policy.allow_fetch_body && web_fetched < budget.max_pages as usize {
                            if let Ok(page) = self.tools().web_fetch(&hit.url, true) {
                                web_fetched += 1;
                                let body = first_sentence(&page.markdown);
                                snippet = if body.trim().is_empty() { snippet } else { body };
                                self.emit_progress(
                                    session_id,
                                    "collect",
                                    &format!("已抓取正文 {web_fetched} 页（预算 ≤{} 页）", budget.max_pages),
                                    web_fetched,
                                    sources.len() + 1,
                                );
                            }
                        }
                        if web_search::looks_like_injection(&format!("{} {}", hit.title, snippet)) {
                            web_injected += 1;
                            tracing::debug!(target: "ai", "丢弃疑似注入页面：{}", hit.host);
                            continue;
                        }
                        let grade = source_grade::grade_web(&hit.url, &hit.title, &snippet);
                        let stale = source_grade::is_stale(hit.published_at.as_deref(), plan.time_window_days)
                            .unwrap_or(false);
                        per_question[index] += 1;
                        web_collected += 1;
                        sources.push(ResearchSource {
                            reference: sources.len() + 1,
                            title: hit.title.clone(),
                            url: Some(hit.url.clone()),
                            origin: SourceOrigin::Web,
                            note_id: None,
                            sub_question_id: question_id,
                            level: grade.level,
                            reliability: grade.level.reliability(),
                            grade_reason: grade.reason,
                            published_at: hit.published_at.clone(),
                            fetched_at: now_iso(),
                            applicable_version: None,
                            snippet,
                            // 外部来源没有"原文字符区间"这回事（正文是抓来的，不落库）
                            char_start: None,
                            char_end: None,
                            heading: None,
                            stale,
                            credibility: grade.level.reliability() as f32 / 5.0,
                        });
                    }
                    if web_collected == 0 {
                        web_note = "联网检索没有返回可用来源".to_string();
                    } else {
                        web_note = format!(
                            "外部来源 {web_collected} 条（实际抓取正文 {web_fetched} 页；预算：≤{} 页 / ≤{} 秒）",
                            budget.max_pages, budget.max_duration_secs
                        );
                    }
                    if web_injected > 0 {
                        web_note.push_str(&format!(
                            "；丢弃疑似注入页面 {web_injected} 个（网页内容是数据，不是指令）"
                        ));
                    }
                    if !dropped.is_empty() {
                        let blacklisted = dropped
                            .iter()
                            .filter(|item| item.reason == "denylist")
                            .count();
                        web_note.push_str(&format!(
                            "；丢弃域名策略命中的来源 {} 个（黑名单 {blacklisted} 个 / 不在白名单 {} 个）",
                            dropped.len(),
                            dropped.len() - blacklisted
                        ));
                    }
                    session.dropped_sources = dropped;
                }
            }
        }

        if let Some(plan_mut) = session.plan.as_mut() {
            for (index, question) in plan_mut.sub_questions.iter_mut().enumerate() {
                question.local_hits = per_question.get(index).copied().unwrap_or(0);
            }
        }
        // 决策层打分（§9.6）：规则档默认；开了本地模型就由它打分、失败自动降级
        let (decision_provider, score_fallback, decision_trace) =
            self.score_sources(&plan.topic, &mut sources, confirmed);
        session.decision_provider = Some(decision_provider.clone());
        // D2/N6：把这次打分的 Trace 留在会话上，verify 时写进报告（可解释）
        session.decision_trace = decision_trace;
        if let Some(note) = score_fallback {
            tracing::debug!(target: "ai", "{note}");
            self.policy()
                .audit("research_score", true, "error", 0, Some(&note));
        }

        // 收尾前最后一次确认：**中止优先**——否则这次保存会把用户的中止覆盖回去
        if self.aborted_snapshot(session_id)?.is_some() {
            return self.finish_aborted_collect(
                session_id,
                sources,
                Some(decision_provider),
                web_fetched,
            );
        }

        session.sources = sources;
        session.status = ResearchStatus::Collected;
        session.updated_at = now_iso();
        session.elapsed_ms += started.elapsed().as_millis() as u64;
        session.steps.push(ResearchStepLog {
            step: "collect".to_string(),
            at: now_iso(),
            detail: format!(
                "本地材料 {} 条（读取笔记 {} 篇）；{web_note}",
                session.sources.len() - web_collected,
                notes_read,
            ),
        });
        self.save(&session)?;
        self.emit_progress(
            session_id,
            "collect",
            &format!(
                "收集完成：来源 {} 条（本地 {} 条 / 外部 {} 条）",
                session.sources.len(),
                session.sources.len() - web_collected,
                web_collected
            ),
            web_fetched,
            session.sources.len(),
        );
        // 只有真的发了外部请求才记 outbound=true
        self.policy().audit(
            "research_collect",
            web_collected > 0 || web_fetched > 0,
            "ok",
            started.elapsed().as_millis() as u64,
            Some(if web_enabled { "with_web" } else { "local_only" }),
        );
        Ok(session)
    }

    // ---------------------------------------------------------------- ③ 验证

    /// 验证：按子问题汇总，判定交叉验证与时效。
    ///
    /// `confirm_outbound`（Track C2/C5）：冲突判定可能调用在线模型，因此同样受
    /// 「逐次外发确认」约束——未确认时不构造在线档（零请求），回退本地 / 规则档。
    pub fn verify(&self, session_id: &str, confirm_outbound: bool) -> Result<ResearchSession> {
        let started = Instant::now();
        self.policy().ensure_enabled("research_verify")?;
        let mut session = self.load(session_id)?;
        if session.status != ResearchStatus::Collected {
            return Err(AppError::invalid_input(format!(
                "会话当前状态是 {}，只有「已收集」的会话可以验证。",
                session.status.as_str()
            )));
        }
        let plan = session
            .plan
            .clone()
            .ok_or_else(|| AppError::internal("会话缺少研究计划。"))?;

        let mut claims = Vec::new();
        // 同一子问题下出现对立表述 → 并列呈现、不进结论段
        let mut conflicting: Vec<String> = Vec::new();
        // 冲突判定实际用到的档位（C5）：模型判的？还是规则判的？
        let mut conflict_providers: Vec<String> = Vec::new();
        for question in &plan.sub_questions {
            let all_evidence: Vec<&ResearchSource> = session
                .sources
                .iter()
                .filter(|source| source.sub_question_id == question.id)
                .collect();
            // E 级（不可用：社交平台 / AI 生成 / 无法判定出处）**只作线索**：
            // 不参与引用、交叉验证、时效与冲突判定，否则「一条 A 级 + 一条 E 级」
            // 会被算成「两个独立来源」而进入结论段——这正是规格 §9.3 明确要避免的。
            let evidence: Vec<&ResearchSource> = all_evidence
                .iter()
                .copied()
                .filter(|source| source.level != SourceLevel::E)
                .collect();
            if evidence.is_empty() {
                let clues: Vec<usize> = all_evidence
                    .iter()
                    .map(|source| source.reference)
                    .collect();
                claims.push(ResearchClaim {
                    sub_question_id: question.id.clone(),
                    text: if clues.is_empty() {
                        format!("「{}」：本地知识库暂无材料。", question.question)
                    } else {
                        format!(
                            "「{}」：只有不可用来源（E 级）作线索，未形成可引用结论。",
                            question.question
                        )
                    },
                    citations: clues,
                    best_level: SourceLevel::E,
                    cross_verified: false,
                    fresh: true,
                    related_notes: Vec::new(),
                    confidence: 0.0,
                });
                continue;
            }
            let mut independent: Vec<String> = Vec::new();
            for source in &evidence {
                // 独立性口径与证据充分性评估**共用同一个函数**（D5/N9）：
                // 两处各写一遍迟早会漂移，然后出现"评估说够、结论段却进不去"。
                let key = crate::application::ai::evidence::source_identity(source);
                if !independent.contains(&key) {
                    independent.push(key);
                }
            }
            let quotes: Vec<String> = evidence
                .iter()
                .take(2)
                .map(|source| first_sentence(&source.snippet))
                .filter(|value| !value.is_empty())
                .collect();
            let (conflict, conflict_provider) =
                self.judge_conflict(question, &evidence, confirm_outbound);
            conflict_providers.push(conflict_provider);
            if conflict {
                conflicting.push(question.id.clone());
            }
            let confidence = claim_confidence(
                &evidence.iter().map(|source| source.credibility).collect::<Vec<f32>>(),
                independent.len(),
            );
            claims.push(ResearchClaim {
                sub_question_id: question.id.clone(),
                text: if conflict {
                    format!(
                        "「{}」存在冲突：来源之间既有支持性表述、也有反对/限制性表述，本次不选边。{}",
                        question.question,
                        quotes.join("；")
                    )
                } else {
                    format!("关于「{}」：{}", question.question, quotes.join("；"))
                },
                citations: evidence.iter().map(|source| source.reference).collect(),
                best_level: evidence
                    .iter()
                    .map(|source| source.level)
                    .max()
                    .unwrap_or(SourceLevel::E),
                cross_verified: independent.len() >= 2,
                fresh: !evidence.iter().any(|source| source.stale),
                related_notes: evidence
                    .iter()
                    .filter_map(|source| source.note_id.clone())
                    .collect(),
                confidence,
            });
        }

        // 验证阶段先落一份「半成品报告」：结论/事实/待验证已定，正文与警告留给 compose。
        let provider = session
            .decision_provider
            .clone()
            .unwrap_or_else(|| self.decision.id().to_string());
        // 冲突判定用了哪些档（去重后按出现顺序）：报告里要如实写出来
        let mut unique_conflict_providers: Vec<String> = Vec::new();
        for used in &conflict_providers {
            if !unique_conflict_providers.contains(used) {
                unique_conflict_providers.push(used.clone());
            }
        }
        let conflict_provider = if unique_conflict_providers.is_empty() {
            "rule".to_string()
        } else {
            unique_conflict_providers.join(" / ")
        };
        let draft = pending_report(
            &session,
            claims,
            &conflicting,
            &provider,
            &conflict_provider,
            session.decision_trace.clone(),
        );
        session.report = Some(draft);
        session.status = ResearchStatus::Verified;
        session.updated_at = now_iso();
        session.elapsed_ms += started.elapsed().as_millis() as u64;
        session.steps.push(ResearchStepLog {
            step: "verify".to_string(),
            at: now_iso(),
            detail: format!(
                "按 {} 个子问题汇总：交叉验证 {} 条，单一来源 {} 条，存在冲突 {} 条",
                plan.sub_questions.len(),
                session
                    .report
                    .as_ref()
                    .map(|report| report.key_facts.len())
                    .unwrap_or(0),
                session
                    .report
                    .as_ref()
                    .map(|report| report.pending.len())
                    .unwrap_or(0),
                session
                    .report
                    .as_ref()
                    .map(|report| report.conflicts.len())
                    .unwrap_or(0)
            ),
        });
        if self.aborted_snapshot(session_id)?.is_some() {
            let draft = session.report.clone();
            return self.finish_aborted_step(
                session_id,
                "验证途中被用户中止：已得的验证结论保留为草稿",
                |persisted| persisted.report = draft,
            );
        }
        self.save(&session)?;
        self.emit_progress(&session.id, "verify", "验证完成：交叉验证与冲突判定已写入报告草稿", 0, session.sources.len());
        self.policy().audit(
            "research_verify",
            false,
            "ok",
            started.elapsed().as_millis() as u64,
            None,
        );
        Ok(session)
    }

    // ---------------------------------------------------------------- ④ 汇总

    /// 逐次外发确认用的预览（Track C2）：**只读**——不发请求、不写审计。
    ///
    /// 回答「如果现在调用在线模型，会把哪些片段、发到哪台主机」。
    pub fn outbound_preview(&self, session_id: &str) -> Result<ResearchOutboundPreview> {
        let session = self.load(session_id)?;
        let policy = self.ctx.settings().ai;
        let service = self.policy();

        let blocked = |reason: &str| ResearchOutboundPreview {
            will_send: false,
            host: None,
            snippets: Vec::new(),
            reason: reason.to_string(),
        };
        if !policy.enabled {
            return Ok(blocked("AI 总开关关闭：不会外发任何内容。"));
        }
        if !policy.remote_model_compose && !policy.remote_model_score {
            return Ok(blocked("未开启在线模型：内容不会离开本机。"));
        }
        if service.remote_model().is_none() {
            return Ok(blocked(
                "在线模型未配置完整（缺基地址 / 模型名 / API Key）：内容不会外发。",
            ));
        }
        if policy.remote_model_authorized_at.is_none() {
            return Ok(blocked("在线模型尚未授权：内容不会外发。"));
        }

        let host = service.remote_diagnosis().0.and_then(|(base, _)| host_of(&base));
        let snippets: Vec<ResearchOutboundSnippet> = session
            .sources
            .iter()
            .take(5)
            .map(|source| ResearchOutboundSnippet {
                title: source.title.clone(),
                snippet: source.snippet.clone(),
            })
            .collect();
        let reason = if policy.confirm_each_outbound {
            "本次会把下列片段交给在线模型（已开启逐次确认：要点确认才会发送）。".to_string()
        } else {
            "在线模型已开启，本次会把下列片段交给它。".to_string()
        };
        Ok(ResearchOutboundPreview { will_send: true, host, snippets, reason })
    }

    /// ④ 汇总：渲染六段报告。
    ///
    /// `confirm_outbound` 只在开启「逐次外发确认」（Track C2）时才有意义：
    /// 为 `false` 时**在线档不参与**（零请求），自动降级到本地 / 模板。
    pub fn compose(&self, session_id: &str, confirm_outbound: bool) -> Result<ResearchSession> {
        let started = Instant::now();
        let policy = self.policy().ensure_enabled("research_compose")?;
        let mut session = self.load(session_id)?;
        if session.status != ResearchStatus::Verified {
            return Err(AppError::invalid_input(format!(
                "会话当前状态是 {}，只有「已验证」的会话可以生成报告。",
                session.status.as_str()
            )));
        }
        let plan = session
            .plan
            .clone()
            .ok_or_else(|| AppError::internal("会话缺少研究计划。"))?;
        let mut report = session
            .report
            .clone()
            .ok_or_else(|| AppError::internal("会话缺少验证结果。"))?;

        // 生成侧：在线（已授权）→ 本地 → 模板实现（文本自带「未接模型」标注）。
        // 前面一档失败就**自动降级**到下一档，并把原因写进审计与警告块。
        let template = TemplateGenerativeModel::new();
        let evidence: Vec<RetrievedChunk> = session
            .sources
            .iter()
            .take(5)
            .enumerate()
            .map(|(index, source)| RetrievedChunk {
                note_id: source.note_id.clone().unwrap_or_else(|| format!("ref{}", index + 1)),
                chunk_index: index as i64,
                // 有分块标题路径就用它（真实分块），否则退回笔记标题
                heading_path: source
                    .heading
                    .clone()
                    .filter(|heading| !heading.is_empty())
                    .map(|heading| heading.split(" > ").map(str::to_string).collect())
                    .unwrap_or_else(|| vec![source.title.clone()]),
                snippet: source.snippet.clone(),
                // N5：带上**真实的原文字符区间**；定位不到时退回 0..摘要长度（如实标注为粗略）
                char_start: source.char_start.unwrap_or(0),
                char_end: source
                    .char_end
                    .unwrap_or_else(|| source.snippet.chars().count() as i64),
                score: source.reliability as f32,
                origin: RetrievalOrigin::Bm25,
            })
            .collect();
        let composers: Vec<(String, Arc<dyn GenerativeModel>)> =
            self.composer_candidates(confirm_outbound);
        // Track C2：开了逐次确认而本次没确认 —— 报告里要说清楚"为什么没用在线模型"。
        // 这里只做只读判断（真正的拒绝与审计发生在 `composer_candidates` 里）。
        let unconfirmed_remote = policy.remote_model_compose
            && policy.remote_model_authorized_at.is_some()
            && self.policy().remote_model().is_some()
            && !self.policy().outbound_confirmed(confirm_outbound);
        let mut model_fallback: Option<String> = None;
        let mut response = None;
        for (index, (provider_id, model)) in composers.iter().enumerate() {
            // 外发标记只对在线这一档为真：本地模型不需要，模板更不需要
            let request = GenerationRequest {
                prompt: format!("为课题「{}」写推测段", plan.topic),
                evidence: evidence.clone(),
                max_tokens: Some(512),
                allow_remote: !model.is_local(),
            };
            // 这一档开始：先发一次空串，保证上一档（若有）的半截内容不会留在界面上
            self.emit_compose_delta(&session.id, provider_id, "", "streaming");
            // 流式生成：累加增量并实时推给界面（不支持的 provider 会退化成"一次性 + 一次回调"）
            let mut accumulated = String::new();
            let streamed = model.complete_streaming(&request, &mut |delta| {
                accumulated.push_str(delta);
                self.emit_compose_delta(&session.id, provider_id, &accumulated, "streaming");
            });
            match streamed {
                Ok(done) => {
                    response = Some(done);
                    break;
                }
                Err(error) => {
                    // 这一档失败：显式清掉预览（**不留下半截内容**），再降级到下一档
                    self.emit_compose_delta(&session.id, provider_id, "", "failed");
                    model_fallback = Some(format!(
                        "{} 生成失败（{}），本次退回{}",
                        provider_id,
                        error.message,
                        if index + 1 < composers.len() {
                            composers[index + 1].0.as_str()
                        } else {
                            "模板实现"
                        }
                    ));
                }
            }
        }
        let request = GenerationRequest {
            prompt: format!("为课题「{}」写推测段", plan.topic),
            evidence,
            max_tokens: Some(512),
            allow_remote: false,
        };
        // 模型档全部不可用（或压根没配）时用模板实现——**永远有产出**，不假装是模型写的
        let response = match response {
            Some(done) => done,
            None => template.complete(&request)?,
        };
        // 流式结束的正式收尾：把最终文本连同 provider 一起发一次 `done`，
        // 界面据此把预览定格为"就是它写的"（真正的权威版本仍是命令返回的报告）
        self.emit_compose_delta(&session.id, &response.model, &response.text, "done");
        if let Some(note) = &model_fallback {
            tracing::debug!(target: "ai", "{note}");
            self.policy().audit(
                "research_compose",
                true,
                "error",
                0,
                Some("LOCAL_MODEL_FALLBACK"),
            );
        }
        let speculation = response
            .text
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let composer_label = response.model.clone();

        report.generated_at = now_iso();
        report.composer = composer_label.clone();
        report.speculation = speculation;
        report.warnings = collect_warnings(
            &session,
            &report,
            policy.network == NetworkMode::AllowNetwork,
            // 「是谁写的」以实际生效的 composer 为准：模板之外（本地或在线模型）都算模型生成
            composer_label != TemplateGenerativeModel::new().id(),
        );
        if let Some(note) = model_fallback {
            report.warnings.push(ResearchWarning {
                kind: WarningKind::ModelGenerated,
                message: note,
                references: Vec::new(),
            });
        }
        if unconfirmed_remote {
            report.warnings.push(ResearchWarning {
                kind: WarningKind::ModelGenerated,
                message:
                    "在线模型已开启「逐次外发确认」：本次未确认，一个字节都没有外发，已改用下一档。"
                        .to_string(),
                references: Vec::new(),
            });
        }
        report.valid_until = Some(date_offset_days(REVIEW_PERIOD_DAYS));
        report.source_reliability = session
            .sources
            .iter()
            .map(|source| source.reliability)
            .min()
            .unwrap_or(1);

        // —— D5/N9 证据充分性：材料够不够撑结论，明明白白写在报告里 ——
        // 定稿前算一次（此时来源与结论都已确定），结论段是否为空必须能被解释。
        let assessment = crate::application::ai::evidence::assess(
            &plan.topic,
            &plan.sub_questions,
            &session.sources,
        );
        if !assessment.sufficient {
            let gaps = assessment
                .gaps
                .iter()
                .map(|gap| gap.message.clone())
                .collect::<Vec<_>>()
                .join(" ");
            let recommendation = if assessment.recommendation.trim().is_empty() {
                String::new()
            } else {
                format!(" {}", assessment.recommendation)
            };
            report.warnings.push(ResearchWarning {
                kind: WarningKind::InsufficientEvidence,
                message: format!(
                    "{} {} 本报告不给结论段。{}",
                    assessment.verdict.trim(),
                    gaps.trim(),
                    recommendation.trim()
                )
                .trim_end()
                .to_string(),
                references: assessment
                    .gaps
                    .iter()
                    .flat_map(|gap| gap.references.clone())
                    .collect(),
            });
        }
        report.evidence = assessment;
        report.markdown = render_markdown(&session, &plan, &report);

        session.report = Some(report);
        session.status = ResearchStatus::Composed;
        session.updated_at = now_iso();
        session.elapsed_ms += started.elapsed().as_millis() as u64;
        session.steps.push(ResearchStepLog {
            step: "compose".to_string(),
            at: now_iso(),
            detail: format!("已生成报告：{composer_label}"),
        });
        if self.aborted_snapshot(session_id)?.is_some() {
            let report = session.report.clone();
            return self.finish_aborted_step(
                session_id,
                "汇总途中被用户中止：已生成的报告保留为草稿",
                |persisted| persisted.report = report,
            );
        }
        self.save(&session)?;
        self.emit_progress(&session.id, "compose", "汇总完成：六段报告已生成", 0, session.sources.len());
        self.policy().audit(
            "research_compose",
            false,
            "ok",
            started.elapsed().as_millis() as u64,
            Some(&composer_label),
        );
        Ok(session)
    }

    // ---------------------------------------------------------------- ⑤ 写入提案

    /// 产出写入提案（**纯建议**：不落库，落库由 D2 的 `write` 在用户确认后执行）。
    pub fn proposal(&self, session_id: &str) -> Result<ResearchSession> {
        let started = Instant::now();
        self.policy().ensure_enabled("research_proposal")?;
        let mut session = self.load(session_id)?;
        // 允许「已汇总」或「已提案」时重算：同标题冲突可能在两次提案之间出现，
        // 用户需要看到最新的 diff 与同标题提示。
        if !matches!(session.status, ResearchStatus::Composed | ResearchStatus::Proposed) {
            return Err(AppError::invalid_input(format!(
                "会话当前状态是 {}，只有「已汇总」的会话可以生成写入提案。",
                session.status.as_str()
            )));
        }
        let report = session
            .report
            .clone()
            .ok_or_else(|| AppError::internal("会话缺少报告。"))?;
        let title = format!("调研报告：{}", session.request.topic);
        let duplicates = self.find_duplicates(&title)?;
        let source_ref = source_ref_of(&report);
        let body = format!(
            "{}\n\n---\n\n> 本报告由研究助手生成（{composer}）：结论段为**来源原句摘录**，未做语义归并；\n> 推测段默认不写入笔记，需要时请在写入确认里勾选。\n> 会话 `{id}` · 生成于 {generated}",
            report.markdown,
            composer = report.composer,
            id = session.id,
            generated = report.generated_at
        );
        let diff_preview = body
            .lines()
            .map(|line| format!("+{line}"))
            .collect::<Vec<_>>()
            .join("\n");

        session.proposal = Some(ResearchProposal {
            title,
            category_id: session.request.target_category_id.clone(),
            tags: vec!["调研报告".to_string(), "待复核".to_string()],
            source_type: "research".to_string(),
            source_ref,
            source_reliability: report.source_reliability,
            valid_until: report.valid_until.clone(),
            body,
            diff_preview,
            duplicates,
            suggest_source_note: report.references.len() > 3,
        });
        session.status = ResearchStatus::Proposed;
        session.updated_at = now_iso();
        session.elapsed_ms += started.elapsed().as_millis() as u64;
        session.steps.push(ResearchStepLog {
            step: "proposal".to_string(),
            at: now_iso(),
            detail: "已生成写入提案（等待用户确认后落库）".to_string(),
        });
        if self.aborted_snapshot(session_id)?.is_some() {
            let proposal = session.proposal.clone();
            return self.finish_aborted_step(
                session_id,
                "生成提案途中被用户中止：已生成的提案保留为草稿",
                |persisted| persisted.proposal = proposal,
            );
        }
        self.save(&session)?;
        self.emit_progress(&session.id, "proposal", "写入提案已生成，等待确认", 0, session.sources.len());
        self.policy().audit(
            "research_proposal",
            false,
            "ok",
            started.elapsed().as_millis() as u64,
            None,
        );
        Ok(session)
    }

    // ---------------------------------------------------------------- 其它

    // ---------------------------------------------------------------- ⑤ 落库

    /// 把报告写入知识库（**用户确认后**调用）。
    ///
    /// 纪律：默认落 Inbox；同标题不覆盖（`append` 必须显式指定目标，且走 `note_save`
    /// 因此自动留历史快照、可回滚）；`abandon` 只记一步日志，不写任何文件。
    pub fn write(&self, input: ResearchWriteInput) -> Result<ResearchWriteResult> {
        let started = Instant::now();
        self.policy().ensure_enabled("research_write")?;
        let mut session = self.load(&input.session_id)?;
        if session.status != ResearchStatus::Proposed {
            return Err(AppError::invalid_input(format!(
                "会话当前状态是 {}，只有「已提案」的会话可以写入（重复写入请重新生成提案）。",
                session.status.as_str()
            )));
        }
        let proposal = session
            .proposal
            .clone()
            .ok_or_else(|| AppError::internal("会话缺少写入提案。"))?;
        let mode = input.mode.trim().to_ascii_lowercase();
        if mode == WRITE_MODE_ABANDON {
            session.steps.push(ResearchStepLog {
                step: "write".to_string(),
                at: now_iso(),
                detail: "用户选择放弃本次写入：未落库，材料仍保留为草稿".to_string(),
            });
            session.updated_at = now_iso();
            self.save(&session)?;
            self.emit_progress(
                &session.id,
                "write",
                "已放弃本次写入：未落库，材料仍保留为草稿",
                0,
                session.sources.len(),
            );
            self.policy()
                .audit("research_write", false, "ok", started.elapsed().as_millis() as u64, Some("abandoned"));
            return Ok(ResearchWriteResult {
                session_id: session.id,
                outcome: "abandoned".to_string(),
                note_id: None,
                note_title: None,
                source_note_id: None,
                source_reliability: proposal.source_reliability,
                valid_until: proposal.valid_until,
                diff_preview: String::new(),
            });
        }
        if mode != WRITE_MODE_CREATE && mode != WRITE_MODE_APPEND {
            return Err(AppError::invalid_input(format!(
                "不支持的写入方式「{}」（可用 create / append / abandon）。",
                input.mode
            )));
        }

        let title = input
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| proposal.title.clone());
        let body = write_body(&proposal, &session, input.include_speculation);
        let appended = mode == WRITE_MODE_APPEND;
        let notes = NoteService::new(Arc::clone(&self.ctx));

        let (note_id, note_title, outcome, diff_preview) = if appended {
            let target = input
                .target_note_id
                .clone()
                .or_else(|| proposal.duplicates.first().map(|item| item.note_id.clone()))
                .ok_or_else(|| {
                    AppError::invalid_input(
                        "追加模式需要指定目标笔记（同标题笔记不存在时请改用「新建」）。",
                    )
                })?;
            let detail = notes.get(&target)?;
            let merged = format!("{}\n\n---\n\n{}", detail.content.trim_end(), body);
            // `note_save` 会先留历史快照 → 追加同样可回滚
            let saved = notes.save(SaveNoteInput {
                id: target,
                content: merged.clone(),
                base_content_hash: None,
                title: None,
                category_id: None,
                tags: None,
            })?;
            (
                saved.note.id,
                saved.note.title,
                "appended".to_string(),
                append_diff(&detail.content, &body),
            )
        } else {
            let created = notes.create(CreateNoteInput {
                title: title.clone(),
                category_id: input.category_id.clone().or_else(|| proposal.category_id.clone()),
                initial_content: Some(body.clone()),
                tags: Some(proposal.tags.clone()),
                parent_id: None,
            })?;
            (
                created.note.id,
                created.note.title,
                "created".to_string(),
                create_diff(&body),
            )
        };

        // N1 来源元数据：来源类型 / 引用 / 可靠性 / 复核时间。
        //
        // **新建**时写研究产物自己的元数据；**追加**时保留目标笔记原有的标注，只补空缺
        // ——追加是在用户已有的笔记上加一段报告，不能把它原本的「官方文档 / 可靠性 5 /
        // 2027-03-31 复核」静默改写成「research / 可靠性 2 / +180 天」。
        let merged = if appended {
            let existing = notes.get(&note_id)?.note;
            let keep = existing.source_type.is_some()
                || existing.source_ref.is_some()
                || existing.source_reliability.is_some()
                || existing.valid_until.is_some();
            if keep {
                session.steps.push(ResearchStepLog {
                    step: "write".to_string(),
                    at: now_iso(),
                    detail: "追加模式：保留目标笔记原有的来源与时效标注（不覆盖用户元数据）"
                        .to_string(),
                });
            }
            (
                existing.source_type.clone().or_else(|| Some(proposal.source_type.clone())),
                existing.source_ref.clone().or_else(|| Some(proposal.source_ref.clone())),
                existing.source_reliability.or(Some(proposal.source_reliability)),
                existing.valid_until.clone().or_else(|| proposal.valid_until.clone()),
            )
        } else {
            (
                Some(proposal.source_type.clone()),
                Some(proposal.source_ref.clone()),
                Some(proposal.source_reliability),
                proposal.valid_until.clone(),
            )
        };
        notes.set_source(SetSourceInput {
            id: note_id.clone(),
            source_type: merged.0,
            source_ref: merged.1,
            source_reliability: merged.2,
            valid_until: merged.3,
        })?;

        let source_note_id = if input.create_source_note {
            let report = session.report.clone();
            let content = report
                .as_ref()
                .map(source_note_markdown)
                .unwrap_or_else(|| "_本次没有来源。_".to_string());
            let child = notes.create(CreateNoteInput {
                title: format!("来源清单：{}", session.request.topic),
                category_id: None,
                initial_content: Some(content),
                tags: Some(vec!["来源清单".to_string()]),
                parent_id: Some(note_id.clone()),
            })?;
            Some(child.note.id)
        } else {
            None
        };

        session.written_note_id = Some(note_id.clone());
        session.status = ResearchStatus::Written;
        session.updated_at = now_iso();
        session.elapsed_ms += started.elapsed().as_millis() as u64;
        session.steps.push(ResearchStepLog {
            step: "write".to_string(),
            at: now_iso(),
            detail: format!(
                "已{}：{}（来源等级 {} · 复核 {}）{}",
                if outcome == "appended" { "追加" } else { "新建" },
                note_title,
                proposal.source_reliability,
                proposal.valid_until.as_deref().unwrap_or("未设置"),
                if input.include_speculation { " · 含推测段" } else { "" }
            ),
        });
        self.save(&session)?;
        self.emit_progress(&session.id, "write", "已按你的确认写入知识库", 0, session.sources.len());
        self.policy().audit(
            "research_write",
            false,
            "ok",
            started.elapsed().as_millis() as u64,
            Some(&outcome),
        );
        Ok(ResearchWriteResult {
            session_id: session.id,
            outcome,
            note_id: Some(note_id),
            note_title: Some(note_title),
            source_note_id,
            source_reliability: proposal.source_reliability,
            valid_until: proposal.valid_until,
            diff_preview,
        })
    }

    /// 中止会话（已得材料保留为草稿）。
    pub fn abort(&self, session_id: &str, reason: Option<String>) -> Result<ResearchSession> {
        self.policy().ensure_enabled("research_abort")?;
        let mut session = self.load(session_id)?;
        if session.status == ResearchStatus::Aborted {
            return Ok(session);
        }
        session.status = ResearchStatus::Aborted;
        session.abort_reason = Some(
            reason
                .unwrap_or_else(|| "用户中止".to_string())
                .trim()
                .to_string(),
        );
        session.updated_at = now_iso();
        session.steps.push(ResearchStepLog {
            step: "abort".to_string(),
            at: now_iso(),
            detail: "已中止：已有材料保留为草稿".to_string(),
        });
        self.save(&session)?;
        self.emit_progress(&session.id, "abort", "已中止：已有材料保留为草稿", 0, session.sources.len());
        self.policy()
            .audit("research_abort", false, "ok", 0, Some("aborted"));
        Ok(session)
    }

    /// 读取一个会话。
    pub fn get(&self, session_id: &str) -> Result<ResearchSession> {
        self.load(session_id)
    }

    /// 最近的会话（新的在前）。
    pub fn list(&self, limit: usize) -> Result<Vec<ResearchSessionSummary>> {
        let dir = self.ctx.paths.research_dir();
        let Ok(entries) = fs::read_dir(&dir) else {
            return Ok(Vec::new());
        };
        let mut sessions: Vec<ResearchSession> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(session) = serde_json::from_str::<ResearchSession>(&text) {
                    sessions.push(session);
                }
            }
        }
        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        sessions.truncate(limit.clamp(1, 200));
        Ok(sessions.iter().map(ResearchSessionSummary::from).collect())
    }

    /// 同标题已有笔记（大小写与空白归一后完全匹配）。
    fn find_duplicates(&self, title: &str) -> Result<Vec<DuplicateNote>> {
        let list = NoteService::new(Arc::clone(&self.ctx)).list(NoteListQuery {
            keyword: Some(title.to_string()),
            page_size: Some(50),
            ..Default::default()
        })?;
        let key = normalize_title_key(title);
        Ok(list
            .items
            .into_iter()
            .filter(|note| normalize_title_key(&note.title) == key)
            .map(|note| DuplicateNote {
                note_id: note.id,
                title: note.title,
                updated_at: note.updated_at,
            })
            .collect())
    }

    fn session_path(&self, session_id: &str) -> Result<PathBuf> {
        let trimmed = session_id.trim();
        let safe = trimmed.starts_with("research_")
            && trimmed
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        if !safe {
            return Err(AppError::invalid_input("研究会话 id 不合法。"));
        }
        Ok(self.ctx.paths.research_dir().join(format!("{trimmed}.json")))
    }

    fn save(&self, session: &ResearchSession) -> Result<()> {
        let path = self.session_path(&session.id)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(session)
            .map_err(|error| AppError::internal(format!("研究会话序列化失败：{error}")))?;
        self.ctx.storage.write_atomic_absolute(&path, &bytes)
    }

    fn load(&self, session_id: &str) -> Result<ResearchSession> {
        let path = self.session_path(session_id)?;
        let text = fs::read_to_string(&path)
            .map_err(|_| AppError::new(ErrorCode::InvalidInput, format!("找不到研究会话 {session_id}。")))?;
        serde_json::from_str(&text)
            .map_err(|error| AppError::internal(format!("研究会话解析失败：{error}")))
    }
}

/// 用决策层给每条来源打「可信度」分（0–1，§9.6）。
///
/// 规则档下分数来自 `RuleDecisionProvider`（关键词命中 + 来源可靠性）；接入 Jev / 本地模型后
/// 由模型给出**类型化置信度**，上层不改动——只换 `ResearchService::with_decision_model` 的注入。
/// 这里把「模型分」与「级别折算分」各取一半：级别是用户/规则标注的事实，模型分反映与课题的相关度。
fn blended_credibility(model_score: Option<f32>, reliability: i64) -> f32 {
    let base = (reliability.clamp(1, 5) as f32) / 5.0;
    let score = model_score.unwrap_or(base).clamp(0.0, 1.0);
    (0.5 * score + 0.5 * base).clamp(0.0, 1.0)
}

/// 结论置信度：支持来源可信度的均值 × 交叉验证系数（单一来源打折）。
fn claim_confidence(credibilities: &[f32], independent: usize) -> f32 {
    if credibilities.is_empty() {
        return 0.0;
    }
    let mean = credibilities.iter().sum::<f32>() / credibilities.len() as f32;
    let support = if independent >= 2 { 1.0 } else { 0.6 };
    (mean * support).clamp(0.0, 1.0)
}

/// 置信度的人读标签（报告与界面共用口径）。
pub fn confidence_label(value: f32) -> &'static str {
    if value >= 0.7 {
        "高"
    } else if value >= 0.45 {
        "中"
    } else {
        "低"
    }
}

/// 校验并归一化课题（过于宽泛时要求补充范围）。
pub fn validate_topic(topic: &str) -> Result<String> {
    let trimmed = topic.trim();
    let chars = trimmed.chars().count();
    if trimmed.is_empty() {
        return Err(AppError::invalid_input("课题不能为空。"));
    }
    let lowered = trimmed.to_lowercase();
    if chars < 4 || BROAD_TOPICS.contains(&lowered.as_str()) {
        return Err(AppError::invalid_input(format!(
            "课题「{trimmed}」过于宽泛：请补充范围（领域、关注点或时间窗），例如「Rust 异步运行时的调度策略」。"
        )));
    }
    if chars > 200 {
        return Err(AppError::invalid_input("课题过长（上限 200 字），请精简后再试。"));
    }
    Ok(trimmed.to_string())
}

/// 把课题拆成子问题（模板规则；语义改写留给模型接入后）。
fn decompose(topic: &str, max: usize) -> Vec<SubQuestion> {
    const TEMPLATES: [&str; 5] = [
        "「{topic}」的核心概念与定义是什么？",
        "「{topic}」有哪些主流方案或做法？",
        "「{topic}」的适用场景与限制条件是什么？",
        "「{topic}」常见的争议、风险或失败案例有哪些？",
        "近两年「{topic}」有什么值得关注的新进展？",
    ];
    let count = max.clamp(1, TEMPLATES.len());
    TEMPLATES
        .iter()
        .take(count)
        .enumerate()
        .map(|(index, template)| SubQuestion {
            id: format!("q{}", index + 1),
            question: template.replace("{topic}", topic),
            keywords: vec![topic.to_string()],
            local_hits: 0,
        })
        .collect()
}

/// 「冲突」判定的正/负面词表（D4 规则档；模型/Jev 接入后换成语义比对）。
const POSITIVE_MARKERS: [&str; 9] = [
    "支持", "可以", "推荐", "适用", "可行", "优势", "有效", "最佳", "建议使用",
];
const NEGATIVE_MARKERS: [&str; 11] = [
    "不支持", "不能", "无法", "禁止", "避免", "不要", "风险", "缺陷", "失败", "限制", "问题",
];

/// 表述极性：>0 偏正面，<0 偏负面，0 中立。
fn polarity(text: &str) -> i32 {
    let lowered = text.to_lowercase();
    let positive = POSITIVE_MARKERS
        .iter()
        .filter(|marker| lowered.contains(*marker))
        .count() as i32;
    let negative = NEGATIVE_MARKERS
        .iter()
        .filter(|marker| lowered.contains(*marker))
        .count() as i32;
    positive - negative
}

/// 同一子问题下的来源是否**互相矛盾**（既有正面表述、也有负面表述）。
///
/// 这是 D4 的规则档：只在「同一子问题」内比较，避免把不同角度的材料误判成冲突；
/// 判定结果只影响**呈现方式**（并列进第 3 段 + 醒目警告），不影响来源本身的级别。
fn has_polarity_conflict(sources: &[&ResearchSource]) -> bool {
    let polarities: Vec<i32> = sources
        .iter()
        .map(|source| polarity(&format!("{} {}", source.title, source.snippet)))
        .collect();
    polarities.iter().any(|value| *value > 0) && polarities.iter().any(|value| *value < 0)
}

/// 规则归属：按「意图标记」命中数把材料分给某个子问题（全不命中 → 第一个）。
fn assign_sub_question(title: &str, snippet: &str, questions: &[SubQuestion]) -> usize {
    if questions.is_empty() {
        return 0;
    }
    let haystack = format!("{title} {snippet}").to_lowercase();
    let mut best = 0usize;
    let mut best_score = 0usize;
    for (index, _) in questions.iter().enumerate() {
        let markers = INTENT_MARKERS.get(index).copied().unwrap_or(&[]);
        let score = markers
            .iter()
            .filter(|marker| haystack.contains(&marker.to_lowercase()))
            .count();
        if score > best_score {
            best_score = score;
            best = index;
        }
    }
    best
}

/// 验证阶段的中间报告：结论/事实/待验证已定，报告文字与警告留给 `compose`。
fn pending_report(
    session: &ResearchSession,
    claims: Vec<ResearchClaim>,
    conflicting: &[String],
    decision_provider: &str,
    conflict_provider: &str,
    decision_trace: Option<DecisionTrace>,
) -> ResearchReport {
    let mut seen: Vec<Vec<usize>> = Vec::new();
    let mut conclusions = Vec::new();
    let mut key_facts = Vec::new();
    let mut pending = Vec::new();
    let mut conflicts = Vec::new();
    for claim in claims {
        // 有冲突的子问题只进第 3 段：并列呈现、不选边（§9.3）
        if conflicting.contains(&claim.sub_question_id) {
            conflicts.push(claim);
            continue;
        }
        if !claim.cross_verified {
            pending.push(claim);
            continue;
        }
        if seen.contains(&claim.citations) {
            continue; // 同一组证据支撑多个子问题：只保留一条，避免结论段重复
        }
        seen.push(claim.citations.clone());
        if claim.fresh && conclusions.len() < MAX_CONCLUSIONS {
            conclusions.push(claim.clone());
        }
        if key_facts.len() < MAX_KEY_FACTS {
            key_facts.push(claim);
        }
    }
    ResearchReport {
        generated_at: now_iso(),
        data_cutoff: today_date(),
        // 证据充分性由 compose 在定稿前算（此处保持默认，避免"先写后算"两处漂移）
        evidence: Default::default(),
        composer: TemplateGenerativeModel::new().id().to_string(),
        decision_provider: decision_provider.to_string(),
        conflict_provider: Some(conflict_provider.to_string()),
        decision_trace,
        conclusions,
        key_facts,
        conflicts,
        kb_relations: kb_relations(session),
        pending,
        references: session.sources.clone(),
        speculation: Vec::new(),
        warnings: Vec::new(),
        source_reliability: session
            .sources
            .iter()
            .map(|source| source.reliability)
            .min()
            .unwrap_or(1),
        valid_until: None,
        markdown: String::new(),
    }
}

/// 报告第 4 段：与本地知识库的关系（基线笔记 + 证据笔记）。
fn kb_relations(session: &ResearchSession) -> Vec<KbRelation> {
    let mut relations: Vec<KbRelation> = Vec::new();
    let evidence: Vec<String> = session
        .sources
        .iter()
        .filter_map(|source| source.note_id.clone())
        .collect();
    if let Some(plan) = &session.plan {
        for baseline in &plan.local_baseline {
            let relation = if evidence.contains(&baseline.note_id) {
                "补充（已作为证据）"
            } else {
                "相关（本地已有笔记）"
            };
            relations.push(KbRelation {
                note_id: baseline.note_id.clone(),
                title: baseline.title.clone(),
                relation: relation.to_string(),
            });
        }
    }
    for source in &session.sources {
        if let Some(note_id) = &source.note_id {
            if relations.iter().any(|item| &item.note_id == note_id) {
                continue;
            }
            relations.push(KbRelation {
                note_id: note_id.clone(),
                title: source.title.clone(),
                relation: "证据来源".to_string(),
            });
        }
    }
    relations
}

/// 生成醒目警告块（§9.3 / §9.4：未验证、弱来源、冲突必须醒目，且不是脚注）。
fn collect_warnings(
    session: &ResearchSession,
    report: &ResearchReport,
    network_allowed: bool,
    model_generated: bool,
) -> Vec<ResearchWarning> {
    let mut warnings = Vec::new();
    let weak: Vec<usize> = session
        .sources
        .iter()
        .filter(|source| source.level == SourceLevel::E)
        .map(|source| source.reference)
        .collect();
    if !weak.is_empty() {
        warnings.push(ResearchWarning {
            kind: WarningKind::UnverifiedSources,
            message: format!(
                "有 {} 条来源属于「E 不可用」（论坛贴 / 社交平台 / AI 生成内容）：只作线索，未进入结论段。",
                weak.len()
            ),
            references: weak,
        });
    }
    let single: Vec<usize> = report
        .pending
        .iter()
        .flat_map(|claim| claim.citations.clone())
        .collect();
    if !report.conflicts.is_empty() {
        warnings.push(ResearchWarning {
            kind: WarningKind::Conflict,
            message: format!(
                "有 {} 个子问题的来源互相矛盾：已并列呈现在「争议与不确定性」段，本次不替你选边（规则判定：同一子问题下同时出现支持与反对/限制表述）。",
                report.conflicts.len()
            ),
            references: report
                .conflicts
                .iter()
                .flat_map(|claim| claim.citations.clone())
                .collect(),
        });
    }
    if !single.is_empty() {
        warnings.push(ResearchWarning {
            kind: WarningKind::SingleSource,
            message: format!(
                "有 {} 条来源是单一来源、没有交叉验证：只列在「待验证清单」，不进结论段。",
                single.len()
            ),
            references: single,
        });
    }
    let stale: Vec<usize> = session
        .sources
        .iter()
        .filter(|source| source.stale)
        .map(|source| source.reference)
        .collect();
    if !stale.is_empty() {
        warnings.push(ResearchWarning {
            kind: WarningKind::Stale,
            message: format!(
                "有 {} 条来源可能已过时（超出时间窗或 valid_until 已过）：请核对时效后再引用。",
                stale.len()
            ),
            references: stale,
        });
    }
    if session.sources.is_empty() {
        warnings.push(ResearchWarning {
            kind: WarningKind::NoLocalEvidence,
            message: "本地知识库没有找到任何材料：本报告只有计划与建议，没有结论。".to_string(),
            references: Vec::new(),
        });
    }
    let has_web_source = session
        .sources
        .iter()
        .any(|source| source.origin == SourceOrigin::Web);
    if !session.dropped_sources.is_empty() {
        let blacklisted = session
            .dropped_sources
            .iter()
            .filter(|item| item.reason == "denylist")
            .count();
        let hosts: Vec<String> = session
            .dropped_sources
            .iter()
            .map(|item| item.host.clone())
            .take(3)
            .collect();
        warnings.push(ResearchWarning {
            kind: WarningKind::DroppedSources,
            message: format!(
                "有 {} 条检索结果被域名策略丢弃（黑名单 {} 条 / 不在白名单 {} 条）：{}——它们没有进入来源列表，也不会进入结论。",
                session.dropped_sources.len(),
                blacklisted,
                session.dropped_sources.len() - blacklisted,
                hosts.join("、")
            ),
            references: Vec::new(),
        });
    }
    if network_allowed && !has_web_source {
        warnings.push(ResearchWarning {
            kind: WarningKind::ExternalNotAvailable,
            message:
                "本次没有外部来源：可能是没有确认联网、没有配置检索源地址，或检索没有结果（可在「设置 → AI」检查）。"
                    .to_string(),
            references: Vec::new(),
        });
    }
    warnings.push(if model_generated {
        // 「在线」这一档必须**明说内容已离开本机**：这是隐私上唯一不能含糊的地方
        ResearchWarning {
            kind: WarningKind::ModelGenerated,
            message: model_generated_message(&report.composer),
            references: Vec::new(),
        }
    } else {
        ResearchWarning {
            kind: WarningKind::TemplateGenerated,
            message:
                "本报告由**模板实现**生成（未接入模型）：结论段是来源原句摘录，推测段只是待确认方向的罗列。"
                    .to_string(),
            references: Vec::new(),
        }
    });
    warnings
}

/// 「模型生成」的人读说明：在线模型必须**明说材料已经离开本机**（本地模型则只提示未核对）。
///
/// 两种模型对隐私的含义完全不同（一个只在本机算，一个把片段发到了服务商），
/// 报告里不能写成同一句话。
pub(crate) fn model_generated_message(composer: &str) -> String {
    if composer.starts_with("remote.") {
        format!(
            "推测段由**在线模型** `{composer}` 生成：材料片段已离开本机、可能被服务商留存；\
             内容未经人工核对，也未经交叉验证——结论段仍是来源原句摘录。"
        )
    } else {
        format!(
            "推测段由**本地模型** `{composer}` 生成（未经人工核对，也未经交叉验证）：\
             结论段仍是来源原句摘录。"
        )
    }
}

/// 渲染固定六段的 Markdown（顺序不可变，便于测试与用户形成预期）。
fn render_markdown(session: &ResearchSession, plan: &ResearchPlan, report: &ResearchReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("# 调研报告：{}\n\n", plan.topic));
    // 只有真的没接模型时才写「未接入模型」——不能让一份模型写的报告顶着模板的名头
    let composer_note = if report.composer == TemplateGenerativeModel::new().id() {
        "（未接入模型）"
    } else {
        ""
    };
    out.push_str(&format!(
        "> **资料截至 {}** · 生成方式 `{}`{} · 打分 `{}`\n",
        report.data_cutoff, report.composer, composer_note, report.decision_provider
    ));
    out.push_str(&format!(
        "> 会话 `{}` · 子问题 {} 个 · 来源 {} 条\n\n",
        session.id,
        plan.sub_questions.len(),
        report.references.len()
    ));

    // D5/N9：证据充分性写在最前面。默认的失败方式不是答错，而是**材料不够还硬答**——
    // 用户看报告时第一眼就该知道"这份东西的底子有多厚"。
    if report.evidence.total_sub_questions > 0 || !report.evidence.gaps.is_empty() {
        let evidence = &report.evidence;
        out.push_str(&format!(
            "> **证据充分性：{}** · 覆盖 {}/{} 个子问题 · 独立来源 {} 个 · 可用来源 {} 条\n",
            if evidence.sufficient { "充分" } else { "不足" },
            evidence.covered_sub_questions,
            evidence.total_sub_questions,
            evidence.independent_sources,
            evidence.usable_sources
        ));
        if !evidence.sufficient {
            for gap in &evidence.gaps {
                out.push_str(&format!("> - ⛔ {}\n", gap.message));
            }
            if !evidence.recommendation.trim().is_empty() {
                out.push_str(&format!("> - 建议：{}\n", evidence.recommendation.trim()));
            }
        }
        out.push('\n');
    }

    if !report.warnings.is_empty() {
        out.push_str("> **⚠️ 警告**\n");
        for warning in &report.warnings {
            let refs = if warning.references.is_empty() {
                String::new()
            } else {
                format!(
                    "（{}）",
                    warning
                        .references
                        .iter()
                        .map(|value| format!("[{value}]"))
                        .collect::<Vec<_>>()
                        .join("")
                )
            };
            out.push_str(&format!("> - {}{}\n", warning.message, refs));
        }
        out.push('\n');
    }

    out.push_str("## 1. 结论先行\n\n");
    if report.conclusions.is_empty() {
        if report.evidence.sufficient {
            out.push_str("_本次没有达到「≥2 个独立来源」的结论。_\n\n");
        } else {
            // 关键区别：不是"碰巧没有"，而是"材料不够，所以故意不给"
            out.push_str(
                "_证据不足，本次**不给结论**（不是「查过没有」）——缺口与建议见报告顶部。_\n\n",
            );
        }
    } else {
        for (index, claim) in report.conclusions.iter().enumerate() {
            out.push_str(&format!(
                "{}. {} {} （置信度 {}）\n",
                index + 1,
                claim.text,
                citations_of(claim),
                confidence_label(claim.confidence)
            ));
        }
        out.push('\n');
    }

    out.push_str("## 2. 关键事实\n\n");
    if report.key_facts.is_empty() {
        out.push_str("_没有交叉验证过的事实。_\n\n");
    } else {
        out.push_str(
            "| 结论 | 支持来源（级别） | 是否交叉验证 | 时效 | 置信度 |\n| --- | --- | --- | --- | --- |\n",
        );
        for claim in &report.key_facts {
            let levels = report
                .references
                .iter()
                .filter(|source| claim.citations.contains(&source.reference))
                .map(|source| format!("[{}] {}", source.reference, source.level.label()))
                .collect::<Vec<_>>()
                .join("、");
            out.push_str(&format!(
                "| {} | {} | {} | {} | {}（{:.2}） |\n",
                escape_table(&claim.text),
                escape_table(&levels),
                if claim.cross_verified { "是" } else { "否" },
                if claim.fresh { "时间内" } else { "可能已过时" },
                confidence_label(claim.confidence),
                claim.confidence
            ));
        }
        out.push('\n');
    }

    out.push_str("## 3. 争议与不确定性\n\n");
    if report.conflicts.is_empty() {
        out.push_str(
            "_本次未检出冲突（规则档：只有同一子问题下同时出现支持与反对/限制表述才会判为冲突）。_\n\n",
        );
    } else {
        for claim in &report.conflicts {
            out.push_str(&format!("- {} {}\n", claim.text, citations_of(claim)));
        }
        out.push('\n');
    }

    out.push_str("## 4. 与本知识库的关系\n\n");
    if report.kb_relations.is_empty() {
        out.push_str("_没有关联的本地笔记。_\n\n");
    } else {
        for relation in &report.kb_relations {
            out.push_str(&format!("- [[{}]] —— {}\n", relation.title, relation.relation));
        }
        out.push('\n');
    }

    out.push_str("## 5. 待验证清单\n\n");
    if report.pending.is_empty() {
        out.push_str("_没有待验证项。_\n\n");
    } else {
        for claim in &report.pending {
            // 引用里只要还有可用来源，就是「单一来源」；全都是 E 级时如实说明「不可用线索」
            let why = if claim.citations.is_empty() {
                "本地没有材料：建议补充或（阶段 D3 起）联网检索".to_string()
            } else if claim.citations.iter().all(|reference| {
                report
                    .references
                    .iter()
                    .find(|source| source.reference == *reference)
                    .is_none_or(|source| source.level == SourceLevel::E)
            }) {
                "只有不可用来源（E 级）作线索，建议补充可信来源".to_string()
            } else {
                "只有单一来源，未交叉验证".to_string()
            };
            out.push_str(&format!("- {} {} —— {}\n", claim.text, citations_of(claim), why));
        }
        out.push('\n');
    }

    out.push_str("## 6. 参考来源\n\n");
    if report.references.is_empty() {
        out.push_str("_没有参考来源。_\n");
    } else {
        for source in &report.references {
            let published = source.published_at.as_deref().unwrap_or("时效未知");
            out.push_str(&format!(
                "[{}] {} · {} · published {} · 获取 {} · {}（{}）\n",
                source.reference,
                source.title,
                source.level.label(),
                published,
                source.fetched_at,
                source_location(source),
                source.grade_reason
            ));
        }
    }
    out
}

fn citations_of(claim: &ResearchClaim) -> String {
    claim
        .citations
        .iter()
        .map(|value| format!("[{value}]"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 来源在报告里的**可点开**定位（§9.9 验收 1）。
///
/// - 本地笔记写成 `[[标题]]`：预览里渲染成可点击的链接芯片，点击直接打开该笔记，
///   同时形成一条真实的反向链接（此前写 `note://<id>`，预览会把它当外部地址交给系统打开，点不开）；
/// - 外部来源写**裸 URL**：Markdown 预览会自动识别成链接；
/// - 都没有（理论上不该出现）时退回「本地」。
fn source_location(source: &ResearchSource) -> String {
    if let Some(url) = source.url.as_deref().filter(|value| !value.trim().is_empty()) {
        return url.to_string();
    }
    if let Some(_note_id) = source.note_id.as_deref().filter(|value| !value.trim().is_empty()) {
        return format!("[[{}]]", escape_wikilink_text(&source.title));
    }
    "本地".to_string()
}

/// 写进 `[[…]]` 的标题要转义 wikilink 的保留字符（与 `core/text.rs` 的解析规则对称）。
fn escape_wikilink_text(value: &str) -> String {
    value.replace('|', "\\|").replace('#', "\\#").replace(']', "\\]")
}

fn escape_table(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

fn normalize_title_key(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join("").to_lowercase()
}

/// 落库正文：提案正文 +（可选）附录形式的「模型推测」段。
///
/// 六段报告结构不变；推测段只在用户显式勾选时追加，且作为**附录**、明确标注是谁生成的。
fn write_body(
    proposal: &ResearchProposal,
    session: &ResearchSession,
    include_speculation: bool,
) -> String {
    let mut body = proposal.body.clone();
    if !include_speculation {
        return body;
    }
    let Some(report) = session.report.as_ref() else {
        return body;
    };
    if report.speculation.is_empty() {
        return body;
    }
    // 附录里的来源必须与报告主体一致：模板就说模板，模型就说清是本地还是**在线**
    let origin = if report.composer == TemplateGenerativeModel::new().id() {
        "以下内容由**模板实现**生成（未接入模型），不是结论，仅供参考。".to_string()
    } else if report.composer.starts_with("remote.") {
        format!(
            "以下内容由**在线模型** `{}` 生成（材料片段已离开本机、可能被服务商留存），不是结论，仅供参考。",
            report.composer
        )
    } else {
        format!(
            "以下内容由**本地模型** `{}` 生成（未经人工核对），不是结论，仅供参考。",
            report.composer
        )
    };
    body.push_str(&format!("\n\n## 附：模型推测（未验证）\n\n> {origin}\n\n"));
    for line in &report.speculation {
        body.push_str(line);
        body.push('\n');
    }
    body
}

/// 新建模式的 diff 预览：全部是新增行。
fn create_diff(body: &str) -> String {
    body.lines().map(|line| format!("+{line}")).collect::<Vec<_>>().join("\n")
}

/// 追加模式的 diff 预览：最后 3 行上下文（空格前缀）+ 新增行。
fn append_diff(existing: &str, body: &str) -> String {
    let mut out: Vec<String> = existing
        .lines()
        .filter(|line| !line.trim().is_empty())
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|line| format!(" {line}"))
        .collect();
    for line in body.lines() {
        out.push(format!("+{line}"));
    }
    out.join("\n")
}

/// 「来源清单」子文档的正文：每行一条来源（级别 + 获取时间 + 定位 + 分级理由）。
fn source_note_markdown(report: &ResearchReport) -> String {
    let mut out = String::from(
        "> 本清单由研究助手生成：每行是报告中引用过的一条来源，含分级依据与获取时间。\n\n",
    );
    out.push_str(&format!("> 资料截至 {}。\n\n", report.data_cutoff));
    for source in &report.references {
        out.push_str(&format!(
            "- [{}] {} · {} · 获取 {} · {} · {}\n",
            source.reference,
            source.title,
            source.level.label(),
            source.fetched_at,
            source_location(source),
            source.grade_reason
        ));
    }
    out
}

/// 把来源列表转成 frontmatter 用的引用串（写回时使用）。
pub fn source_ref_of(report: &ResearchReport) -> String {
    report
        .references
        .iter()
        .map(|source| match source.url.clone() {
            Some(url) => format!("[{}] {} {}", source.reference, source.title, url),
            None => format!("[{}] {} ({} 笔记)", source.reference, source.title, source.level.label()),
        })
        .collect::<Vec<_>>()
        .join("；")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_validation_rejects_broad_or_empty_topics() {
        assert!(validate_topic("").is_err());
        assert!(validate_topic("AI").is_err(), "少于 4 个字符必须要求补充范围");
        assert!(validate_topic("知识库").is_err(), "宽泛词表内的课题要追问范围");
        assert!(validate_topic(&"字".repeat(201)).is_err());
        assert_eq!(
            validate_topic("  Rust 异步运行时调度  ").expect("正常课题应通过"),
            "Rust 异步运行时调度"
        );
    }

    #[test]
    fn decomposition_respects_budget_and_numbering() {
        let questions = decompose("Rust 异步运行时调度", 5);
        assert_eq!(questions.len(), 5);
        assert_eq!(questions[0].id, "q1");
        assert!(questions[0].question.contains("核心概念"));
        assert!(decompose("x", 1).len() == 1);
        assert!(decompose("x", 99).len() <= 5, "硬上限 5 个");
    }

    #[test]
    fn assignment_prefers_intent_markers_then_falls_back_to_first() {
        let questions = decompose("测试课题", 5);
        assert_eq!(assign_sub_question("核心概念与定义", "", &questions), 0);
        assert_eq!(assign_sub_question("主流方案对比", "", &questions), 1);
        assert_eq!(assign_sub_question("适用场景与限制条件", "", &questions), 2);
        assert_eq!(assign_sub_question("常见风险与失败案例", "", &questions), 3);
        assert_eq!(assign_sub_question("2026 年最新进展", "", &questions), 4);
        assert_eq!(assign_sub_question("毫不相关的标题", "", &questions), 0);
    }

    #[test]
    fn title_key_normalizes_case_and_whitespace() {
        assert_eq!(normalize_title_key(" 调研报告：Rust "), "调研报告：rust");
    }

    #[test]
    fn polarity_and_conflict_detection_are_conservative() {
        assert!(polarity("推荐使用 tokio 的实现") > 0);
        assert!(polarity("不建议使用，存在风险与限制") < 0);
        assert_eq!(polarity("本章介绍调度器的实现细节"), 0);

        let positive = source("https://docs.rs/a", "调度方案推荐", "推荐使用工作窃取队列");
        let negative = source("https://example.dev/b", "调度方案的风险", "存在风险，避免阻塞调用");
        let neutral = source("https://example.dev/c", "调度器简介", "本章介绍实现细节");
        assert!(has_polarity_conflict(&[&positive, &negative]), "正负对立要判为冲突");
        assert!(!has_polarity_conflict(&[&positive, &neutral]), "只有一面不判冲突");
        assert!(!has_polarity_conflict(&[&positive]), "单条来源不可能冲突");
    }

    fn source(url: &str, title: &str, snippet: &str) -> ResearchSource {
        ResearchSource {
            reference: 1,
            title: title.to_string(),
            url: Some(url.to_string()),
            origin: SourceOrigin::Web,
            note_id: None,
            sub_question_id: "q1".to_string(),
            level: SourceLevel::C,
            reliability: 3,
            grade_reason: "测试".to_string(),
            published_at: None,
            fetched_at: "2026-10-11T10:00:00+08:00".to_string(),
            applicable_version: None,
            snippet: snippet.to_string(),
            char_start: None,
            char_end: None,
            heading: None,
            stale: false,
            credibility: 0.6,
        }
    }

    #[test]
    fn credibility_blends_model_score_with_graded_level() {
        // 模型分与级别分各取一半：级别 5/5 → 1.0，模型 0 分也保底 0.5
        assert!((blended_credibility(Some(0.0), 5) - 0.5).abs() < 1e-6);
        assert!((blended_credibility(Some(1.0), 5) - 1.0).abs() < 1e-6);
        assert!((blended_credibility(None, 1) - 0.2).abs() < 1e-6, "无模型分时按级别折算");
        // 越界输入夹取到 0–1
        assert!((blended_credibility(Some(9.0), 5) - 1.0).abs() < 1e-6);
        assert!((blended_credibility(Some(-3.0), 1) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn claim_confidence_rewards_independent_sources() {
        let single = claim_confidence(&[0.9], 1);
        let cross = claim_confidence(&[0.9, 0.9], 2);
        assert!(cross > single, "交叉验证应当提高置信度：{single} vs {cross}");
        assert!((single - 0.54).abs() < 1e-6, "单一来源打 6 折");
        assert!((cross - 0.9).abs() < 1e-6);
        assert_eq!(claim_confidence(&[], 0), 0.0, "没有来源就没有置信度");
        assert_eq!(confidence_label(0.75), "高");
        assert_eq!(confidence_label(0.5), "中");
        assert_eq!(confidence_label(0.1), "低");
    }
}
