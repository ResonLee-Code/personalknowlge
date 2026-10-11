//! 研究助手集成测试（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §9.1–§9.9）。
//!
//! 覆盖 §9.9 的可测验收标准：
//! 1. 结论段每一条都能落到引用（note_id / url + 获取时间）；
//! 2. 单一来源的结论**不出现在**结论段与关键事实段；
//! 3. 默认口径下**没有任何外发**（审计里不存在 `outbound = true`）；
//! 4. 写入提案默认落 Inbox、同标题给出 duplicate、写前有 diff；
//! 5. 报告顶部固定「资料截至 YYYY-MM-DD」；
//! 6. 弱来源 / 单一来源 / 模板生成三类警告在报告里可见。

use std::sync::Arc;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::research::ResearchService;
use personal_knowledge_os_lib::application::ai::traits::{
    DecisionKind, DecisionModel, DecisionResult, DecisionState,
};
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::core::time::today_date;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SetSourceInput};
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::infrastructure::frontmatter;
use personal_knowledge_os_lib::domain::research::{
    ResearchRequest, ResearchSession, ResearchStatus, ResearchWriteInput, SourceLevel, WarningKind,
    WRITE_MODE_ABANDON, WRITE_MODE_APPEND, WRITE_MODE_CREATE,
};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    _data: TempDir,
    workspace: TempDir,
    app: Arc<AppInner>,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("研究助手测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn research(&self) -> ResearchService {
        ResearchService::new(self.context())
    }

    fn policy(&self) -> AiPolicyService {
        AiPolicyService::new(self.context())
    }

    fn enable_ai(&self) {
        self.policy()
            .update(UpdateAiPolicyInput { enabled: Some(true), ..Default::default() })
            .expect("打开 AI 总开关");
    }

    /// 建一篇带来源标注的笔记，返回 id。
    fn note(&self, title: &str, content: &str, reliability: Option<i64>) -> String {
        let notes = NoteService::new(self.context());
        let detail = notes
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(content.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("新建笔记");
        if let Some(level) = reliability {
            notes
                .set_source(SetSourceInput {
                    id: detail.note.id.clone(),
                    source_type: Some("official".to_string()),
                    source_ref: None,
                    source_reliability: Some(level),
                    valid_until: None,
                })
                .expect("标注来源");
        }
        detail.note.id
    }
}

/// 桩决策模型：把每条候选的分数固定成 `score`，用于验证「换 provider 就换打分」这条 Jev 插槽。
struct FixedScoreModel {
    score: f32,
}

impl DecisionModel for FixedScoreModel {
    fn id(&self) -> &'static str {
        "test.fixed"
    }

    fn is_local(&self) -> bool {
        true
    }

    fn decide(
        &self,
        kind: DecisionKind,
        state: &DecisionState,
    ) -> personal_knowledge_os_lib::core::error::Result<Vec<DecisionResult>> {
        Ok(state
            .candidates
            .iter()
            .map(|candidate| DecisionResult {
                note_id: candidate.note_id.clone(),
                choice: personal_knowledge_os_lib::application::ai::traits::DecisionChoice::Use,
                score: self.score,
                confidence: 1.0,
                reason: format!("桩模型（{kind:?}）固定打分"),
                kind,
            })
            .collect())
    }
}

/// 一个「方案」类主题的知识库：两条独立笔记指向同一子问题，另外两条各指向不同子问题。
fn seed(fixture: &Fixture) {
    fixture.note(
        "Rust 异步运行时调度方案对比",
        "主流方案是对比 tokio 与 async-std 的调度实现，方案差异集中在工作窃取队列。",
        Some(4),
    );
    fixture.note(
        "Rust 异步调度实现要点",
        "实现要点在于任务唤醒与本地队列的批量窃取，方案上可参考 tokio 的实现。",
        Some(5),
    );
    fixture.note(
        "Rust 异步调度的限制与注意事项",
        "限制条件：阻塞调用会拖垮运行时；注意不要让 CPU 密集任务占用调度线程。",
        Some(3),
    );
    fixture.note(
        "Rust 异步调度风险与踩坑",
        "风险在于误用 spawn_blocking；失败案例常见于把同步锁带进异步任务。",
        None,
    );
}

