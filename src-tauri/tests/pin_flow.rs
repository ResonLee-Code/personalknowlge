//! 置顶文档（V1.2.1，对标语雀「置顶文档」）集成测试。
//!
//! 断言三件事：写回 frontmatter 的 `pinned`、在所有排序方式下排最前、
//! 以及置顶状态可以从 Markdown 重建（派生列而非事实来源）。

use std::sync::Arc;

use personal_knowledge_os_lib::application::index_service::IndexService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, NoteListQuery, NoteSort, ReorderNotesInput};
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
            .create("置顶测试", &workspace.path().to_string_lossy())
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

    fn titles(&self, sort: NoteSort) -> Vec<String> {
        self.notes()
            .list(NoteListQuery { sort: Some(sort), page_size: Some(50), ..Default::default() })
            .expect("列表")
            .items
            .into_iter()
            .map(|item| item.title)
            .collect()
    }
}

#[test]
fn pinned_note_is_written_to_frontmatter_and_sorted_first() {
    let fixture = Fixture::new();
    let _a = fixture.create("甲");
    let _b = fixture.create("乙");
    let c = fixture.create("丙");
    let before = fixture.titles(NoteSort::UpdatedDesc);
    // 置顶的对象取「本篇测试创建的笔记里自然排最后」的那篇，便于观察是否被提到最前
    let mine = ["甲", "乙", "丙"];
    let target = before
        .iter()
        .rev()
        .find(|title| mine.contains(&title.as_str()))
        .expect("测试笔记都在列表里")
        .clone();
    let target_id = [_a.clone(), _b.clone(), c.clone()]
        .into_iter()
        .find(|id| fixture.notes().get(id).expect("读取").note.title == target)
        .expect("找到笔记 id");

    let detail = fixture.notes().set_pinned(&target_id, true).expect("置顶");
    assert!(detail.note.pinned);

    // frontmatter 是唯一事实来源
    let absolute = fixture.workspace.path().join(&detail.note.file_path);
    let raw = std::fs::read_to_string(&absolute).expect("读文件");
    assert!(raw.contains("pinned: true"), "frontmatter 应写入 pinned\n{raw}");

    for sort in [NoteSort::UpdatedDesc, NoteSort::TitleAsc, NoteSort::Manual, NoteSort::CreatedDesc] {
        assert_eq!(fixture.titles(sort)[0], target, "置顶应排在最前（{sort:?}）");
    }

    // 置顶不改写 updated_at
    assert_eq!(
        detail.note.updated_at,
        fixture.notes().get(&target_id).expect("读取置顶笔记").note.updated_at
    );

    // 取消置顶后回到普通顺序
    let unpinned = fixture.notes().set_pinned(&target_id, false).expect("取消置顶");
    assert!(!unpinned.note.pinned);
    let raw_after = std::fs::read_to_string(&absolute).expect("读文件");
    assert!(!raw_after.contains("pinned: true"), "取消置顶应移除 frontmatter 字段");
    assert_eq!(fixture.titles(NoteSort::UpdatedDesc), before, "取消置顶应回到原顺序");
}

#[test]
fn pinned_state_survives_rescan_and_can_be_hand_edited() {
    let fixture = Fixture::new();
    let a = fixture.create("甲");
    fixture.create("乙");
    fixture.create("丙");
    fixture.notes().set_pinned(&a, true).expect("置顶甲");
    assert_eq!(fixture.titles(NoteSort::UpdatedDesc)[0], "甲");

    // 模拟数据库重建：整库重扫后仍应保持置顶
    IndexService::new(fixture.app.context().unwrap())
        .scan_workspace()
        .expect("重扫工作区");
    assert_eq!(fixture.titles(NoteSort::UpdatedDesc)[0], "甲", "置顶应从 Markdown 重建");

    // 用户手写 frontmatter 也能生效（`pinned: true` 投影到派生列）
    let b = fixture.create("乙2");
    let detail = fixture.notes().get(&b).expect("读取");
    let absolute = fixture.workspace.path().join(&detail.note.file_path);
    let raw = std::fs::read_to_string(&absolute).expect("读文件");
    let patched = raw.replace("status: active", "status: active\npinned: true\norder: 7");
    assert_ne!(patched, raw, "手写 frontmatter 失败，原文：\n{raw}");
    std::fs::write(&absolute, patched).expect("手写 frontmatter");
    IndexService::new(fixture.app.context().unwrap())
        .scan_workspace()
        .expect("重扫");
    let reloaded = fixture.notes().get(&b).expect("读取");
    assert!(reloaded.note.pinned, "外部写入的 pinned 应同步到库");
    assert_eq!(reloaded.note.sort_order, 7, "外部写入的 order 应同步到库");
}

#[test]
fn pinning_does_not_disturb_manual_order_inside_the_group() {
    let fixture = Fixture::new();
    let a = fixture.create("甲");
    let b = fixture.create("乙");
    let c = fixture.create("丙");

    fixture
        .notes()
        .reorder(ReorderNotesInput { ids: vec![c.clone(), b.clone(), a.clone()] })
        .expect("重排");
    assert_eq!(&fixture.titles(NoteSort::Manual)[..3], ["丙", "乙", "甲"]);

    // 置顶其中一篇后：它排最前，其余保持原有相对顺序
    fixture.notes().set_pinned(&b, true).expect("置顶乙");
    let titles = fixture.titles(NoteSort::Manual);
    assert_eq!(&titles[..3], ["乙", "丙", "甲"], "{titles:?}");
}
