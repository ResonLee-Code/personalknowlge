use std::sync::Arc;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::writing::WritingService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::error::ErrorCode;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SaveNoteInput};
use personal_knowledge_os_lib::domain::writing::{
    WritingApplyInput, WritingMode, WritingRequest, WritingSegment,
};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    _data: TempDir,
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
            .create("写作测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        AiPolicyService::new(app.context().expect("上下文"))
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                ..Default::default()
            })
            .expect("开启 AI 以使用模板 / 本地降级");
        Self {
            _data: data,
            _workspace: workspace,
            app,
        }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("上下文"))
    }

    fn writing(&self) -> WritingService {
        WritingService::new(self.app.context().expect("上下文"))
    }

    fn note(&self, title: &str, body: &str) -> String {
        self.notes()
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(body.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记")
            .note
            .id
    }
}

#[test]
fn five_writing_modes_are_cited_and_apply_requires_confirmation() {
    let fixture = Fixture::new();
    let target = fixture.note(
        "写作原则",
        "# 写作原则\n\n本地优先是核心边界。引用必须能回到原文。所有生成内容先预览。",
    );
    let _source = fixture.note(
        "引用规范",
        "# 引用规范\n\n每一个生成段落都要附来源。引用必须能回到原文区间。",
    );
    let current = fixture.notes().get(&target).expect("读取目标").content;

    for mode in [
        WritingMode::Outline,
        WritingMode::Continue,
        WritingMode::Rewrite,
        WritingMode::Shorten,
        WritingMode::Expand,
    ] {
        let draft = fixture
            .writing()
            .draft(WritingRequest {
                note_id: target.clone(),
                mode: Some(mode),
                instruction: "保持事实边界，引用规范".to_string(),
                selection: Some("本地优先是核心边界。".to_string()),
                selection_start: Some(8),
                selection_end: Some(20),
                confirm_outbound: false,
            })
            .expect("生成草稿");
        assert!(!draft.segments.is_empty(), "{mode:?}");
        assert!(
            draft
                .segments
                .iter()
                .all(|segment| !segment.citations.is_empty()),
            "{mode:?} 的每个段落都必须有引用"
        );
        assert_eq!(
            fixture.notes().get(&target).expect("草稿只读").content,
            current,
            "生成阶段不得写文件"
        );
    }

    let draft = fixture
        .writing()
        .draft(WritingRequest {
            note_id: target.clone(),
            mode: Some(WritingMode::Outline),
            instruction: "整理为大纲".to_string(),
            confirm_outbound: false,
            ..Default::default()
        })
        .expect("生成待应用草稿");
    let base = draft.base_content_hash.clone();
    let apply_input = WritingApplyInput {
        note_id: target.clone(),
        base_content_hash: base.clone(),
        mode: WritingMode::Outline,
        target_start: draft.target_start,
        target_end: draft.target_end,
        accepted_segments: vec![draft.segments[0].clone()],
        confirm: false,
    };
    let preview = fixture
        .writing()
        .apply_preview(&apply_input)
        .expect("写前预览");
    assert!(preview.content.contains("写作来源"));
    assert_eq!(
        fixture.notes().get(&target).expect("预览只读").content,
        current,
        "apply_preview 不能写文件"
    );
    let rejected = fixture
        .writing()
        .apply(apply_input.clone())
        .expect_err("未确认必须拒绝");
    assert_eq!(rejected.code, ErrorCode::InvalidInput);

    let applied = fixture
        .writing()
        .apply(WritingApplyInput {
            confirm: true,
            ..apply_input
        })
        .expect("确认后写入");
    assert!(applied.detail.content.contains("写作来源"));
    assert!(applied.accepted_segments >= 1);

    let stale = WritingApplyInput {
        note_id: target,
        base_content_hash: base,
        mode: WritingMode::Rewrite,
        target_start: 0,
        target_end: 0,
        accepted_segments: vec![WritingSegment {
            id: "W1".to_string(),
            text: "过期草稿 [1]".to_string(),
            citations: draft.citations.clone(),
        }],
        confirm: true,
    };
    let conflict = fixture
        .writing()
        .apply_preview(&stale)
        .expect_err("目标笔记哈希变化后必须拒绝");
    assert_eq!(conflict.code, ErrorCode::NoteConflict);
}

#[test]
fn source_changed_after_generation_blocks_apply() {
    let fixture = Fixture::new();
    let target = fixture.note("扩写目标", "# 扩写目标\n\n需要解释引用的可追溯性。");
    let source = fixture.note(
        "来源事实",
        "# 来源事实\n\n可追溯引用必须保留原文区间与来源笔记标题。",
    );
    let draft = fixture
        .writing()
        .draft(WritingRequest {
            note_id: target.clone(),
            mode: Some(WritingMode::Expand),
            instruction: "可追溯引用 来源事实".to_string(),
            selection: Some("需要解释引用的可追溯性。".to_string()),
            selection_start: Some(8),
            selection_end: Some(22),
            confirm_outbound: false,
        })
        .expect("生成草稿");
    let source_citation = draft
        .citations
        .iter()
        .find(|citation| citation.note_id == source)
        .cloned()
        .expect("相关来源应成为可引用材料");
    fixture
        .notes()
        .save(SaveNoteInput {
            id: source,
            content: "# 来源事实\n\n来源已被改写，旧引用区间不再有效。".to_string(),
            base_content_hash: None,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("修改来源");
    let error = fixture
        .writing()
        .apply_preview(&WritingApplyInput {
            note_id: target,
            base_content_hash: draft.base_content_hash,
            mode: WritingMode::Expand,
            target_start: draft.target_start,
            target_end: draft.target_end,
            accepted_segments: vec![WritingSegment {
                id: "W1".to_string(),
                text: format!("扩写内容 [{}]", source_citation.citation_id),
                citations: vec![source_citation],
            }],
            confirm: true,
        })
        .expect_err("来源漂移必须阻止写入");
    assert_eq!(error.code, ErrorCode::NoteConflict);
}
