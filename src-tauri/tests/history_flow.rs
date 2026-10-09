//! 版本历史（V1.2.1 P2）集成测试。
//!
//! 覆盖：保存自动留档、快照列表与读取、还原只回滚正文、节流与数量上限、
//! 永久删除时清理历史目录。

use std::sync::Arc;

use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SaveNoteInput};
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
            .create("历史测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, workspace, app }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }

    fn save(&self, id: &str, content: &str) {
        self.notes()
            .save(SaveNoteInput {
                id: id.into(),
                content: content.into(),
                base_content_hash: None,
                title: None,
                category_id: None,
                tags: None,
            })
            .expect("保存");
    }

    fn history_dir(&self, note_id: &str) -> std::path::PathBuf {
        self.workspace
            .path()
            .join(".knowledge")
            .join("history")
            .join(note_id)
    }
}

#[test]
fn saving_keeps_previous_content_as_snapshot() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "历史笔记".into(),
            category_id: None,
            initial_content: Some("第一版正文".into()),
            tags: None,
            parent_id: None,
        })
        .expect("创建")
        .note
        .id;

    // 首次保存：写入前的内容是「第一版正文」
    fixture.save(&note, "第二版正文");
    let entries = fixture.notes().history_list(&note).expect("列出历史");
    assert_eq!(entries.len(), 1, "应留下 1 份历史快照");
    let snapshot = fixture
        .notes()
        .history_read(&note, &entries[0].entry_id)
        .expect("读取快照");
    assert!(snapshot.body.contains("第一版正文"), "快照应是写入前的内容");
    assert!(snapshot.content.contains("title:"), "快照保留完整 frontmatter");
    assert_eq!(entries[0].title, "历史笔记");
    assert!(!entries[0].saved_at.is_empty(), "快照带时间戳");

    // 快照落在工作区内，是可读的普通文件
    let files: Vec<_> = std::fs::read_dir(fixture.history_dir(&note))
        .expect("历史目录存在")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(files.len(), 1);

    // 立刻再保存一次：60 秒节流内不重复留档（否则自动保存会刷爆历史）
    fixture.save(&note, "第三版正文");
    assert_eq!(
        fixture.notes().history_list(&note).expect("列出历史").len(),
        1,
        "节流窗口内不应新增快照"
    );
}

#[test]
fn restoring_a_version_rolls_back_body_only() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "还原笔记".into(),
            category_id: None,
            initial_content: Some("旧正文".into()),
            tags: Some(vec!["标签A".into()]),
            parent_id: None,
        })
        .expect("创建")
        .note
        .id;

    fixture.save(&note, "新正文");
    // 改标题后再还原：标题应保持「改后的标题」，只回滚正文
    fixture
        .notes()
        .rename(&note, "改后的标题", false)
        .expect("重命名");

    let entries = fixture.notes().history_list(&note).expect("列出历史");
    let detail = fixture
        .notes()
        .history_restore(&note, &entries[0].entry_id)
        .expect("还原");
    assert!(detail.content.contains("旧正文"), "正文应回到快照内容");
    assert!(!detail.content.contains("新正文"));
    assert_eq!(detail.note.title, "改后的标题", "还原内容不应改动标题");
    assert!(
        detail.tags.iter().any(|tag| tag.name == "标签A"),
        "标签也应保留"
    );

    // 还原本身也要留档：现在应有两份快照（还原前的「新正文」也被记下来了）
    let after = fixture.notes().history_list(&note).expect("列出历史");
    assert_eq!(after.len(), 2, "还原前的内容同样会被留档");
    let newest = fixture
        .notes()
        .history_read(&note, &after[0].entry_id)
        .expect("读取最新快照");
    assert!(newest.body.contains("新正文"), "最新快照是还原前的状态");
}

#[test]
fn history_keeps_at_most_fifty_snapshots_and_can_be_removed() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "上限笔记".into(),
            category_id: None,
            initial_content: Some("v0".into()),
            tags: None,
            parent_id: None,
        })
        .expect("创建")
        .note
        .id;

    // 直接调用 HistoryService::capture_forced 绕过节流，模拟长期编辑
    let history = personal_knowledge_os_lib::application::history_service::HistoryService::new(
        fixture.app.context().expect("工作区已打开"),
    );
    let service = fixture.notes();
    for index in 1..=55 {
        history.capture_forced(&note, &format!("v{index}")).expect("留档");
    }
    let entries = service.history_list(&note).expect("列出历史");
    assert_eq!(entries.len(), 50, "每篇最多保留 50 份");

    // 删除单个快照
    let remaining = service
        .history_remove(&note, &entries[0].entry_id)
        .expect("删除快照");
    assert_eq!(remaining, 49);
    assert!(service.history_read(&note, &entries[0].entry_id).is_err());
}

#[test]
fn purging_a_note_removes_its_history_directory() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "待清理".into(),
            category_id: None,
            initial_content: Some("内容".into()),
            tags: None,
            parent_id: None,
        })
        .expect("创建")
        .note
        .id;
    fixture.save(&note, "改过");
    assert!(fixture.history_dir(&note).is_dir());

    fixture.notes().delete(&note).expect("移入回收站");
    fixture.notes().purge(&note).expect("永久删除");
    assert!(!fixture.history_dir(&note).exists(), "历史目录应一并清理");
}
