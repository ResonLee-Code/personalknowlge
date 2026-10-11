//! 「Markdown 是唯一事实来源」验收（0 信任）。
//!
//! 产品承诺：删掉 `.knowledge/database.sqlite` 之后重新打开工作区，
//! 一切都能从 Markdown 完整重建——笔记、**稳定的 id**、分类、标签、
//! 子文档层级（`parent`）、手动顺序（`order`）、置顶（`pinned`）、
//! 来源与时效（N1）、双向链接与搜索索引。
//!
//! 这个测试就是那条承诺的证据：先建库写入，**删库**，再打开，逐项对照。

use std::sync::Arc;

use personal_knowledge_os_lib::application::link_service::LinkService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::tag_service::TagService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::{
    CreateNoteInput, NoteListQuery, ReorderNotesInput, SetParentInput, SetSourceInput,
};
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::infrastructure::frontmatter;
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
            .create("重建测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.context())
    }

    /// 删掉数据库文件并重新打开工作区（模拟「数据库丢了 / 换机器只剩 Markdown」）。
    fn drop_database_and_reopen(&self) {
        WorkspaceService::new(Arc::clone(&self.app))
            .close()
            .expect("关闭工作区");
        let db = self
            .workspace
            .path()
            .join(".knowledge")
            .join("database.sqlite");
        std::fs::remove_file(&db).expect("删除数据库");
        for suffix in ["-wal", "-shm"] {
            let extra = db.with_file_name(format!("database.sqlite{suffix}"));
            if extra.exists() {
                std::fs::remove_file(extra).expect("删除 WAL/SHM");
            }
        }
        WorkspaceService::new(Arc::clone(&self.app))
            .open(&self.workspace.path().to_string_lossy())
            .expect("重新打开工作区");
    }
}