fn run_until(fixture: &Fixture, request: ResearchRequest, steps: &[&str]) -> ResearchSession {
    let research = fixture.research();
    let mut session = research.start(request).expect("立项");
    for step in steps {
        session = match *step {
            "collect" => research.collect(&session.id, false).expect("收集"),
            "verify" => research.verify(&session.id).expect("验证"),
            "compose" => research.compose(&session.id).expect("汇总"),
            "proposal" => research.proposal(&session.id).expect("写入提案"),
            other => panic!("未知步骤 {other}"),
        };
    }
    session
}

fn request(topic: &str) -> ResearchRequest {
    ResearchRequest { topic: topic.to_string(), ..Default::default() }
}

#[test]
fn research_needs_the_ai_gate_and_a_specific_topic() {
    let fixture = Fixture::new();
    seed(&fixture);
    let research = fixture.research();

    // 总开关关闭：任何一步都被闸门挡住（默认最保守）
    let blocked = research
        .start(request("Rust 异步运行时调度"))
        .expect_err("未开启 AI 应被拒绝");
    assert_eq!(blocked.code.as_str(), "AI_DISABLED");

    fixture.enable_ai();
    let broad = research.start(request("知识库")).expect_err("宽泛课题应要求补充范围");
    assert_eq!(broad.code.as_str(), "INVALID_INPUT");
    assert!(broad.message.contains("过于宽泛"), "{}", broad.message);

    // 步骤顺序不可跳过：没收集就不能验证
    let session = research.start(request("Rust 异步运行时调度")).expect("立项");
    assert_eq!(session.status, ResearchStatus::Planned);
    assert_eq!(
        session.plan.as_ref().expect("计划").sub_questions.len(),
        5,
        "默认拆 5 个子问题（预算上限）"
    );
    let premature = research.verify(&session.id).expect_err("未收集不能验证");
    assert_eq!(premature.code.as_str(), "INVALID_INPUT");
}

#[test]
fn local_research_produces_a_six_section_report_without_any_outbound_call() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    seed(&fixture);

    let session = run_until(
        &fixture,
        request("Rust 异步运行时调度"),
        &["collect", "verify", "compose"],
    );
    assert_eq!(session.status, ResearchStatus::Composed);

    // 收集：只采纳本地材料
    assert!(!session.sources.is_empty(), "本地应找到材料");
    assert!(
        session.sources.iter().all(|source| source.note_id.is_some()),
        "D1/D2 只允许本地来源"
    );
    assert!(
        session.sources.iter().all(|source| !source.grade_reason.is_empty()),
        "每条来源都要写明分级依据（可复核，不是黑盒）"
    );

    let report = session.report.as_ref().expect("报告");
    // 验收 5：报告顶部固定「资料截至 YYYY-MM-DD」
    assert!(
        report.markdown.contains(&format!("资料截至 {}", today_date())),
        "报告抬头必须写资料截止日期"
    );

    // 验收 1：结论段每一条都能落到引用
    for claim in &report.conclusions {
        assert!(claim.cross_verified, "结论段只放交叉验证过的内容");
        assert!(!claim.citations.is_empty(), "结论必须带引用");
        for reference in &claim.citations {
            let source = report
                .references
                .iter()
                .find(|source| source.reference == *reference)
                .unwrap_or_else(|| panic!("引用 [{reference}] 必须能在参考来源里找到"));
            assert!(
                source.note_id.is_some() || source.url.is_some(),
                "引用必须能定位（本地 noteId 或外部 URL）"
            );
            assert!(!source.fetched_at.is_empty(), "引用必须有获取/更新时间");
        }
    }

    // 验收 1（可点开）：定位必须写成预览里点得动的形式——
    // 本地来源 `[[标题]]`（点击打开笔记），外部来源裸 URL（自动成链）。
    // `note://<id>` 会被预览当成外部地址交给系统打开，点了打不开，因此不得再出现。
    assert!(
        !report.markdown.contains("note://"),
        "报告里不得再用点不开的 note:// 定位：\n{}",
        report.markdown
    );
    // 只看第 6 段：第 4 段「与本知识库的关系」本来就有 `[[笔记]]`，
    // 若整篇 contains 会被它蒙混过关（这是个很容易写出假绿的地方）。
    let references_section = report
        .markdown
        .split("## 6. 参考来源")
        .nth(1)
        .unwrap_or_else(|| panic!("报告必须有第 6 段参考来源：\n{}", report.markdown));
    for source in &report.references {
        if source.note_id.is_some() {
            assert!(
                references_section.contains(&format!("[[{}]]", source.title)),
                "本地来源必须写成可点击的 [[标题]]：{}\n{}",
                source.title,
                references_section
            );
        }
    }

    // 验收 2：单一来源的结论不得出现在第 1、2 段
    let single: Vec<String> = report
        .pending
        .iter()
        .filter(|claim| claim.citations.len() == 1)
        .map(|claim| claim.sub_question_id.clone())
        .collect();
    assert!(!single.is_empty(), "本用例应至少有一条单一来源的待验证项");
    for claim in report.conclusions.iter().chain(report.key_facts.iter()) {
        assert!(
            !single.contains(&claim.sub_question_id),
            "单一来源的子问题不得进入结论/关键事实段"
        );
        assert!(claim.cross_verified, "结论与关键事实必须交叉验证过");
    }

    // 验收 6：弱来源 / 单一来源 / 模板生成三类警告可见
    let kinds: Vec<WarningKind> = report.warnings.iter().map(|warning| warning.kind).collect();
    assert!(kinds.contains(&WarningKind::SingleSource), "{kinds:?}");
    assert!(kinds.contains(&WarningKind::TemplateGenerated), "{kinds:?}");
    assert!(report.markdown.contains("⚠️ 警告"), "警告块必须在报告里可见");
    assert!(
        report.markdown.contains("未检出冲突"),
        "第 3 段必须说明冲突检测的能力边界"
    );

    // 验收 3：默认口径（联网档位=仅本地）下没有任何外发
    let audit = fixture.policy().audit_tail(50).expect("审计");
    assert!(audit.iter().any(|entry| entry.capability == "research_collect"));
    assert!(
        audit.iter().all(|entry| !entry.outbound),
        "全程不得有 outbound=true 的记录"
    );
}

