//! 来源与时效（N1）的集成测试：frontmatter 往返 + 扫描同步。

use std::sync::Arc;

use personal_knowledge_os_lib::application::index_service::IndexService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SetSourceInput};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
    root: std::path::PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        let root = workspace.path().to_path_buf();
        WorkspaceService::new(Arc::clone(&app))
            .create("资产测试", &root.to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app, root }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }
}

#[test]
fn source_metadata_round_trips_through_frontmatter_and_database() {
    let fixture = Fixture::new();
    let notes = fixture.notes();
    let detail = notes
        .create(CreateNoteInput {
            title: "Tauri 选型依据".into(),
            category_id: None,
            initial_content: None,
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");

    let updated = notes
        .set_source(SetSourceInput {
            id: detail.note.id.clone(),
            source_type: Some("official".into()),
            source_ref: Some("https://tauri.app/".into()),
            source_reliability: Some(5),
            valid_until: Some("2027-01-01".into()),
        })
        .expect("更新来源");

    assert_eq!(updated.note.source_type.as_deref(), Some("official"));
    assert_eq!(updated.note.source_reliability, Some(5));
    assert_eq!(updated.note.valid_until.as_deref(), Some("2027-01-01"));

    // 写进了 Markdown frontmatter（外部编辑器可见），且原有字段未被破坏
    let stored = fixture
        .app
        .context()
        .unwrap()
        .storage
        .read_note(&updated.note.file_path)
        .expect("读取文件");
    assert_eq!(stored.frontmatter.source_type.as_deref(), Some("official"));
    assert_eq!(stored.frontmatter.source_ref.as_deref(), Some("https://tauri.app/"));
    assert_eq!(stored.frontmatter.source_reliability, Some(5));
    assert_eq!(stored.frontmatter.valid_until.as_deref(), Some("2027-01-01"));
    assert_eq!(stored.frontmatter.title.as_deref(), Some("Tauri 选型依据"));
    assert_eq!(stored.frontmatter.id.as_deref(), Some(detail.note.id.as_str()));

    // 清空部分字段
    let cleared = notes
        .set_source(SetSourceInput {
            id: detail.note.id.clone(),
            source_type: Some("experiment".into()),
            source_ref: None,
            source_reliability: Some(9), // 越界应被忽略
            valid_until: None,
        })
        .expect("再次更新");
    assert_eq!(cleared.note.source_type.as_deref(), Some("experiment"));
    assert_eq!(cleared.note.source_ref, None);
    assert_eq!(cleared.note.source_reliability, None, "越界等级应被忽略");
    assert_eq!(cleared.note.valid_until, None);
}

#[test]
fn external_frontmatter_edit_syncs_source_metadata() {
    let fixture = Fixture::new();
    let notes = fixture.notes();
    let detail = notes
        .create(CreateNoteInput {
            title: "外部编辑来源".into(),
            category_id: None,
            initial_content: None,
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");

    // 模拟外部编辑器：直接改文件 frontmatter
    let absolute = fixture.root.join(&detail.note.file_path);
    let raw = std::fs::read_to_string(&absolute).expect("读取");
    let patched = raw.replace(
        "---\n",
        "---\nsource_type: book\nsource_ref: 《设计数据密集型应用》\nsource_reliability: 4\nvalid_until: 2026-12-31\n",
    );
    std::fs::write(&absolute, patched).expect("写入");

    IndexService::new(fixture.app.context().unwrap())
        .scan_workspace()
        .expect("扫描");

    let refreshed = notes.get(&detail.note.id).expect("读取笔记");
    assert_eq!(refreshed.note.source_type.as_deref(), Some("book"));
    assert_eq!(refreshed.note.source_reliability, Some(4));
    assert_eq!(refreshed.note.valid_until.as_deref(), Some("2026-12-31"));
}

#[test]
fn save_rename_move_tags_keep_source_and_relations() {
    // 回归：以前 save / rename / move / set_tags 会把来源与关系字段重置为空
    let fixture = Fixture::new();
    let notes = fixture.notes();
    let detail = notes
        .create(CreateNoteInput {
            title: "元数据保真".into(),
            category_id: None,
            initial_content: Some("正文".into()),
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");

    notes
        .set_source(SetSourceInput {
            id: detail.note.id.clone(),
            source_type: Some("book".into()),
            source_ref: Some("《领域驱动设计》".into()),
            source_reliability: Some(4),
            valid_until: Some("2030-01-01".into()),
        })
        .expect("写入来源");

    // 1) 保存正文
    notes
        .save(personal_knowledge_os_lib::domain::note::SaveNoteInput {
            id: detail.note.id.clone(),
            content: "正文已更新".into(),
            base_content_hash: None,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("保存");
    let after_save = notes.get(&detail.note.id).expect("读取");
    assert_eq!(after_save.note.source_type.as_deref(), Some("book"));
    assert_eq!(after_save.note.source_reliability, Some(4));
    assert_eq!(after_save.note.valid_until.as_deref(), Some("2030-01-01"));

    // 2) 改标签
    notes.set_tags(&detail.note.id, &["元数据".into()]).expect("改标签");
    let after_tags = notes.get(&detail.note.id).expect("读取");
    assert_eq!(after_tags.note.source_type.as_deref(), Some("book"));

    // 3) 重命名
    let renamed = notes.rename(&detail.note.id, "元数据保真 v2", false).expect("重命名");
    assert_eq!(renamed.note.source_ref.as_deref(), Some("《领域驱动设计》"));

    // 4) 移动分类
    let category = personal_knowledge_os_lib::application::category_service::CategoryService::new(
        fixture.app.context().unwrap(),
    )
    .create(personal_knowledge_os_lib::domain::category::CreateCategoryInput {
        name: "保真分类".into(),
        parent_id: None,
        create_directory: None,
    })
    .expect("建分类");
    let moved = notes
        .move_to_category(&detail.note.id, Some(&category.id))
        .expect("移动");
    assert_eq!(moved.note.source_reliability, Some(4));
    assert_eq!(moved.note.valid_until.as_deref(), Some("2030-01-01"));

    // 文件里同样保留
    let stored = fixture
        .app
        .context()
        .unwrap()
        .storage
        .read_note(&moved.note.file_path)
        .expect("读取文件");
    assert_eq!(stored.frontmatter.source_type.as_deref(), Some("book"));
}
