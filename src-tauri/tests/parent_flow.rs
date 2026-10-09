//! 子文档层级（V1.2.1）集成测试。

use std::sync::Arc;

use personal_knowledge_os_lib::application::category_service::CategoryService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, NoteListQuery, SetParentInput};
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
            .create("层级测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }
}

#[test]
fn creates_child_notes_inheriting_category_and_lists_parent_id() {
    let fixture = Fixture::new();
    let category = CategoryService::new(fixture.app.context().unwrap())
        .create(CreateCategoryInput {
            name: "层级分类".into(),
            parent_id: None,
            create_directory: None,
        })
        .expect("建分类");
    let parent = fixture
        .notes()
        .create(CreateNoteInput {
            title: "父文档".into(),
            category_id: Some(category.id.clone()),
            initial_content: Some("父文档正文".into()),
            tags: None,
            parent_id: None,
        })
        .expect("创建父文档");

    // 子文档：不指定分类 → 继承父文档分类
    let child = fixture
        .notes()
        .create(CreateNoteInput {
            title: "子文档".into(),
            category_id: None,
            initial_content: Some("子文档正文".into()),
            tags: None,
            parent_id: Some(parent.note.id.clone()),
        })
        .expect("创建子文档");

    assert_eq!(child.note.parent_id.as_deref(), Some(parent.note.id.as_str()));
    assert_eq!(
        child.note.category_id.as_deref(),
        Some(category.id.as_str()),
        "子文档应继承父文档分类"
    );

    // frontmatter 里写入的是父文档 id（可被外部编辑器看到）
    let stored = fixture
        .app
        .context()
        .unwrap()
        .storage
        .read_note(&child.note.file_path)
        .expect("读取子文档");
    assert_eq!(stored.frontmatter.parent.as_deref(), Some(parent.note.id.as_str()));

    // 列表摘要带 parentId，便于前端建树
    let items = fixture.notes().list(NoteListQuery::default()).expect("列表").items;
    let found = items.iter().find(|item| item.id == child.note.id).expect("列表含子文档");
    assert_eq!(found.parent_id.as_deref(), Some(parent.note.id.as_str()));
}

#[test]
fn set_parent_moves_between_levels_and_rejects_cycles() {
    let fixture = Fixture::new();
    let a = fixture
        .notes()
        .create(CreateNoteInput {
            title: "A 文档".into(),
            category_id: None,
            initial_content: None,
            tags: None,
            parent_id: None,
        })
        .expect("创建 A");
    let b = fixture
        .notes()
        .create(CreateNoteInput {
            title: "B 文档".into(),
            category_id: None,
            initial_content: None,
            tags: None,
            parent_id: None,
        })
        .expect("创建 B");

    // B 挂到 A 下（按标题解析）
    let moved = fixture
        .notes()
        .set_parent(SetParentInput { id: b.note.id.clone(), parent_id: Some("A 文档".into()) })
        .expect("设置父文档");
    assert_eq!(moved.note.parent_id.as_deref(), Some(a.note.id.as_str()));

    // 防环：A 不能挂到 B（B 已是 A 的子）下
    assert!(
        fixture
            .notes()
            .set_parent(SetParentInput { id: a.note.id.clone(), parent_id: Some(b.note.id.clone()) })
            .is_err(),
        "应拒绝把父文档挂到自己的子文档下"
    );
    // 自己也不能作为自己的父
    assert!(fixture
        .notes()
        .set_parent(SetParentInput { id: a.note.id.clone(), parent_id: Some(a.note.id.clone()) })
        .is_err());

    // 移回顶层
    let top = fixture
        .notes()
        .set_parent(SetParentInput { id: b.note.id.clone(), parent_id: None })
        .expect("移回顶层");
    assert!(top.note.parent_id.is_none());
    let stored = fixture
        .app
        .context()
        .unwrap()
        .storage
        .read_note(&b.note.file_path)
        .expect("读取 B");
    assert!(stored.frontmatter.parent.is_none(), "frontmatter 的 parent 应被清空");
}

#[test]
fn rename_and_delete_of_parent_keep_children_stable() {
    let fixture = Fixture::new();
    let parent = fixture
        .notes()
        .create(CreateNoteInput {
            title: "父文档".into(),
            category_id: None,
            initial_content: None,
            tags: None,
            parent_id: None,
        })
        .expect("创建父文档");
    let child = fixture
        .notes()
        .create(CreateNoteInput {
            title: "子文档".into(),
            category_id: None,
            initial_content: None,
            tags: None,
            parent_id: Some(parent.note.id.clone()),
        })
        .expect("创建子文档");

    // 父文档重命名：父子关系按 id 保存，不受影响
    fixture.notes().rename(&parent.note.id, "父文档（改名）", false).expect("重命名父文档");
    let still_child = fixture.notes().get(&child.note.id).expect("读取子文档");
    assert_eq!(still_child.note.parent_id.as_deref(), Some(parent.note.id.as_str()));

    // 父文档移入回收站：子文档保留，parent_id 仍指向父（列表按根级展示）
    fixture.notes().delete(&parent.note.id).expect("删除父文档");
    let after = fixture.notes().get(&child.note.id).expect("读取子文档");
    assert_eq!(after.note.parent_id.as_deref(), Some(parent.note.id.as_str()));
    let items = fixture.notes().list(NoteListQuery::default()).expect("列表").items;
    assert!(items.iter().any(|item| item.id == child.note.id), "子文档不应随父文档消失");
}