#[test]
fn proposal_lands_in_inbox_keeps_diff_and_never_overwrites_a_same_title_note() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    seed(&fixture);

    let session = run_until(
        &fixture,
        request("Rust 异步运行时调度"),
        &["collect", "verify", "compose", "proposal"],
    );
    assert_eq!(session.status, ResearchStatus::Proposed);
    let proposal = session.proposal.as_ref().expect("写入提案");

    // 验收 4：默认落 Inbox（未分类）、写前有 diff、同标题先给 duplicate
    assert!(proposal.category_id.is_none(), "默认落 Inbox（未分类）");
    assert!(
        proposal.diff_preview.lines().all(|line| line.starts_with('+')),
        "写前预览必须是新增 diff"
    );
    assert!(proposal.duplicates.is_empty(), "此时还没有同名笔记");
    assert!(proposal.tags.contains(&"调研报告".to_string()));
    assert_eq!(proposal.source_type, "research");
    assert!(proposal.source_reliability >= 1 && proposal.source_reliability <= 5);
    assert!(proposal.valid_until.is_some(), "研究产物必须带复核时间");

    // 同标题已有笔记 → 提案必须给出提示（**不覆盖**，由用户选择新建/追加/放弃）
    NoteService::new(fixture.context())
        .create(CreateNoteInput {
            title: proposal.title.clone(),
            category_id: None,
            initial_content: Some("已存在的同名笔记".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("新建同名笔记");
    let again = fixture.research().proposal(&session.id).expect("重新生成提案");
    let duplicates = again.proposal.as_ref().expect("提案").duplicates.clone();
    assert_eq!(duplicates.len(), 1, "同标题必须被识别出来：{duplicates:?}");
    assert_eq!(duplicates[0].title, proposal.title);
}

#[test]
fn sessions_are_persisted_and_can_be_aborted_with_materials_kept() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    seed(&fixture);

    let research = fixture.research();
    let mut session = research.start(request("Rust 异步运行时调度")).expect("立项");
    session = research.collect(&session.id, false).expect("收集");

    // 会话落盘：新开一个服务实例也能读回来
    let reloaded = fixture.research().get(&session.id).expect("读取会话");
    assert_eq!(reloaded.status, ResearchStatus::Collected);
    assert_eq!(reloaded.sources.len(), session.sources.len());
    assert!(fixture
        .workspace
        .path()
        .join(".knowledge")
        .join("research")
        .join(format!("{}.json", session.id))
        .exists());

    let aborted = research
        .abort(&session.id, Some("测试终止".to_string()))
        .expect("中止");
    assert_eq!(aborted.status, ResearchStatus::Aborted);
    assert_eq!(aborted.abort_reason.as_deref(), Some("测试终止"));
    assert!(!aborted.sources.is_empty(), "中止后已得材料仍作为草稿保留");
    let blocked = research.verify(&session.id).expect_err("中止后不能再推进");
    assert_eq!(blocked.code.as_str(), "INVALID_INPUT");

    let listed = research.list(10).expect("列表");
    assert!(listed.iter().any(|item| item.id == session.id));
}

#[test]
fn local_source_levels_follow_note_reliability_metadata() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    fixture.note(
        "Rust 异步调度实现要点（甲）",
        "方案上比较 tokio 的实现要点。",
        Some(5),
    );
    fixture.note(
        "Rust 异步调度实现要点（乙）",
        "方案上实现要点是任务窃取。",
        Some(2),
    );

    let session = run_until(
        &fixture,
        request("Rust 异步调度实现要点"),
        &["collect", "verify", "compose"],
    );
    let report = session.report.as_ref().expect("报告");
    let levels: Vec<SourceLevel> = report.references.iter().map(|source| source.level).collect();
    assert!(levels.contains(&SourceLevel::A), "reliability=5 → A：{levels:?}");
    assert!(levels.contains(&SourceLevel::D), "reliability=2 → D：{levels:?}");
    assert_eq!(
        report.source_reliability, 2,
        "报告整体可靠性按**最低级别**取整（最保守）"
    );
}

