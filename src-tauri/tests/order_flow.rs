//! 手动排序（自定义顺序，V1.2.1）集成测试。
//!
//! 关注三件事：
//! 1. `notes_reorder` 把顺序写进 frontmatter 的 `order:`，并投影到 `notes.sort_order`；
//! 2. `manual` 排序按 order 返回列表，未排序（0）的笔记用更新时间兜底；
//! 3. 顺序以文件为准：删库重扫后仍能重建（不会被数据库锁死）。

use std::sync::Arc;

use personal_knowledge_os_lib::application::index_service::IndexService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::{
    CreateNoteInput, NoteListQuery, NoteSort, ReorderNotesInput, SetParentInput,
};
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
            .create("排序测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, workspace, app }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }

    fn create(&self, title: &str) -> String {
        self.notes()
            .create(CreateNoteInput {
                title: title.into(),
                category_id: None,
                initial_content: Some(format!("{title} 的正文")),
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记")
            .note
            .id
    }

    fn manual_titles(&self) -> Vec<String> {
        self.notes()
            .list(NoteListQuery {
                sort: Some(NoteSort::Manual),
                page_size: Some(50),
                ..Default::default()
            })
            .expect("按自定义顺序列出")
            .items
            .into_iter()
            .map(|item| item.title)
            .collect()
    }
}

#[test]
fn reorder_writes_frontmatter_order_and_manual_sort_follows_it() {
    let fixture = Fixture::new();
    let a = fixture.create("甲");
    let b = fixture.create("乙");
    let c = fixture.create("丙");
    let updated_before: Vec<String> = fixture
        .notes()
        .list(NoteListQuery {
            sort: Some(NoteSort::UpdatedDesc),
            page_size: Some(50),
            ..Default::default()
        })
        .expect("按更新时间列出")
        .items
        .into_iter()
        .map(|item| item.title)
        .collect();
    let stamp_before = fixture.notes().get(&c).expect("读取丙").note.updated_at;

    let updated = fixture
        .notes()
        .reorder(ReorderNotesInput { ids: vec![c.clone(), a.clone(), b.clone()] })
        .expect("重排");
    // 同层归一化：传 3 篇，工作区预置的欢迎笔记也一起排序（排在后面），因此是 4 篇
    assert_eq!(updated, 4, "同层全部笔记都应写入新的 order");
    let titles = fixture.manual_titles();
    assert_eq!(&titles[..3], ["丙", "甲", "乙"], "传入顺序在前：{titles:?}");
    assert!(titles[3].starts_with("欢迎使用"), "未被指定的同层笔记排在后面：{titles:?}");

    // frontmatter 是唯一事实来源：order 必须落盘
    let detail = fixture.notes().get(&c).expect("读取丙");
    let absolute = fixture.workspace.path().join(&detail.note.file_path);
    let raw = std::fs::read_to_string(&absolute).expect("读文件");
    assert!(raw.contains("order: 1"), "frontmatter 应写入 order: 1\n{raw}");

    // 列表摘要也带上排序位次，前端才能按同一套键渲染
    let summary = fixture
        .notes()
        .list(NoteListQuery {
            sort: Some(NoteSort::Manual),
            page_size: Some(50),
            ..Default::default()
        })
        .expect("列表");
    assert_eq!(summary.items[0].sort_order, 1);

    // 默认排序（最近修改）完全不变，且排序不算是内容改动（updated_at 保持不变）
    let updated_after: Vec<String> = fixture
        .notes()
        .list(NoteListQuery {
            sort: Some(NoteSort::UpdatedDesc),
            page_size: Some(50),
            ..Default::default()
        })
        .expect("按更新时间列出")
        .items
        .into_iter()
        .map(|item| item.title)
        .collect();
    assert_eq!(updated_after, updated_before, "自定义顺序不应改变「最近修改」的顺序");
    assert_eq!(
        fixture.notes().get(&c).expect("读取丙").note.updated_at,
        stamp_before,
        "拖拽排序不应改写 updated_at"
    );
}

#[test]
fn reorder_is_per_sibling_group_for_child_notes() {
    let fixture = Fixture::new();
    let parent = fixture.create("父文档");
    let child_a = fixture.create("子文档甲");
    let child_b = fixture.create("子文档乙");
    let top = fixture.create("顶层笔记");

    for child in [&child_a, &child_b] {
        fixture
            .notes()
            .set_parent(SetParentInput { id: child.clone(), parent_id: Some(parent.clone()) })
            .expect("设为子文档");
    }
    // 只重排子文档这一层
    fixture
        .notes()
        .reorder(ReorderNotesInput { ids: vec![child_b.clone(), child_a.clone()] })
        .expect("重排子文档");

    let items = fixture
        .notes()
        .list(NoteListQuery {
            sort: Some(NoteSort::Manual),
            page_size: Some(50),
            ..Default::default()
        })
        .expect("列表")
        .items;
    let child_order: Vec<String> = items
        .iter()
        .filter(|item| item.parent_id.as_deref() == Some(parent.as_str()))
        .map(|item| item.title.clone())
        .collect();
    assert_eq!(child_order, vec!["子文档乙", "子文档甲"]);
    // 顶层笔记没有 order，保持 0（用更新时间兜底），不影响子层顺序
    let top_summary = items.iter().find(|item| item.id == top).expect("顶层笔记");
    assert_eq!(top_summary.sort_order, 0);
}

#[test]
fn order_is_rebuilt_from_files_after_rescan() {
    let fixture = Fixture::new();
    let a = fixture.create("甲");
    let b = fixture.create("乙");
    fixture
        .notes()
        .reorder(ReorderNotesInput { ids: vec![b.clone(), a.clone()] })
        .expect("重排");

    // 模拟数据库丢失/损坏后的重建：整库重扫
    IndexService::new(fixture.app.context().unwrap())
        .scan_workspace()
        .expect("重扫工作区");

    let titles = fixture.manual_titles();
    assert_eq!(&titles[..2], ["乙", "甲"], "顺序应从 Markdown 的 order 重建：{titles:?}");
}

#[test]
fn reorder_ignores_unknown_ids_and_rejects_empty_input() {
    let fixture = Fixture::new();
    let a = fixture.create("甲");
    let updated = fixture
        .notes()
        .reorder(ReorderNotesInput { ids: vec!["note_missing".into(), a.clone()] })
        .expect("已删除的 id 跳过");
    // 不存在的 id 被忽略；同层归一化会把「甲」和预置的欢迎笔记一起写入 order
    assert_eq!(updated, 2);
    assert_eq!(fixture.manual_titles()[0], "甲", "已知笔记仍按传入顺序排在最前");
    assert!(fixture.notes().reorder(ReorderNotesInput { ids: vec![] }).is_err());
}
