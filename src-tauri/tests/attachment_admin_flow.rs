//! 附件管理（V1.1.2）：清点、引用识别与删除的集成测试。

use std::sync::Arc;

use personal_knowledge_os_lib::application::attachment_service::{
    AttachmentService, SaveAttachmentInput,
};
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::base64;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SaveNoteInput};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D];

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
            .create("附件管理测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn attachments(&self) -> AttachmentService {
        AttachmentService::new(self.app.context().expect("工作区已打开"))
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }
}

#[test]
fn lists_attachments_with_reference_notes_and_deletes_them() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "带图的笔记".into(),
            category_id: None,
            initial_content: None,
            tags: None,
        })
        .expect("创建笔记");

    let used = fixture
        .attachments()
        .save(SaveAttachmentInput {
            note_id: Some(note.note.id.clone()),
            file_name: Some("被引用的图.png".into()),
            mime_type: Some("image/png".into()),
            data_base64: base64::encode(PNG),
        })
        .expect("保存附件 1");
    let orphan = fixture
        .attachments()
        .save(SaveAttachmentInput {
            note_id: Some(note.note.id.clone()),
            file_name: Some("没人用的图.png".into()),
            mime_type: Some("image/png".into()),
            data_base64: base64::encode(PNG),
        })
        .expect("保存附件 2");

    // 正文引用其中的一张（写成 Markdown 图片链接）
    fixture
        .notes()
        .save(SaveNoteInput {
            id: note.note.id.clone(),
            content: format!("![图]({})", used.link_path),
            base_content_hash: None,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("保存正文");

    let report = fixture.attachments().list().expect("列出附件");
    assert_eq!(report.items.len(), 2);
    assert_eq!(report.referenced_count, 1, "只有一张图被正文引用");
    let referenced = report
        .items
        .iter()
        .find(|item| item.relative_path == used.relative_path)
        .expect("找到被引用附件");
    assert_eq!(referenced.references.len(), 1);
    assert_eq!(referenced.references[0].title, "带图的笔记");
    assert!(referenced.bytes > 0);
    assert!(referenced.month.starts_with("20"), "目录按月命名：{}", referenced.month);
    assert_eq!(report.total_bytes, report.items.iter().map(|item| item.bytes).sum::<u64>());

    let orphan_entry = report
        .items
        .iter()
        .find(|item| item.relative_path == orphan.relative_path)
        .expect("找到孤立附件");
    assert!(orphan_entry.references.is_empty());

    // 删除：只允许附件目录；删除后清单减少
    assert!(fixture.attachments().remove("00_Inbox/恶意路径.png").is_err());
    let freed = fixture
        .attachments()
        .remove(&orphan.relative_path)
        .expect("删除附件");
    assert_eq!(freed, PNG.len() as u64);
    let after = fixture.attachments().list().expect("再次列出");
    assert_eq!(after.items.len(), 1);
}

#[test]
fn empty_workspace_returns_empty_report() {
    let fixture = Fixture::new();
    let report = fixture.attachments().list().expect("列出附件");
    assert!(report.items.is_empty());
    assert_eq!(report.total_bytes, 0);
    assert_eq!(report.referenced_count, 0);
}
