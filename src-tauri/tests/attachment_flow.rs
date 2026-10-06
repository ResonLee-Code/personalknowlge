//! 附件落盘的集成测试（V1.1 M2-1）。

use std::path::PathBuf;
use std::sync::Arc;

use personal_knowledge_os_lib::application::attachment_service::{
    AttachmentService, ReadAttachmentInput, SaveAttachmentInput, MAX_ATTACHMENT_BYTES,
};
use personal_knowledge_os_lib::application::category_service::CategoryService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::base64;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

/// 1×1 的透明 PNG。
const SAMPLE_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
    0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
    0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
    0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
    0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        let root = workspace.path().to_path_buf();
        WorkspaceService::new(Arc::clone(&app))
            .create("附件测试", &root.to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app, root }
    }

    fn attachments(&self) -> AttachmentService {
        AttachmentService::new(self.app.context().expect("工作区已打开"))
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }
}

fn save_input(note_id: Option<String>, file_name: &str) -> SaveAttachmentInput {
    SaveAttachmentInput {
        note_id,
        file_name: Some(file_name.to_string()),
        mime_type: Some("image/png".to_string()),
        data_base64: base64::encode(SAMPLE_PNG),
    }
}

#[test]
fn saves_pasted_image_into_month_directory_and_returns_link_path() {
    let fixture = Fixture::new();
    let category = CategoryService::new(fixture.app.context().unwrap())
        .create(CreateCategoryInput {
            name: "附件测试分类".into(),
            parent_id: None,
            create_directory: None,
        })
        .expect("创建分类");
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "索引设计".into(),
            category_id: Some(category.id.clone()),
            initial_content: None,
            tags: None,
        })
        .expect("创建笔记");

    let saved = fixture
        .attachments()
        .save(save_input(Some(note.note.id.clone()), "截图 1.png"))
        .expect("保存附件");

    // 文件名：时间戳 - 清洗后的原名 - 短 ID.扩展名
    assert!(saved.relative_path.starts_with("attachments/"));
    assert!(saved.relative_path.ends_with(".png"), "{}", saved.relative_path);
    assert!(saved.file_name.starts_with("20") && saved.file_name.contains("截图-1"));
    assert!(std::path::Path::new(&saved.absolute_path).is_file());
    assert_eq!(std::fs::read(&saved.absolute_path).expect("读取附件"), SAMPLE_PNG);
    assert_eq!(saved.bytes, SAMPLE_PNG.len() as u64);
    assert!(saved.is_image);

    // 链接路径相对笔记所在目录（笔记在分类目录下 → 需要 ../）
    assert!(saved.link_path.starts_with("../attachments/"), "{}", saved.link_path);
    let resolved = fixture.root.join(&category.dir_name).join(&saved.link_path);
    assert!(resolved.is_file(), "链接路径应能在笔记目录下解析：{:?}", resolved);

    // 附件不能被当作笔记扫描进 notes 表
    let listed = fixture
        .notes()
        .list(Default::default())
        .expect("列出笔记");
    assert!(listed.items.iter().all(|item| !item.file_path.starts_with("attachments/")));
}

#[test]
fn accepts_data_url_payload_and_infers_extension_from_mime() {
    let fixture = Fixture::new();
    let payload = format!("data:image/png;base64,{}", base64::encode(SAMPLE_PNG));
    let saved = fixture
        .attachments()
        .save(SaveAttachmentInput {
            note_id: None,
            file_name: None,
            mime_type: None,
            data_base64: payload,
        })
        .expect("保存附件");

    assert!(saved.relative_path.ends_with(".png"));
    assert!(saved.is_image);
    // 未指定笔记时链接路径就是工作区相对路径
    assert_eq!(saved.link_path, saved.relative_path);
}

#[test]
fn rejects_empty_and_oversized_payloads() {
    let fixture = Fixture::new();
    let empty = fixture.attachments().save(SaveAttachmentInput {
        note_id: None,
        file_name: Some("a.png".into()),
        mime_type: Some("image/png".into()),
        data_base64: String::new(),
    });
    assert!(empty.is_err());

    let oversized = vec![0u8; MAX_ATTACHMENT_BYTES + 1];
    let too_big = fixture.attachments().save(SaveAttachmentInput {
        note_id: None,
        file_name: Some("big.png".into()),
        mime_type: Some("image/png".into()),
        data_base64: base64::encode(&oversized),
    });
    let error = too_big.expect_err("应拒绝超大附件");
    assert!(error.message.contains("上限"), "{}", error.message);
}

#[test]
fn rejects_invalid_base64() {
    let fixture = Fixture::new();
    let error = fixture
        .attachments()
        .save(SaveAttachmentInput {
            note_id: None,
            file_name: Some("a.png".into()),
            mime_type: None,
            data_base64: "这不是 base64 *".into(),
        })
        .expect_err("应拒绝非法 Base64");
    assert_eq!(error.code.as_str(), "INVALID_INPUT");
}

#[test]
fn reads_workspace_relative_image_as_data_url() {
    let fixture = Fixture::new();
    let category = CategoryService::new(fixture.app.context().unwrap())
        .create(CreateCategoryInput {
            name: "附件读取分类".into(),
            parent_id: None,
            create_directory: None,
        })
        .expect("创建分类");
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "带图笔记".into(),
            category_id: Some(category.id.clone()),
            initial_content: None,
            tags: None,
        })
        .expect("创建笔记");
    let saved = fixture
        .attachments()
        .save(save_input(Some(note.note.id.clone()), "示意图.png"))
        .expect("保存附件");

    let data = fixture
        .attachments()
        .read(ReadAttachmentInput {
            note_id: Some(note.note.id.clone()),
            source: saved.link_path.clone(),
        })
        .expect("读取附件");
    assert!(data.data_url.starts_with("data:image/png;base64,"));
    assert_eq!(data.mime_type, "image/png");
    assert_eq!(data.bytes, SAMPLE_PNG.len() as u64);

    // 外部链接与穿越路径都不允许通过读取命令访问
    assert!(
        fixture
            .attachments()
            .read(ReadAttachmentInput {
                note_id: Some(note.note.id.clone()),
                source: "https://example.com/a.png".into(),
            })
            .is_err()
    );
    assert!(
        fixture
            .attachments()
            .read(ReadAttachmentInput {
                note_id: Some(note.note.id.clone()),
                source: "../../../../etc/passwd".into(),
            })
            .is_err()
    );
}
