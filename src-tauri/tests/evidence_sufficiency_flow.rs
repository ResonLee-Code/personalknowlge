//! 证据充分性 + 查询扩展（Track D5 / N9）集成测试。
//!
//! 验收标准（PLAN_V1.3.0 §2 Track D 的 D5 行）：**证据不足时如实说「不够」，而不是硬答**。
//! 这里把它拆成三条可执行的口径：
//!
//! 1. 材料一条都没有 → 报告必须**明说证据不足**（顶部结论 + 警告块 + 结论段为空）；
//! 2. 材料够（每个子问题都有、且都有 ≥2 个独立来源）→ 报告仍然正常给结论
//!    （证明上一档不是「永远说不足」）；
//! 3. 查询扩展真的扩大了召回：只命中扩展查询（课题原文查不到）的笔记也要被收进来。

use std::sync::Arc;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::research::ResearchService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SetSourceInput};
use personal_knowledge_os_lib::domain::research::{
    EvidenceGapKind, ResearchRequest, ResearchSession, WarningKind,
};
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    _data: TempDir,
    /// 只是要它在整个用例期间活着（TempDir 一 drop 目录就没了）
    _workspace: TempDir,
    app: Arc<AppInner>,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("证据充分性测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn research(&self) -> ResearchService {
        ResearchService::new(self.context())
    }

    fn enable_ai(&self) {
        AiPolicyService::new(self.context())
            .update(UpdateAiPolicyInput { enabled: Some(true), ..Default::default() })
            .expect("打开 AI 总开关");
    }

    /// 建一篇带来源标注的笔记（reliability 4 ≈ B 级，算可用来源）。
    fn note(&self, title: &str, content: &str) -> String {
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
        notes
            .set_source(SetSourceInput {
                id: detail.note.id.clone(),
                source_type: Some("official".to_string()),
                source_ref: None,
                source_reliability: Some(4),
                valid_until: None,
            })
            .expect("标注来源");
        detail.note.id
    }
}

fn run_until(fixture: &Fixture, topic: &str, steps: &[&str]) -> ResearchSession {
    let research = fixture.research();
    let mut session = research
        .start(ResearchRequest { topic: topic.to_string(), ..Default::default() })
        .expect("立项");
    for step in steps {
        session = match *step {
            "collect" => research.collect(&session.id, false).expect("收集"),
            "verify" => research.verify(&session.id, false).expect("验证"),
            "compose" => research.compose(&session.id, false).expect("汇总"),
            other => panic!("未知步骤 {other}"),
        };
    }
    session
}

/// ① 一条材料都没有：必须如实说「不够」，且不产出结论。
#[test]
fn no_material_is_reported_as_insufficient_evidence_not_as_a_conclusion() {
    let fixture = Fixture::new();
    fixture.enable_ai();

    let session = run_until(
        &fixture,
        "玄武岩调度策略的取舍",
        &["collect", "verify", "compose"],
    );
    let report = session.report.as_ref().expect("报告");

    assert!(
        !report.evidence.sufficient,
        "一条材料都没有时必须判「不足」：{:?}",
        report.evidence
    );
    assert_eq!(
        report.evidence.gaps.first().map(|gap| gap.kind),
        Some(EvidenceGapKind::NoMaterial),
        "{:?}",
        report.evidence.gaps
    );
    assert_eq!(report.evidence.covered_sub_questions, 0, "没有任何子问题被覆盖");
    assert_eq!(report.evidence.total_sub_questions, 5, "默认 5 个子问题");

    // 报告必须有**醒目**的不给结论声明（顶部 + 警告块 + 第 1 段）
    assert!(
        report.markdown.contains("证据充分性：不足"),
        "报告顶部要写清楚：{}",
        report.markdown
    );
    assert!(
        report.markdown.contains("不给结论"),
        "结论段要说明为什么是空的：{}",
        report.markdown
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning.kind == WarningKind::InsufficientEvidence),
        "警告块里要有「证据不足」：{:?}",
        report
            .warnings
            .iter()
            .map(|warning| warning.kind)
            .collect::<Vec<_>>()
    );
    assert!(
        report.conclusions.is_empty() && report.key_facts.is_empty(),
        "没有材料就没有结论段与关键事实段：{:?} / {:?}",
        report.conclusions,
        report.key_facts
    );
    assert!(
        !report.evidence.recommendation.trim().is_empty(),
        "不足时要给出怎么补（例如打开联网档位）"
    );
}