#[test]
fn deleting_the_database_rebuilds_everything_from_markdown() {
    let fixture = Fixture::new();
    let notes = fixture.notes();

    // 父文档：带来源与时效、置顶、手动顺序、标签，并在正文里链接子文档
    let parent = notes
        .create(CreateNoteInput {
            title: "重建测试父文档".to_string(),
            category_id: None,
            initial_content: Some(
                "正文：见 [[重建测试子文档]]。\n\n第二段用于确认正文完整保留。".to_string(),
            ),
            tags: Some(vec!["重建".to_string(), "验收".to_string()]),
            parent_id: None,
        })
        .expect("父文档")
        .note;
    notes
        .set_source(SetSourceInput {
            id: parent.id.clone(),
            source_type: Some("official".to_string()),
            source_ref: Some("https://example.com/spec".to_string()),
            source_reliability: Some(4),
            valid_until: Some("2027-06-30".to_string()),
        })
        .expect("写来源与时效");
    notes.set_pinned(&parent.id, true).expect("置顶");
    notes
        .reorder(ReorderNotesInput { ids: vec![parent.id.clone()] })
        .expect("设置手动顺序");

    // 子文档：挂到父文档下面
    let child = notes
        .create(CreateNoteInput {
            title: "重建测试子文档".to_string(),
            category_id: None,
            initial_content: Some("子文档正文".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("子文档")
        .note;
    notes
        .set_parent(SetParentInput {
            id: child.id.clone(),
            parent_id: Some(parent.id.clone()),
        })
        .expect("建立父子关系");

    // 删库 → 重新打开：一切必须从 Markdown 重建
    fixture.drop_database_and_reopen();
    let notes = fixture.notes();

    let list = notes
        .list(NoteListQuery { page_size: Some(200), ..Default::default() })
        .expect("笔记列表");
    let rebuilt_parent = list
        .items
        .iter()
        .find(|note| note.title == "重建测试父文档")
        .expect("父文档应被重新采纳");
    let rebuilt_child = list
        .items
        .iter()
        .find(|note| note.title == "重建测试子文档")
        .expect("子文档应被重新采纳");

    // 1) id 稳定：来自 frontmatter，不随数据库重建变化（链接与关系不会断）
    assert_eq!(rebuilt_parent.id, parent.id, "父文档 id 必须稳定");
    assert_eq!(rebuilt_child.id, child.id, "子文档 id 必须稳定");

    // 2) 分类 / 标签 / 置顶 / 手动顺序 / 父子层级
    assert_eq!(rebuilt_parent.category_id, parent.category_id);
    assert!(rebuilt_parent.pinned, "置顶要从 frontmatter 重建");
    assert!(rebuilt_parent.sort_order > 0, "手动顺序要从 frontmatter 重建");
    assert_eq!(
        rebuilt_child.parent_id.as_deref(),
        Some(parent.id.as_str()),
        "子文档层级要从 frontmatter 重建"
    );
    let tags = TagService::new(fixture.context())
        .list()
        .expect("标签列表");
    assert!(
        tags.iter().any(|tag| tag.name == "重建"),
        "标签要重建：{:?}",
        tags.iter().map(|tag| tag.name.clone()).collect::<Vec<_>>()
    );

    // 3) 来源与时效（N1）原样保留
    let detail = notes.get(&parent.id).expect("读回父文档");
    assert_eq!(detail.note.source_type.as_deref(), Some("official"));
    assert_eq!(
        detail.note.source_ref.as_deref(),
        Some("https://example.com/spec")
    );
    assert_eq!(detail.note.source_reliability, Some(4));
    assert_eq!(detail.note.valid_until.as_deref(), Some("2027-06-30"));
    assert!(detail.content.contains("第二段用于确认正文完整保留"), "正文不能被截断");

    // 4) 双向链接与反链重建
    let backlinks = LinkService::new(fixture.context())
        .backlinks(&child.id)
        .expect("反链");
    assert!(
        backlinks.iter().any(|link| link.note_id == parent.id),
        "父文档指向子文档的链接要重建：{backlinks:?}"
    );

    // 5) 搜索索引重建（删库后不需要手动点「重建索引」就能搜到）
    let hits = SearchService::new(fixture.context())
        .search(SearchQuery {
            keyword: "重建测试".to_string(),
            page: Some(1),
            page_size: Some(20),
            ..Default::default()
        })
        .expect("检索");
    assert!(
        hits.hits.len() >= 2,
        "两篇笔记都应被索引：{:?}",
        hits.hits
            .iter()
            .map(|hit| hit.note.title.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn externally_added_child_file_is_nested_even_when_the_parent_is_declared_by_title() {
    let fixture = Fixture::new();
    let notes = fixture.notes();
    let parent = notes
        .create(CreateNoteInput {
            title: "外部层级父文档".to_string(),
            category_id: None,
            initial_content: Some("父文档正文".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("父文档")
        .note;

    // 模拟「用外部编辑器新建一篇子文档」：直接往工作区写文件，parent 用**标题**声明
    let child_relative = "00_Inbox/外部新增子文档.md";
    let child_id = "note_external_child_0001";
    let markdown = format!(
        "---\nid: {child_id}\ntitle: 外部新增子文档\ntype: note\nstatus: active\nparent: 外部层级父文档\n---\n\n子文档正文\n"
    );
    std::fs::write(fixture.workspace.path().join(child_relative), markdown).expect("写文件");

    // 扫描（模拟文件监听 / 显式扫描触发的采纳）
    personal_knowledge_os_lib::application::index_service::IndexService::new(fixture.context())
        .scan_workspace()
        .expect("扫描");

    let child = notes.get(child_id).expect("子文档应被采纳");
    assert_eq!(
        child.note.parent_id.as_deref(),
        Some(parent.id.as_str()),
        "按标题声明的父文档也要在这一轮扫描里解析出来（不能等下次外部编辑）"
    );
}

#[test]
fn bulk_adoption_projects_hierarchy_order_pinned_metadata_links_and_relations() {
    let fixture = Fixture::new();
    let parent_id = "note_bulk_parent_0001";
    let parent_title = "批量层级父文档";
    let root = fixture.workspace.path().join("00_Inbox");

    // 父文档：带置顶 + 手动顺序 + 标签 + 来源元数据
    std::fs::write(
        root.join("bulk_父文档.md"),
        format!(
            "---\nid: {parent_id}\ntitle: {parent_title}\ntype: note\nstatus: active\n\
             tags:\n  - 批量\npinned: true\norder: 2\nsource_type: official\n\
             source_ref: https://example.com/bulk\nsource_reliability: 5\nvalid_until: 2027-01-31\n\
             ---\n\n父文档正文\n"
        ),
    )
    .expect("写父文档");

    // 60 篇子文档（> 50 → 走批量采纳路径）：奇数篇按 id 声明父文档，偶数篇按标题
    let mut child_ids = Vec::new();
    for index in 1..=60 {
        let child_id = format!("note_bulk_child_{index:04}");
        let declared = if index % 2 == 1 {
            parent_id.to_string()
        } else {
            parent_title.to_string()
        };
        let extra = match index {
            1 => "relations:\n  - type: RELATED_TO\n    target: 批量层级父文档\n".to_string(),
            2 => "source_type: book\nsource_reliability: 3\n".to_string(),
            3 => "order: 5\n".to_string(),
            _ => String::new(),
        };
        let body = if index == 3 {
            format!("子文档正文见 [[{parent_title}]]")
        } else {
            "子文档正文".to_string()
        };
        std::fs::write(
            root.join(format!("bulk_子文档_{index:04}.md")),
            format!(
                "---\nid: {child_id}\ntitle: 批量子文档 {index:04}\ntype: note\nstatus: active\nparent: {declared}\n{extra}---\n\n{body}\n"
            ),
        )
        .expect("写子文档");
        child_ids.push(child_id);
    }

    let report =
        personal_knowledge_os_lib::application::index_service::IndexService::new(fixture.context())
            .scan_workspace()
            .expect("扫描");
    assert!(
        report.added >= 61,
        "一次采纳 61 篇应走批量路径（added = {}）",
        report.added
    );

    let notes = fixture.notes();
    let parent = notes.get(parent_id).expect("父文档");
    assert!(parent.note.pinned, "批量路径要投影 pinned");
    assert_eq!(parent.note.sort_order, 2, "批量路径要投影 order");
    assert_eq!(parent.note.source_type.as_deref(), Some("official"));
    assert_eq!(parent.note.source_reliability, Some(5));
    assert_eq!(parent.note.valid_until.as_deref(), Some("2027-01-31"));

    // 每一篇子文档（无论按 id 还是按标题声明）都要挂上去
    for child_id in &child_ids {
        let child = notes.get(child_id).expect("子文档");
        assert_eq!(
            child.note.parent_id.as_deref(),
            Some(parent_id),
            "{} 的父文档未被解析",
            child.note.title
        );
    }
    assert_eq!(
        notes.get(&child_ids[2]).expect("第 3 篇").note.sort_order,
        5,
        "子文档的 order 也要投影"
    );
    assert_eq!(
        notes.get(&child_ids[1]).expect("第 2 篇").note.source_type.as_deref(),
        Some("book")
    );

    // 标签（批量路径在事务里写 note_tags）
    let tags = TagService::new(fixture.context()).list().expect("标签");
    assert!(tags.iter().any(|tag| tag.name == "批量"), "标签要建立");

    // 双向链接（批处理结束后统一重建）
    let backlinks = LinkService::new(fixture.context())
        .backlinks(parent_id)
        .expect("反链");
    assert!(
        backlinks.iter().any(|link| link.note_id == child_ids[2]),
        "第 3 篇指向父文档的 [[链接]] 要在批量重建后生效：{backlinks:?}"
    );

    // 类型化关系（frontmatter `relations:`）
    let relations = personal_knowledge_os_lib::application::relation_service::RelationService::new(
        fixture.context(),
    )
    .list(&child_ids[0])
    .expect("关系列表");
    assert!(
        relations
            .outgoing
            .iter()
            .any(|view| view.relation_type == "RELATED_TO" && view.target_title == parent_title),
        "frontmatter 里的 RELATED_TO 关系要建立：{relations:?}"
    );

    // FTS（批量路径在同一事务里写索引）
    let hits = SearchService::new(fixture.context())
        .search(SearchQuery {
            keyword: "批量层级父文档".to_string(),
            page: Some(1),
            page_size: Some(20),
            ..Default::default()
        })
        .expect("检索");
    assert!(hits.hits.iter().any(|hit| hit.note.id == parent_id), "批量导入后立即可检索");
}

#[test]
fn renaming_a_parent_updates_title_declared_children_and_survives_a_rebuild() {
    let fixture = Fixture::new();
    let notes = fixture.notes();
    let parent = notes
        .create(CreateNoteInput {
            title: "改名前的父文档".to_string(),
            category_id: None,
            initial_content: Some("父文档".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("父文档")
        .note;

    // 子文档按**标题**声明父文档（外部编辑器手写的常见写法）
    let child_relative = "00_Inbox/按标题声明的子文档.md";
    let child_id = "note_title_declared_child";
    std::fs::write(
        fixture.workspace.path().join(child_relative),
        format!(
            "---\nid: {child_id}\ntitle: 按标题声明的子文档\ntype: note\nstatus: active\nparent: 改名前的父文档\n---\n\n子文档正文\n"
        ),
    )
    .expect("写子文档");
    personal_knowledge_os_lib::application::index_service::IndexService::new(fixture.context())
        .scan_workspace()
        .expect("扫描");
    assert_eq!(
        notes.get(child_id).expect("子文档").note.parent_id.as_deref(),
        Some(parent.id.as_str()),
        "按标题声明的父文档应当先被解析"
    );

    // 父文档改名：子文档里的 `parent:` 声明必须跟着改，否则重建后会掉层级
    notes
        .rename(&parent.id, "改名后的父文档", false)
        .expect("重命名父文档");
    let parsed = frontmatter::parse(
        &std::fs::read_to_string(fixture.workspace.path().join(child_relative)).expect("读子文档"),
    );
    assert_eq!(
        parsed.frontmatter.parent.as_deref(),
        Some("改名后的父文档"),
        "子文档的 parent 声明要同步成新标题"
    );

    // 最强断言：删库重建之后，层级仍然在
    fixture.drop_database_and_reopen();
    let rebuilt = fixture.notes().get(child_id).expect("重建后读子文档");
    assert_eq!(
        rebuilt.note.parent_id.as_deref(),
        Some(parent.id.as_str()),
        "改名的父文档在删库重建后仍然要能解析出层级"
    );
}

#[test]
fn restoring_a_backup_keeps_hierarchy_metadata_links_and_search() {
    let fixture = Fixture::new();
    let notes = fixture.notes();

    // 父文档 + 按标题声明的子文档（故意用最容易在恢复后断掉的那种声明方式）
    let parent = notes
        .create(CreateNoteInput {
            title: "备份父文档".to_string(),
            category_id: None,
            initial_content: Some("父文档正文，见 [[备份子文档]]".to_string()),
            tags: Some(vec!["备份验收".to_string()]),
            parent_id: None,
        })
        .expect("父文档")
        .note;
    notes.set_pinned(&parent.id, true).expect("置顶");
    notes
        .reorder(ReorderNotesInput { ids: vec![parent.id.clone()] })
        .expect("手动顺序");

    let child_id = "note_backup_child_0001";
    std::fs::write(
        fixture.workspace.path().join("00_Inbox/备份子文档.md"),
        format!(
            "---\nid: {child_id}\ntitle: 备份子文档\ntype: note\nstatus: active\nparent: 备份父文档\nsource_type: official\nsource_reliability: 4\nvalid_until: 2027-09-30\n---\n\n子文档正文\n"
        ),
    )
    .expect("写子文档");
    personal_knowledge_os_lib::application::index_service::IndexService::new(fixture.context())
        .scan_workspace()
        .expect("扫描");

    // 备份 → 恢复到空目录 → 用新工作区打开
    let backup = personal_knowledge_os_lib::application::backup_service::BackupService::new(
        fixture.context(),
    )
    .create(personal_knowledge_os_lib::domain::backup::BackupCreateInput {
        target_dir: None,
        note: None,
    })
    .expect("创建备份");
    let validation = personal_knowledge_os_lib::application::backup_service::BackupService::new(
        fixture.context(),
    )
    .validate(&backup.path)
    .expect("校验备份");
    assert!(validation.valid, "备份校验细节：{:?}", validation.checks);

    let restore_root = fixture._data.path().join("restored-workspace");
    personal_knowledge_os_lib::application::backup_service::BackupService::new(fixture.context())
        .restore(personal_knowledge_os_lib::domain::backup::RestoreBackupInput {
            archive_path: backup.path.clone(),
            target_dir: restore_root.to_string_lossy().to_string(),
        })
        .expect("恢复备份");

    WorkspaceService::new(Arc::clone(&fixture.app))
        .close()
        .expect("关闭原工作区");
    WorkspaceService::new(Arc::clone(&fixture.app))
        .open(&restore_root.to_string_lossy())
        .expect("打开恢复出来的工作区");

    let restored_notes = NoteService::new(fixture.context());
    let restored_parent = restored_notes.get(&parent.id).expect("恢复后的父文档");
    let restored_child = restored_notes.get(child_id).expect("恢复后的子文档");

    assert!(restored_parent.note.pinned, "置顶要随备份恢复");
    assert!(restored_parent.note.sort_order > 0, "手动顺序要随备份恢复");
    assert_eq!(
        restored_child.note.parent_id.as_deref(),
        Some(parent.id.as_str()),
        "父子层级要随备份恢复"
    );
    assert_eq!(restored_child.note.source_type.as_deref(), Some("official"));
    assert_eq!(restored_child.note.source_reliability, Some(4));
    assert_eq!(restored_child.note.valid_until.as_deref(), Some("2027-09-30"));

    let tags = TagService::new(fixture.context()).list().expect("标签");
    assert!(tags.iter().any(|tag| tag.name == "备份验收"), "标签要随备份恢复");

    let backlinks = LinkService::new(fixture.context())
        .backlinks(child_id)
        .expect("反链");
    assert!(
        backlinks.iter().any(|link| link.note_id == parent.id),
        "双向链接要随备份恢复：{backlinks:?}"
    );

    let hits = SearchService::new(fixture.context())
        .search(SearchQuery {
            keyword: "备份父文档".to_string(),
            page: Some(1),
            page_size: Some(20),
            ..Default::default()
        })
        .expect("检索");
    assert!(
        hits.hits.iter().any(|hit| hit.note.id == parent.id),
        "恢复后的工作区要立刻可检索"
    );
}