#[test]
fn write_back_creates_an_inbox_note_with_metadata_and_an_optional_source_note() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    seed(&fixture);

    let session = run_until(
        &fixture,
        request("Rust 异步运行时调度"),
        &["collect", "verify", "compose", "proposal"],
    );
    let notes = NoteService::new(fixture.context());
    let result = fixture
        .research()
        .write(ResearchWriteInput {
            session_id: session.id.clone(),
            mode: WRITE_MODE_CREATE.to_string(),
            include_speculation: true,
            create_source_note: true,
            ..Default::default()
        })
        .expect("写入");
    assert_eq!(result.outcome, "created");
    assert!(result.diff_preview.lines().all(|line| line.starts_with('+')));
    let note_id = result.note_id.clone().expect("落库笔记 id");

    let detail = notes.get(&note_id).expect("读回笔记");
    assert!(detail.note.category_id.is_none(), "默认落 Inbox（未分类）");
    assert_eq!(detail.note.source_type.as_deref(), Some("research"));
    assert!(detail.note.source_ref.as_deref().unwrap_or("").contains("[1]"));
    assert_eq!(detail.note.source_reliability, Some(result.source_reliability));
    assert!(detail.note.valid_until.is_some(), "研究产物必须带复核时间");
    assert!(detail.content.contains("资料截至"), "正文含报告抬头");
    assert!(
        detail.content.contains("附：模型推测（未验证）"),
        "勾选后才追加推测附录"
    );

    // 来源清单子文档：父文档指向报告
    let source_note_id = result.source_note_id.clone().expect("来源清单");
    let source_note = notes.get(&source_note_id).expect("读回来源清单");
    assert_eq!(source_note.note.parent_id.as_deref(), Some(note_id.as_str()));
    assert!(source_note.content.contains("分级依据") || source_note.content.contains("级别"));

    // 会话状态推进为 written，且审计里仍然没有任何外发
    let reloaded = fixture.research().get(&session.id).expect("读取会话");
    assert_eq!(reloaded.status, ResearchStatus::Written);
    assert_eq!(reloaded.written_note_id.as_deref(), Some(note_id.as_str()));
    let audit = fixture.policy().audit_tail(50).expect("审计");
    assert!(audit.iter().any(|entry| entry.capability == "research_write"));
    assert!(audit.iter().all(|entry| !entry.outbound));

    // 重复写入必须被拒绝（避免重复落库）
    let again = fixture
        .research()
        .write(ResearchWriteInput {
            session_id: session.id.clone(),
            mode: WRITE_MODE_CREATE.to_string(),
            ..Default::default()
        })
        .expect_err("已写入的会话不能再次落库");
    assert_eq!(again.code.as_str(), "INVALID_INPUT");
}

