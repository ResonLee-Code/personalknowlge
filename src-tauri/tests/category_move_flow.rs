//! 分类移动（0 信任：这是会**挪磁盘目录 + 改笔记路径**的操作，之前没有测试覆盖）。
//!
//! 覆盖：把分类移到顶层 / 移到别的父分类（含整棵子树与其中的笔记）、防环（移到自己或后代之下）、
//! 目标父分类不存在、以及移动后重开工作区（扫描）仍然稳定。

use std::sync::Arc;

use personal_knowledge_os_lib::application::category_service::CategoryService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, NoteListQuery};
use personal_knowledge_os_lib::domain::search::SearchQuery;
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
            .create("分类移动测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn categories(&self) -> CategoryService {
        CategoryService::new(self.context())
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.context())
    }

    fn absolute(&self, relative: &str) -> std::path::PathBuf {
        self.workspace.path().join(relative.replace('\\', "/"))
    }

    fn category_by_name(&self, name: &str) -> personal_knowledge_os_lib::domain::category::Category {
        self.categories()
            .list()
            .expect("分类列表")
            .into_iter()
            .find(|category| category.name == name)
            .unwrap_or_else(|| panic!("分类 {name} 应存在"))
    }
}

#[test]
fn moving_a_category_carries_its_subtree_notes_and_paths() {
    let fixture = Fixture::new();
    let categories = fixture.categories();
    let tech = fixture.category_by_name("技术");

    // 技术/数据库/索引（三层），每层各放一篇笔记
    let database = categories
        .create(CreateCategoryInput {
            name: "数据库".to_string(),
            parent_id: Some(tech.id.clone()),
            create_directory: Some(true),
        })
        .expect("建子分类");
    let index = categories
        .create(CreateCategoryInput {
            name: "索引".to_string(),
            parent_id: Some(database.id.clone()),
            create_directory: Some(true),
        })
        .expect("建孙分类");

    let in_tech = fixture
        .notes()
        .create(CreateNoteInput {
            title: "技术层笔记".to_string(),
            category_id: Some(tech.id.clone()),
            initial_content: Some("技术".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("技术层笔记")
        .note;
    let in_database = fixture
        .notes()
        .create(CreateNoteInput {
            title: "数据库层笔记".to_string(),
            category_id: Some(database.id.clone()),
            initial_content: Some("数据库".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("数据库层笔记")
        .note;
    let in_index = fixture
        .notes()
        .create(CreateNoteInput {
            title: "索引层笔记".to_string(),
            category_id: Some(index.id.clone()),
            initial_content: Some("索引".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("索引层笔记")
        .note;

    let old_database_dir = database.dir_name.clone();
    let old_index_dir = index.dir_name.clone();
    // `dir_name` 存的是**叶子目录名**，完整路径要沿父链拼出来（`CategoryLookup::directory_of`）
    let lookup = fixture.context().category_lookup().expect("分类查找");
    let tech_dir = lookup.directory_of(&tech.id);
    let database_dir_before = lookup.directory_of(&database.id);
    let index_dir_before = lookup.directory_of(&index.id);
    assert_eq!(
        database_dir_before,
        format!("{tech_dir}/{old_database_dir}"),
        "子分类的完整目录应是父目录 + 叶子名"
    );
    assert_eq!(
        index_dir_before,
        format!("{database_dir_before}/{old_index_dir}"),
        "孙分类同理"
    );
    assert!(fixture.absolute(&database_dir_before).is_dir(), "分类目录应已创建");
    assert!(
        fixture.absolute(&in_index.file_path).exists(),
        "三层笔记文件应在磁盘上"
    );

    // 把「数据库」整棵子树移到顶层
    let moved = categories
        .move_category(&database.id, None)
        .expect("移动到顶层");
    assert_eq!(moved.parent_id, None, "移动后父分类应为空");
    let lookup_after = fixture.context().category_lookup().expect("分类查找");
    let database_dir_after = lookup_after.directory_of(&database.id);
    let index_dir_after = lookup_after.directory_of(&index.id);
    assert_ne!(
        database_dir_after, database_dir_before,
        "移动到顶层后完整目录应当变化"
    );
    assert!(
        !database_dir_after.contains(&format!("{tech_dir}/")),
        "新目录不应再挂在技术下面：{database_dir_after}"
    );

    // 目录整体搬走：老目录消失、新目录里保留子目录与文件
    assert!(
        !fixture.absolute(&database_dir_before).exists(),
        "老目录应被移走"
    );
    assert!(fixture.absolute(&database_dir_after).is_dir(), "新目录应存在");
    let moved_index_dir = index_dir_after.clone();
    assert!(
        fixture.absolute(&moved_index_dir).is_dir(),
        "子分类目录应随父目录一起移动：{moved_index_dir}"
    );

    // 笔记路径跟着更新，且文件真的在新的位置
    let refreshed_database_note = fixture.notes().get(&in_database.id).expect("读数据库层笔记");
    assert!(
        refreshed_database_note
            .note
            .file_path
            .starts_with(&format!("{database_dir_after}/")),
        "直属笔记路径要更新：{}",
        refreshed_database_note.note.file_path
    );
    assert!(fixture.absolute(&refreshed_database_note.note.file_path).exists());
    let refreshed_index_note = fixture.notes().get(&in_index.id).expect("读索引层笔记");
    assert!(
        refreshed_index_note
            .note
            .file_path
            .starts_with(&format!("{database_dir_after}/")),
        "子分类里的笔记路径也要更新：{}",
        refreshed_index_note.note.file_path
    );
    assert!(fixture.absolute(&refreshed_index_note.note.file_path).exists());

    // 技术层的笔记完全不受影响
    let untouched = fixture.notes().get(&in_tech.id).expect("读技术层笔记");
    assert_eq!(untouched.note.file_path, in_tech.file_path);
    assert!(fixture.absolute(&untouched.note.file_path).exists());

    // 分类归属（id）不变，只是层级变了
    assert_eq!(refreshed_database_note.note.category_id.as_deref(), Some(database.id.as_str()));
    assert_eq!(refreshed_index_note.note.category_id.as_deref(), Some(index.id.as_str()));

    // 移动后仍然可检索（路径变了但索引与元数据一致）
    let hits = SearchService::new(fixture.context())
        .search(SearchQuery {
            keyword: "数据库层笔记".to_string(),
            page: Some(1),
            page_size: Some(10),
            ..Default::default()
        })
        .expect("检索");
    assert!(hits.hits.iter().any(|hit| hit.note.id == in_database.id));

    // 再移回「技术」下面：路径应回到原位（目录名按父级重新编号）
    let moved_back = categories
        .move_category(&database.id, Some(tech.id.as_str()))
        .expect("移回技术");
    assert_eq!(moved_back.parent_id.as_deref(), Some(tech.id.as_str()));
    let lookup_back = fixture.context().category_lookup().expect("分类查找");
    let database_dir_back = lookup_back.directory_of(&database.id);
    assert_eq!(
        database_dir_back, database_dir_before,
        "移回原父分类后目录应回到原位"
    );
    let back_note = fixture.notes().get(&in_index.id).expect("移回后读索引层笔记");
    assert!(fixture.absolute(&back_note.note.file_path).exists(), "移回后文件应在原位");
    assert!(
        back_note.note.file_path.starts_with(&format!("{database_dir_back}/")),
        "移回后路径应按新目录：{}",
        back_note.note.file_path
    );

    // 重开工作区（重新扫描）后仍然一致：分类、路径、笔记数量都对
    WorkspaceService::new(Arc::clone(&fixture.app))
        .close()
        .expect("关闭");
    WorkspaceService::new(Arc::clone(&fixture.app))
        .open(&fixture.workspace.path().to_string_lossy())
        .expect("重新打开");
    let listed = fixture
        .notes()
        .list(NoteListQuery { page_size: Some(200), ..Default::default() })
        .expect("列表");
    for note_id in [&in_tech.id, &in_database.id, &in_index.id] {
        let summary = listed
            .items
            .iter()
            .find(|item| &item.id == note_id)
            .expect("重开扫描后仍然存在");
        assert!(
            fixture.absolute(&summary.file_path).exists(),
            "库里的路径必须指向真实文件：{}",
            summary.file_path
        );
    }
    assert_eq!(
        fixture.category_by_name("索引").parent_id.as_deref(),
        Some(database.id.as_str()),
        "重开后子分类关系不变"
    );
}

#[test]
fn moving_a_category_under_itself_or_a_descendant_is_rejected() {
    let fixture = Fixture::new();
    let categories = fixture.categories();
    let tech = fixture.category_by_name("技术");
    let database = categories
        .create(CreateCategoryInput {
            name: "数据库".to_string(),
            parent_id: Some(tech.id.clone()),
            create_directory: Some(true),
        })
        .expect("建子分类");
    let index = categories
        .create(CreateCategoryInput {
            name: "索引".to_string(),
            parent_id: Some(database.id.clone()),
            create_directory: Some(true),
        })
        .expect("建孙分类");

    let self_move = categories
        .move_category(&database.id, Some(database.id.as_str()))
        .expect_err("不能移到自己下面");
    assert_eq!(self_move.code.as_str(), "CATEGORY_MOVE_INVALID");

    let descendant_move = categories
        .move_category(&database.id, Some(index.id.as_str()))
        .expect_err("不能移到自己的后代下面");
    assert_eq!(descendant_move.code.as_str(), "CATEGORY_MOVE_INVALID");

    let missing_parent = categories
        .move_category(&database.id, Some("cat_不存在"))
        .expect_err("目标父分类不存在");
    assert_eq!(missing_parent.code.as_str(), "CATEGORY_NOT_FOUND");

    // 失败的移动不能改变任何东西
    let unchanged = fixture.category_by_name("数据库");
    assert_eq!(unchanged.parent_id.as_deref(), Some(tech.id.as_str()));
    assert_eq!(unchanged.dir_name, database.dir_name);
}
