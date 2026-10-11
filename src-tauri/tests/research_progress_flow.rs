//! 研究进度事件（AI_ARCHITECTURE_PLAN §9.2：每一步都要让界面看到「进行到哪了」）。
//!
//! 验收要点：
//! 1. 每一步都发事件，且带的是**元数据**（会话 id / 步骤 / 说明 / 已抓页数 / 来源数）；
//! 2. 事件里**不得**出现笔记正文（与审计同一条隐私口径）；
//! 3. 没有联网时 `pagesFetched` 保持 0，不会假装抓过网页。

use std::sync::{Arc, Mutex};

use serde_json::Value;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::research::ResearchService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::{events as event_names, EventSink};
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::domain::research::ResearchRequest;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

/// 把事件记下来，供断言逐条检查。
#[derive(Default)]
struct RecordingSink {
    events: Mutex<Vec<(String, Value)>>,
}

impl EventSink for RecordingSink {
    fn emit(&self, event: &str, payload: Value) {
        self.events
            .lock()
            .expect("事件锁")
            .push((event.to_string(), payload));
    }
}

impl RecordingSink {
    fn progress(&self) -> Vec<Value> {
        self.events
            .lock()
            .expect("事件锁")
            .iter()
            .filter(|(event, _)| event == event_names::RESEARCH_PROGRESS)
            .map(|(_, payload)| payload.clone())
            .collect()
    }
}

const NOTE_BODY: &str = "调度器实现要点：任务唤醒与本地队列的批量窃取，风险是阻塞调用。";

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
    sink: Arc<RecordingSink>,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("数据目录");
        let workspace = tempfile::tempdir().expect("工作区");
        let sink = Arc::new(RecordingSink::default());
        let app = AppInner::new(data.path().to_path_buf(), Arc::clone(&sink) as Arc<dyn EventSink>)
            .expect("应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("进度事件测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app, sink }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }
}

#[test]
fn every_research_step_emits_metadata_only_progress_events() {
    let fixture = Fixture::new();
    AiPolicyService::new(fixture.context())
        .update(UpdateAiPolicyInput { enabled: Some(true), ..Default::default() })
        .expect("打开 AI 总开关");
    NoteService::new(fixture.context())
        .create(CreateNoteInput {
            title: "Rust 异步运行时调度方案".to_string(),
            category_id: None,
            initial_content: Some(NOTE_BODY.to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("建笔记");

    let research = ResearchService::new(fixture.context());
    let session = research
        .start(ResearchRequest {
            topic: "Rust 异步运行时调度".to_string(),
            ..Default::default()
        })
        .expect("立项");
    research.collect(&session.id, false).expect("收集");
    research.verify(&session.id).expect("验证");
    research.compose(&session.id).expect("汇总");
    research.proposal(&session.id).expect("提案");
    research
        .abort(&session.id, Some("测试中止".to_string()))
        .expect("中止");

    let progress = fixture.sink.progress();
    assert!(!progress.is_empty(), "研究过程必须发进度事件");

    let steps: Vec<String> = progress
        .iter()
        .filter_map(|payload| payload.get("step").and_then(|value| value.as_str()))
        .map(str::to_string)
        .collect();
    for expected in ["plan", "collect", "verify", "compose", "proposal", "abort"] {
        assert!(
            steps.iter().any(|step| step == expected),
            "缺少「{expected}」步骤的进度事件：{steps:?}"
        );
    }

    for payload in &progress {
        let object = payload.as_object().expect("进度事件必须是对象");
        for key in ["sessionId", "step", "detail", "pagesFetched", "sources"] {
            assert!(object.contains_key(key), "进度事件缺少字段 {key}：{payload}");
        }
        assert_eq!(
            object.get("sessionId").and_then(Value::as_str),
            Some(session.id.as_str()),
            "事件必须指明是哪次会话"
        );
        let serialized = payload.to_string();
        assert!(
            !serialized.contains("任务唤醒") && !serialized.contains("批量窃取"),
            "进度事件里出现了笔记正文：{serialized}"
        );
        assert_eq!(
            object.get("pagesFetched").and_then(Value::as_u64),
            Some(0),
            "仅本地口径下 pagesFetched 必须是 0：{payload}"
        );
    }

    let last_collect = progress
        .iter()
        .filter(|payload| payload.get("step").and_then(Value::as_str) == Some("collect"))
        .next_back()
        .expect("收集步骤的进度事件");
    assert!(
        last_collect.get("sources").and_then(Value::as_u64).unwrap_or(0) >= 1,
        "收集完成时应报出采纳的来源条数：{last_collect}"
    );
}