#[test]
fn append_mode_keeps_the_existing_note_and_leaves_a_rollback_snapshot() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    seed(&fixture);
    let notes = NoteService::new(fixture.context());

    // 先有一篇同标题笔记（正文非空），提案必须提示「同标题」
    let existing = notes
        .create(CreateNoteInput {
            title: "调研报告：Rust 异步运行时调度".to_string(),
            category_id: None,
            initial_content: Some("原有的结论：先看 tokio 文档。".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("新建同标题笔记")
        .note
        .id;

    let session = run_until(
        &fixture,
        request("Rust 异步运行时调度"),
        &["collect", "verify", "compose", "proposal"],
    );
    assert_eq!(
        session.proposal.as_ref().expect("提案").duplicates.len(),
        1,
        "提案必须先提示同标题"
    );

    let result = fixture
        .research()
        .write(ResearchWriteInput {
            session_id: session.id.clone(),
            mode: WRITE_MODE_APPEND.to_string(),
            target_note_id: Some(existing.clone()),
            ..Default::default()
        })
        .expect("追加");
    assert_eq!(result.outcome, "appended");
    assert_eq!(result.note_id.as_deref(), Some(existing.as_str()));
    assert!(
        result.diff_preview.lines().any(|line| line.starts_with(' ')),
        "追加模式的 diff 必须带上下文行"
    );

    let detail = notes.get(&existing).expect("读回笔记");
    assert!(detail.content.contains("原有的结论"), "追加不能覆盖原有正文");
    assert!(detail.content.contains("资料截至"), "新报告已追加");
    assert!(!detail.content.contains("附：模型推测"), "默认不写推测段");

    // 追加走 note_save → 自动留历史快照，可回滚
    let history = notes.history_list(&existing).expect("历史");
    assert!(!history.is_empty(), "追加前必须留下可回滚的快照");
}

#[test]
fn abandon_mode_writes_nothing_and_is_recorded() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    seed(&fixture);

    let session = run_until(
        &fixture,
        request("Rust 异步运行时调度"),
        &["collect", "verify", "compose", "proposal"],
    );
    let before = NoteService::new(fixture.context()).recent(100).expect("笔记数").len();
    let result = fixture
        .research()
        .write(ResearchWriteInput {
            session_id: session.id.clone(),
            mode: WRITE_MODE_ABANDON.to_string(),
            ..Default::default()
        })
        .expect("放弃");
    assert_eq!(result.outcome, "abandoned");
    assert!(result.note_id.is_none());

    let after = NoteService::new(fixture.context()).recent(100).expect("笔记数").len();
    assert_eq!(before, after, "放弃不得新建任何笔记");
    let reloaded = fixture.research().get(&session.id).expect("读取会话");
    assert_eq!(reloaded.status, ResearchStatus::Proposed, "放弃不改变状态");
    assert!(reloaded
        .steps
        .iter()
        .any(|step| step.step == "write" && step.detail.contains("放弃")));
}

#[test]
fn opposing_sources_are_presented_side_by_side_and_never_become_a_conclusion() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    // 同一子问题下的两条来源互相矛盾（一条偏支持、一条偏风险）
    fixture.note(
        "Rust 异步调度方案推荐实践",
        "推荐使用工作窃取队列，这种方案适用于高并发场景。",
        Some(4),
    );
    fixture.note(
        "Rust 异步调度方案的风险",
        "这种方案存在风险：误用会拖垮运行时，不建议在生产环境直接照搬。",
        Some(4),
    );

    let session = run_until(
        &fixture,
        request("Rust 异步调度方案"),
        &["collect", "verify", "compose"],
    );
    let report = session.report.as_ref().expect("报告");

    assert!(
        !report.conflicts.is_empty(),
        "对立表述必须被识别成冲突：{:?}",
        report.pending
    );
    let conflicted: Vec<String> = report
        .conflicts
        .iter()
        .map(|claim| claim.sub_question_id.clone())
        .collect();
    assert!(
        report
            .conclusions
            .iter()
            .chain(report.key_facts.iter())
            .all(|claim| !conflicted.contains(&claim.sub_question_id)),
        "有冲突的子问题不得进入结论段与关键事实段（不选边）"
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.kind == WarningKind::Conflict),
        "冲突必须在警告块里可见"
    );
    assert!(
        report.markdown.contains("不选边") || report.markdown.contains("争议"),
        "报告第 3 段必须并列呈现冲突：\n{}",
        report.markdown
    );
    assert!(
        !report.markdown.contains("本次未检出冲突"),
        "有冲突时不能显示「未检出冲突」占位文案"
    );
}