/// ② 材料够：每个子问题都有材料、且都有 ≥2 个独立来源 → 仍然正常给结论。
#[test]
fn enough_independent_material_still_produces_conclusions() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let topic = "玄武岩调度策略";
    // 5 个子问题各两篇（标题带各自的意图词，让规则归属能对上）
    let titles = [
        "玄武岩调度策略的定义与概念",
        "玄武岩调度策略的主流方案与做法",
        "玄武岩调度策略的适用场景与限制",
        "玄武岩调度策略的风险与失败案例",
        "玄武岩调度策略的最近进展与趋势",
    ];
    for (index, title) in titles.iter().enumerate() {
        for copy in 0..2 {
            // 正文刻意**不含**其它子问题的意图词（注意 / 边界 / 风险 …），
            // 否则规则归属会把它算到别人头上，这份"材料充足"的布置就名不副实了。
            fixture.note(
                &format!("{title}（{copy}）"),
                &format!("玄武岩调度策略的材料 {index}-{copy}：正文只记录要点，供交叉验证用。"),
            );
        }
    }

    let session = run_until(&fixture, topic, &["collect", "verify", "compose"]);
    let report = session.report.as_ref().expect("报告");
    assert!(
        report.evidence.sufficient,
        "材料充足却判成不足，说明门槛太严：{:?}",
        report.evidence.gaps
    );
    assert_eq!(report.evidence.covered_sub_questions, 5);
    assert!(report.evidence.independent_sources >= 2);
    assert!(report.markdown.contains("证据充分性：充分"), "{}", report.markdown);
    assert!(
        !report.conclusions.is_empty(),
        "证据充分时必须能给出结论（否则这个能力只会让它更保守，不是更诚实）"
    );
    assert!(
        !report
            .warnings
            .iter()
            .any(|warning| warning.kind == WarningKind::InsufficientEvidence),
        "充分时不该出现「证据不足」警告"
    );
}

/// ③ 查询扩展真的扩大召回：只命中扩展查询的笔记也会被收进来。
#[test]
fn query_expansion_reaches_notes_that_the_raw_topic_cannot_find() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let topic = "SQLite 全文检索与中文分词";

    // 这篇能被「课题原文」查到
    let topical = fixture.note(
        "SQLite 全文检索与中文分词的总览",
        "SQLite 全文检索与中文分词的组合是本地知识库检索的核心：索引与查询都要照顾中文。",
    );
    // 这篇只讲「中文分词」——课题原文里那串连续汉字它没有，只有扩展查询能捞到它
    let only_segment = fixture.note(
        "中文分词的双字方案",
        "中文分词需要双字扩展；长尾词要靠更好的分词策略，否则召回会有缺口。",
    );

    // 控制组：只用课题原文检索，确认它**查不到**上面那篇（否则这条用例证明不了扩展有用）
    let baseline = SearchService::new(fixture.context())
        .search(SearchQuery {
            keyword: topic.to_string(),
            page: Some(1),
            page_size: Some(20),
            ..Default::default()
        })
        .expect("课题原文检索");
    assert!(
        baseline.hits.iter().any(|hit| hit.note.id == topical),
        "课题原文至少要能查到那篇总览"
    );
    assert!(
        !baseline.hits.iter().any(|hit| hit.note.id == only_segment),
        "控制组失效：课题原文居然也能查到「中文分词」那篇，本用例就证明不了扩展的价值"
    );

    let research = fixture.research();
    let session = research
        .start(ResearchRequest { topic: topic.to_string(), ..Default::default() })
        .expect("立项");
    let plan = session.plan.as_ref().expect("计划");
    assert_eq!(
        plan.queries.first().map(String::as_str),
        Some(topic),
        "第一条永远是课题原文"
    );
    assert!(
        plan.queries.iter().any(|query| query == "中文分词"),
        "连接词「与」两侧应当各自成一条查询：{:?}",
        plan.queries
    );
    assert!(
        plan.queries.iter().any(|query| query == "SQLite"),
        "拉丁词应当单独成一条查询：{:?}",
        plan.queries
    );

    let session = research.collect(&session.id, false).expect("收集");
    let collected: Vec<&str> = session
        .sources
        .iter()
        .filter_map(|source| source.note_id.as_deref())
        .collect();
    assert!(
        collected.contains(&topical.as_str()),
        "课题原文命中的笔记必须在来源里：{collected:?}"
    );
    assert!(
        collected.contains(&only_segment.as_str()),
        "只命中扩展查询的笔记也要被收进来（这就是查询扩展的价值）：{collected:?}"
    );
}
