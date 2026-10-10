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
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::core::time::today_date;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SetSourceInput};
use personal_knowledge_os_lib::domain::research::{
    ResearchRequest, ResearchSession, ResearchStatus, SourceLevel, WarningKind,
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