#[test]
fn decision_layer_scores_every_source_and_rewards_cross_verification() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    // 用「Rust 异步调度」做课题：三篇笔记都命中该短语，其中两篇落在同一子问题（可交叉验证）
    fixture.note(
        "Rust 异步调度方案对比",
        "主流方案是对比 tokio 与 async-std 的调度实现。",
        Some(4),
    );
    fixture.note(
        "Rust 异步调度实现要点",
        "实现要点在于任务唤醒与本地队列的批量窃取。",
        Some(5),
    );
    fixture.note(
        "Rust 异步调度的限制",
        "限制条件：阻塞调用会拖垮运行时，注意别占用调度线程。",
        Some(3),
    );

    let session = run_until(
        &fixture,
        request("Rust 异步调度"),
        &["collect", "verify", "compose"],
    );
    let report = session.report.as_ref().expect("报告");

    // 默认走规则档（§9.6 的降级路径），并如实写进报告
    assert_eq!(report.decision_provider, "rule");

    // 每条来源都要有 0–1 的可信度，且不低于「级别折算」的一半（级别是事实，模型分是参考）
    for source in &session.sources {
        assert!(
            source.credibility > 0.0 && source.credibility <= 1.0,
            "可信度必须落在 (0,1]：{} → {}",
            source.title,
            source.credibility
        );
        let floor = source.reliability as f32 / 10.0;
        assert!(
            source.credibility >= floor - 1e-6,
            "可信度不应低于级别折算的一半：{}",
            source.title
        );
    }

    // 交叉验证过的结论置信度高于单一来源的待验证项
    let cross = report
        .key_facts
        .iter()
        .map(|claim| claim.confidence)
        .fold(0.0_f32, f32::max);
    let single = report
        .pending
        .iter()
        .filter(|claim| claim.citations.len() == 1)
        .map(|claim| claim.confidence)
        .fold(0.0_f32, f32::max);
    assert!(cross > 0.0, "交叉验证的结论必须有置信度");
    assert!(single > 0.0, "单一来源也要给出置信度（低）");
    assert!(cross > single, "交叉验证应当更可信：{cross} vs {single}");

    // 报告里能读到置信度与打分来源
    assert!(report.markdown.contains("打分 `rule`"), "{}", report.markdown);
    assert!(report.markdown.contains("置信度"), "关键事实表要有置信度列");
}

#[test]
fn swapping_the_decision_model_changes_scores_without_touching_the_pipeline() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    fixture.note(
        "Rust 异步调度方案对比",
        "主流方案是对比 tokio 与 async-std 的调度实现。",
        Some(5),
    );
    fixture.note(
        "Rust 异步调度实现要点",
        "实现要点在于任务唤醒与本地队列的批量窃取。",
        Some(5),
    );

    // A：规则档（默认）
    let rule = run_until(&fixture, request("Rust 异步调度"), &["collect"]);
    let rule_score = rule.sources.first().expect("规则档来源").credibility;

    // B：注入桩模型（0 分）——同一条流水线，只换 provider
    let stubbed = ResearchService::with_decision_model(
        fixture.context(),
        Arc::new(FixedScoreModel { score: 0.0 }),
    );
    let started = stubbed
        .start(request("Rust 异步调度"))
        .expect("立项");
    let collected = stubbed
        .collect(&started.id, false)
        .expect("收集（桩模型）");
    let stub_score = collected.sources.first().expect("桩模型来源").credibility;

    assert_eq!(collected.sources.len(), rule.sources.len(), "换 provider 不影响收集结果");
    assert!(
        stub_score < rule_score,
        "0 分的模型应当压低可信度：{stub_score} vs {rule_score}"
    );
    assert!(stub_score > 0.0, "级别折算的一半是下限，不会被模型拉到 0");
}

