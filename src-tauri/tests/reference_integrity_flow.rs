//! 引用完整性（0 信任：不允许出现「指向不存在的分类 / 笔记」的悬挂引用）。
//!
//! 覆盖会写入分类引用的四条路径：新建笔记、单篇移动、批量移动、导入，
//! 以及 AI 写回（`research_write` 指定分类时）。
//! 要求：给不存在的分类 → 明确报错，且**不产生任何副作用**（不建文件、不改路径、不改会话状态）。

use std::sync::Arc;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::research::ResearchService;
use personal_knowledge_os_lib::application::batch_service::{BatchMoveInput, BatchService};
use personal_knowledge_os_lib::application::import_service::{ImportNotesInput, ImportService};
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, NoteListQuery};
use personal_knowledge_os_lib::domain::research::{ResearchRequest, ResearchStatus, ResearchWriteInput};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

const MISSING: &str = "cat_不存在的分类";

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
            .create("引用完整性测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.context())
    }

    fn absolute(&self, relative: &str) -> std::path::PathBuf {
        self.workspace.path().join(relative.replace('\\', "/"))
    }
}

#[test]
fn creating_a_note_with_a_missing_category_writes_nothing() {
    let fixture = Fixture::new();
    let before = fixture.notes().list(NoteListQuery::default()).expect("笔记列表").total;

    let error = fixture
        .notes()
        .create(CreateNoteInput {
            title: "不该被创建".to_string(),
            category_id: Some(MISSING.to_string()),
            initial_content: Some("正文".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect_err("指定不存在的分类必须报错");
    assert_eq!(error.code.as_str(), "CATEGORY_NOT_FOUND");

    let after = fixture.notes().list(NoteListQuery::default()).expect("笔记列表").total;
    assert_eq!(before, after, "失败的新建不能留下半成品笔记");
    assert!(
        !fixture.absolute("00_Inbox/不该被创建.md").exists(),
        "失败的新建不能留下文件"
    );
}

#[test]
fn moving_to_a_missing_category_keeps_the_note_where_it_was() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "移动目标".to_string(),
            category_id: None,
            initial_content: Some("正文".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("新建笔记")
        .note;

    let error = fixture
        .notes()
        .move_to_category(&note.id, Some(MISSING))
        .expect_err("移动到不存在的分类必须报错");
    assert_eq!(error.code.as_str(), "CATEGORY_NOT_FOUND");

    let reloaded = fixture.notes().get(&note.id).expect("读回笔记");
    assert_eq!(reloaded.note.file_path, note.file_path, "失败时文件路径不能变");
    assert_eq!(reloaded.note.category_id, None, "失败时分类归属不能变");
    assert!(fixture.absolute(&note.file_path).exists(), "原文件要还在");
}

#[test]
fn batch_move_reports_the_failure_per_note_and_keeps_the_rest_intact() {
    let fixture = Fixture::new();
    let first = fixture
        .notes()
        .create(CreateNoteInput {
            title: "批量甲".to_string(),
            category_id: None,
            initial_content: Some("甲".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("甲")
        .note;
    let second = fixture
        .notes()
        .create(CreateNoteInput {
            title: "批量乙".to_string(),
            category_id: None,
            initial_content: Some("乙".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("乙")
        .note;

    // 整批都指向不存在的分类：全部失败，且都不动
    let report = BatchService::new(fixture.context())
        .move_notes(BatchMoveInput {
            ids: vec![first.id.clone(), second.id.clone()],
            category_id: Some(MISSING.to_string()),
        })
        .expect("批量移动本身不该报错");
    assert_eq!(report.succeeded, 0);
    assert_eq!(report.failures.len(), 2, "每一篇都要有失败记录：{report:?}");
    for failure in &report.failures {
        assert!(failure.message.contains("分类不存在"), "{}", failure.message);
    }
    assert_eq!(
        fixture.notes().get(&first.id).expect("甲").note.file_path,
        first.file_path
    );
    assert!(fixture.absolute(&first.file_path).exists());
}

#[test]
fn import_rejects_a_missing_category() {
    let fixture = Fixture::new();
    let source = tempfile::tempdir().expect("导入源");
    let file = source.path().join("待导入.md");
    std::fs::write(&file, "# 待导入\n\n正文\n").expect("写源文件");

    let error = ImportService::new(fixture.context())
        .import(ImportNotesInput {
            paths: vec![file.to_string_lossy().to_string()],
            category_id: Some(MISSING.to_string()),
            preserve_structure: None,
            mode: None,
        })
        .expect_err("导入到不存在的分类必须报错");
    assert_eq!(error.code.as_str(), "CATEGORY_NOT_FOUND");

    let listed = fixture.notes().list(NoteListQuery::default()).expect("笔记列表");
    assert!(
        listed.items.iter().all(|item| item.title != "待导入"),
        "导入失败不得留下笔记：{:?}",
        listed.items.iter().map(|item| item.title.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn research_write_with_a_missing_category_fails_and_keeps_the_session_retryable() {
    let fixture = Fixture::new();
    AiPolicyService::new(fixture.context())
        .update(UpdateAiPolicyInput { enabled: Some(true), ..Default::default() })
        .expect("打开 AI");
    fixture
        .notes()
        .create(CreateNoteInput {
            title: "调研素材".to_string(),
            category_id: None,
            initial_content: Some("素材正文".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("素材");

    let research = ResearchService::new(fixture.context());
    let session = research
        .start(ResearchRequest { topic: "调研素材".to_string(), ..Default::default() })
        .expect("立项");
    let session = research.collect(&session.id, false).expect("收集");
    let session = research.verify(&session.id, false).expect("验证");
    let session = research.compose(&session.id, false).expect("汇总");
    let session = research.proposal(&session.id).expect("提案");
    let before = fixture.notes().list(NoteListQuery::default()).expect("笔记列表").total;

    let error = research
        .write(ResearchWriteInput {
            session_id: session.id.clone(),
            mode: "create".to_string(),
            category_id: Some(MISSING.to_string()),
            ..Default::default()
        })
        .expect_err("写入不存在的分类必须报错");
    assert_eq!(error.code.as_str(), "CATEGORY_NOT_FOUND");

    let after = fixture.notes().list(NoteListQuery::default()).expect("笔记列表").total;
    assert_eq!(before, after, "失败时不能落库");
    let reloaded = research.get(&session.id).expect("读回会话");
    assert_eq!(reloaded.status, ResearchStatus::Proposed, "失败后会话应仍可重试");
    assert!(reloaded.written_note_id.is_none(), "失败不应记录已写入的笔记 id");

    // 改用 Inbox 再写一次：应当成功（证明会话状态没被弄坏）
    let result = research
        .write(ResearchWriteInput {
            session_id: session.id.clone(),
            mode: "create".to_string(),
            ..Default::default()
        })
        .expect("改到 Inbox 后应当成功");
    assert_eq!(result.outcome, "created");
}
