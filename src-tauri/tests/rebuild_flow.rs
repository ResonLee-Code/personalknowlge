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