#[test]
fn research_steps_before_write_touch_nothing_on_disk() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    seed(&fixture);
    let context = fixture.context();

    let before_files = context.storage.list_markdown_files().expect("文件清单");
    let before_notes = NoteService::new(fixture.context())
        .recent(500)
        .expect("笔记数")
        .len();

    // 走完前四步 + 生成写入提案（**都还没有落库**）
    let session = run_until(
        &fixture,
        request("Rust 异步运行时调度"),
        &["collect", "verify", "compose", "proposal"],
    );
    assert_eq!(session.status, ResearchStatus::Proposed);

    let after_files = context.storage.list_markdown_files().expect("文件清单");
    let after_notes = NoteService::new(fixture.context())
        .recent(500)
        .expect("笔记数")
        .len();
    assert_eq!(
        before_notes, after_notes,
        "AI 不直接写库：写入提案之前不得新增任何笔记"
    );
    assert_eq!(
        before_files, after_files,
        "写入提案之前不得改动工作区里的任何 Markdown 文件"
    );
    // 会话文件是旁路数据，不在 Markdown 清单里
    assert!(
        after_files
            .iter()
            .all(|path| !path.contains(".knowledge/research")),
        "研究会话不应混进可扫描的 Markdown：{after_files:?}"
    );
}

#[test]
fn every_research_step_is_gated_by_the_ai_switch() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    seed(&fixture);
    let research = fixture.research();
    let session = research.start(request("Rust 异步运行时调度")).expect("立项");
    research.collect(&session.id, false).expect("收集");

    // 关掉总开关：**每一步**都必须被闸门挡住（闸门在步骤校验之前）
    fixture
        .policy()
        .update(UpdateAiPolicyInput { enabled: Some(false), ..Default::default() })
        .expect("关闭总开关");

    let steps: Vec<(&str, String)> = vec![
        (
            "collect",
            research.collect(&session.id, false).expect_err("收集").code.as_str().to_string(),
        ),
        ("verify", research.verify(&session.id).expect_err("验证").code.as_str().to_string()),
        ("compose", research.compose(&session.id).expect_err("汇总").code.as_str().to_string()),
        ("proposal", research.proposal(&session.id).expect_err("提案").code.as_str().to_string()),
        (
            "write",
            research
                .write(ResearchWriteInput {
                    session_id: session.id.clone(),
                    mode: WRITE_MODE_CREATE.to_string(),
                    ..Default::default()
                })
                .expect_err("落库")
                .code
                .as_str()
                .to_string(),
        ),
        ("abort", research.abort(&session.id, None).expect_err("中止").code.as_str().to_string()),
    ];
    for (step, code) in &steps {
        assert_eq!(code, "AI_DISABLED", "{step} 必须被闸门挡住");
    }

    // 读取会话是本地操作、不经过闸门：关掉 AI 之后草稿仍然读得到（不丢数据）
    let draft = research.get(&session.id).expect("读取会话不应被闸门挡住");
    assert_eq!(draft.status, ResearchStatus::Collected);
    assert!(!draft.sources.is_empty());

    // 审计里能查到这些拒绝记录（能力名 + AI_DISABLED）
    let audit = fixture.policy().audit_tail(30).expect("审计");
    assert!(
        audit
            .iter()
            .filter(|entry| entry.outcome == "denied" && entry.note.as_deref() == Some("AI_DISABLED"))
            .count()
            >= 6,
        "每一步的拒绝都要留痕：{audit:?}"
    );
    assert!(audit.iter().all(|entry| !entry.outbound), "被拒绝时不得外发");
}

