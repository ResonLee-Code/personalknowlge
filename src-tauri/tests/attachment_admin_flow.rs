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
            parent_id: None,
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

/// **F3 的「引用」口径守卫**：四种写法都要被认成引用，否则附件会被误报成孤儿、
/// 下一次清理就会删掉**还在用**的图。
///
/// | 写法 | 出处 | 期望 |
/// | --- | --- | --- |
/// | `![图](../attachments/2026-10/a.png)` | 本应用自己写入（相对路径，带 `../`） | 被引用 |
/// | `![[attachments/2026-10/a.png]]` | 用户手写的 wikilink（带路径） | 被引用 |
/// | `![[a.png]]` | Obsidian 习惯：**只写文件名** | 被引用（**保守方向**） |
/// | `[看图](a.png)` | 普通 markdown 链接（只写文件名） | 被引用 |
///
/// 反向：正文里**完全没提**的附件仍然是孤儿（否则"清理"就永远清不掉东西）。
#[test]
fn every_documented_reference_style_keeps_an_attachment_out_of_the_orphan_list() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "四种写法".into(),
            category_id: None,
            initial_content: None,
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");

    let mut saved = Vec::new();
    for (index, name) in ["相对路径.png", "带路径wiki.png", "只写文件名.png", "普通链接.png"]
        .iter()
        .enumerate()
    {
        saved.push(
            fixture
                .attachments()
                .save(SaveAttachmentInput {
                    note_id: Some(note.note.id.clone()),
                    file_name: Some((*name).into()),
                    mime_type: Some("image/png".into()),
                    data_base64: base64::encode(PNG),
                })
                .unwrap_or_else(|error| panic!("保存附件 {index} 失败：{}", error.message)),
        );
    }
    let orphan = fixture
        .attachments()
        .save(SaveAttachmentInput {
            note_id: Some(note.note.id.clone()),
            file_name: Some("真的没人用.png".into()),
            mime_type: Some("image/png".into()),
            data_base64: base64::encode(PNG),
        })
        .expect("保存孤儿附件");

    let body = [
        format!("![相对](../{})", saved[0].relative_path),
        format!("![[{}]]", saved[1].relative_path),
        format!("![[{}]]", saved[2].file_name),
        format!("[看图]({})", saved[3].file_name),
    ]
    .join("\n\n");
    fixture
        .notes()
        .save(SaveNoteInput {
            id: note.note.id.clone(),
            content: body,
            base_content_hash: None,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("保存正文");

    let report = fixture.attachments().list().expect("列出附件");
    assert_eq!(report.items.len(), 5);
    for entry in &report.items {
        if entry.relative_path == orphan.relative_path {
            assert!(
                entry.references.is_empty(),
                "没人提过的附件必须仍是孤儿，否则清理功能形同虚设"
            );
            continue;
        }
        assert_eq!(
            entry.references.len(),
            1,
            "「{}」应当被认成被引用（文件名：{}）",
            entry.relative_path,
            entry.file_name
        );
    }
    assert_eq!(report.referenced_count, 4, "四种写法各认一个");
}

/// 「只写文件名」按**完整文件名精确匹配**；写用户原始文件名属于**悬空引用**，不算引用。
///
/// 为什么后者不算：附件落盘时会被重命名成 `<时间戳>-<名字>-<短id>.<扩展名>`
/// （见 `attachment_service::save`）。用户若按**自己原来的**文件名写 `![[图.png]]`，
/// 那条链接在预览里本来就打不开——它指向的文件压根不存在。把这张图继续算成"被引用"，
/// 只会让真正没人用的文件永远清不掉。
#[test]
fn bare_name_reference_matches_the_stored_file_name_not_the_original_one() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "同名附件".into(),
            category_id: None,
            initial_content: None,
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");

    let saved = fixture
        .attachments()
        .save(SaveAttachmentInput {
            note_id: Some(note.note.id.clone()),
            file_name: Some("截图.png".into()),
            mime_type: Some("image/png".into()),
            data_base64: base64::encode(PNG),
        })
        .expect("保存附件");
    assert_ne!(
        saved.file_name, "截图.png",
        "落盘时会被重命名（时间戳 + 短 id），这正是「原始文件名」匹配不上的原因"
    );
    // 第二张：正文里写的是**用户原始文件名**，属于悬空引用
    let dangling = fixture
        .attachments()
        .save(SaveAttachmentInput {
            note_id: Some(note.note.id.clone()),
            file_name: Some("另一张.png".into()),
            mime_type: Some("image/png".into()),
            data_base64: base64::encode(PNG),
        })
        .expect("保存另一张附件");

    fixture
        .notes()
        .save(SaveNoteInput {
            id: note.note.id.clone(),
            content: format!("![[{}]]\n\n![[另一张.png]]", saved.file_name),
            base_content_hash: None,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("保存正文");

    let report = fixture.attachments().list().expect("列出附件");
    assert_eq!(
        report.referenced_count, 1,
        "只有「完整文件名」那条算引用；写原始文件名的那条是悬空引用"
    );
    let dangling_entry = report
        .items
        .iter()
        .find(|item| item.relative_path == dangling.relative_path)
        .expect("找到悬空引用的附件");
    assert!(
        dangling_entry.references.is_empty(),
        "悬空引用（指向不存在的文件名）不该把它从孤儿列表里摘出去"
    );
}

#[test]
fn empty_workspace_returns_empty_report() {
    let fixture = Fixture::new();
    let report = fixture.attachments().list().expect("列出附件");
    assert!(report.items.is_empty());
    assert_eq!(report.total_bytes, 0);
    assert_eq!(report.referenced_count, 0);
}