#[test]
fn written_report_is_a_normal_markdown_note_and_becomes_searchable() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    seed(&fixture);

    let session = run_until(
        &fixture,
        request("Rust 异步运行时调度"),
        &["collect", "verify", "compose", "proposal"],
    );
    let result = fixture
        .research()
        .write(ResearchWriteInput {
            session_id: session.id.clone(),
            mode: WRITE_MODE_CREATE.to_string(),
            ..Default::default()
        })
        .expect("写入");
    let note_id = result.note_id.clone().expect("笔记 id");

    // 1) 文件是唯一事实来源：磁盘上的 Markdown 自带 frontmatter 元数据 + 报告正文
    let detail = NoteService::new(fixture.context())
        .get(&note_id)
        .expect("读回笔记");
    let path = fixture
        .workspace
        .path()
        .join(detail.note.file_path.replace('\\', "/"));
    let raw = std::fs::read_to_string(&path).expect("笔记文件存在");
    let parsed = frontmatter::parse(&raw);
    assert!(parsed.parse_error.is_none(), "frontmatter 必须可解析");
    assert_eq!(parsed.frontmatter.id.as_deref(), Some(note_id.as_str()));
    assert_eq!(parsed.frontmatter.source_type.as_deref(), Some("research"));
    assert_eq!(
        parsed.frontmatter.source_reliability,
        Some(result.source_reliability),
        "可靠性要落到文件里（按最低来源级别取整）"
    );
    assert!(parsed.frontmatter.valid_until.is_some(), "复核时间要落到文件里");
    assert!(
        parsed.frontmatter.source_ref.as_deref().unwrap_or("").contains("[1]"),
        "来源清单要落到文件里：{:?}",
        parsed.frontmatter.source_ref
    );
    assert!(parsed.body.contains("资料截至"), "正文含报告抬头");
    assert!(
        parsed.frontmatter.tags.iter().any(|tag| tag == "调研报告"),
        "标签要落到文件里：{:?}",
        parsed.frontmatter.tags
    );

    // 2) 落库后立刻能被检索到（索引已更新，不是等下次扫描）
    let hits = SearchService::new(fixture.context())
        .search(SearchQuery {
            keyword: "调研报告".to_string(),
            page: Some(1),
            page_size: Some(20),
            ..Default::default()
        })
        .expect("检索");
    assert!(
        hits.hits.iter().any(|hit| hit.note.id == note_id),
        "写入后应当立刻可被检索到"
    );
}

#[test]
fn scanning_after_write_back_does_not_duplicate_the_report_note() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    seed(&fixture);
    let context = fixture.context();

    let session = run_until(
        &fixture,
        request("Rust 异步运行时调度"),
        &["collect", "verify", "compose", "proposal"],
    );
    let result = fixture
        .research()
        .write(ResearchWriteInput {
            session_id: session.id.clone(),
            mode: WRITE_MODE_CREATE.to_string(),
            create_source_note: true,
            ..Default::default()
        })
        .expect("写入");
    let note_id = result.note_id.clone().expect("笔记 id");

    let notes_before = NoteService::new(fixture.context())
        .recent(500)
        .expect("笔记数")
        .len();
    let files_before = context.storage.list_markdown_files().expect("文件清单");

    // 写回之后再扫描两轮：文件是我们自己写的，**不能被当成外部新增重复采纳**
    for round in 1..=2 {
        personal_knowledge_os_lib::application::index_service::IndexService::new(fixture.context())
            .scan_workspace()
            .unwrap_or_else(|error| panic!("第 {round} 轮扫描失败：{}", error.message));
    }

    let notes_after = NoteService::new(fixture.context())
        .recent(500)
        .expect("笔记数")
        .len();
    let files_after = context.storage.list_markdown_files().expect("文件清单");
    assert_eq!(
        notes_before, notes_after,
        "扫描不得把 AI 写入的笔记重复采纳成新笔记"
    );
    assert_eq!(files_before.len(), files_after.len(), "扫描不得新建文件");

    // 同标题只剩一份（真的没有重复落库）
    let listed = NoteService::new(fixture.context())
        .list(personal_knowledge_os_lib::domain::note::NoteListQuery {
            page_size: Some(500),
            ..Default::default()
        })
        .expect("列表");
    let same_title: Vec<&str> = listed
        .items
        .iter()
        .filter(|item| item.title == "调研报告：Rust 异步运行时调度")
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(same_title, vec![note_id.as_str()], "报告笔记应当只有一份");
    assert!(
        listed
            .items
            .iter()
            .any(|item| item.title == "来源清单：Rust 异步运行时调度"),
        "来源清单子文档也要在"
    );
}
