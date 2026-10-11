//! 外部干扰（0 信任 / 稳健性）：工作区目录被外部删除或移走时，应用不能 panic，
//! 也不能把「读不到」当成「笔记内容为空」而静默丢数据。

use std::sync::Arc;

use personal_knowledge_os_lib::application::index_service::IndexService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, NoteListQuery, SaveNoteInput};
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
            .create("外部干扰测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.context())
    }
}

#[test]
fn deleting_the_workspace_directory_reports_errors_instead_of_panicking() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "会被外部删除的笔记".to_string(),
            category_id: None,
            initial_content: Some("原始正文".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("新建笔记")
        .note;

    // 模拟「工作区目录被外部删除」（误删 / 磁盘被拔 / 换机器）
    std::fs::remove_dir_all(fixture.workspace.path()).expect("删除工作区目录");

    // 扫描：要么返回错误，要么把笔记标记为缺失——绝不能 panic 或假装一切正常
    match IndexService::new(fixture.context()).scan_workspace() {
        Ok(report) => {
            assert!(
                report.missing >= 1 || report.warnings.iter().any(|w| w.contains("读取")),
                "目录没了却不能当成「没有变化」：{report:?}"
            );
        }
        Err(error) => {
            assert!(
                !error.message.is_empty(),
                "扫描失败必须给出可读原因，而不是空消息"
            );
        }
    }

    // 保存：必须返回明确错误（不能把读不到的文件当作空内容写回去）
    let save = fixture.notes().save(SaveNoteInput {
        id: note.id.clone(),
        content: "新正文".to_string(),
        base_content_hash: None,
        title: None,
        category_id: None,
        tags: None,
    });
    assert!(save.is_err(), "工作区目录都没了，保存必须失败而不是成功");

    // 列表仍然可用（数据库还在应用数据目录之外的工作区里——这里只要求不 panic）
    let listed = fixture
        .notes()
        .list(NoteListQuery { page_size: Some(50), ..Default::default() });
    assert!(listed.is_ok() || listed.is_err(), "列表调用不得 panic");
}

#[test]
fn moving_the_workspace_directory_keeps_the_app_alive() {
    let fixture = Fixture::new();
    fixture
        .notes()
        .create(CreateNoteInput {
            title: "会被搬走的笔记".to_string(),
            category_id: None,
            initial_content: Some("正文".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("新建笔记");

    // 把整个工作区目录改名（外部移动）
    let moved = fixture.workspace.path().with_extension("moved");
    std::fs::rename(fixture.workspace.path(), &moved).expect("移走工作区目录");

    let scan = IndexService::new(fixture.context()).scan_workspace();
    match scan {
        Ok(report) => {
            assert!(
                report.missing >= 1 || !report.warnings.is_empty(),
                "目录被移走时扫描结果应当反映异常：{report:?}"
            );
        }
        Err(_) => {}
    }

    // 放回去之后应当恢复正常（不需要重启应用）
    std::fs::rename(&moved, fixture.workspace.path()).expect("放回工作区目录");
    let report = IndexService::new(fixture.context())
        .scan_workspace()
        .expect("放回后扫描应当成功");
    assert_eq!(
        report.missing, 0,
        "目录放回后不应再有缺失笔记：{report:?}"
    );
    assert!(
        report.warnings.is_empty(),
        "目录放回后不应再有读取告警：{report:?}"
    );
    let listed = fixture
        .notes()
        .list(NoteListQuery { page_size: Some(50), ..Default::default() })
        .expect("列表");
    assert!(
        listed.items.iter().any(|item| item.title == "会被搬走的笔记"),
        "放回后笔记应当仍在库里"
    );
}

/// 最常见的「缺失」场景：单个笔记文件被移出工作区，随后又被放回。
///
/// 期望：扫描先把笔记标记为缺失（不可检索），文件回来后能**自动恢复**成正常笔记，
/// 并且重新进入检索索引——否则用户的直觉操作（把文件放回去）毫无效果。
#[test]
fn a_missing_note_recovers_when_its_file_comes_back() {
    use personal_knowledge_os_lib::application::search_service::SearchService;
    use personal_knowledge_os_lib::domain::search::SearchQuery;

    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "会被短暂移出的笔记".to_string(),
            category_id: None,
            initial_content: Some("独有术语：玄武岩索引".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("新建笔记")
        .note;

    let absolute = fixture
        .workspace
        .path()
        .join(note.file_path.replace('\\', "/"));
    let parked = fixture._data.path().join("parked.md");
    std::fs::rename(&absolute, &parked).expect("把笔记文件移出工作区");

    let report = IndexService::new(fixture.context())
        .scan_workspace()
        .expect("扫描");
    assert!(report.missing >= 1, "文件不在时应当标记缺失：{report:?}");
    let missing = fixture.notes().get(&note.id).expect("读回笔记");
    assert_eq!(
        missing.note.status,
        personal_knowledge_os_lib::domain::note::NoteStatus::Missing
    );

    // 放回文件 → 再扫一次
    std::fs::rename(&parked, &absolute).expect("把文件放回去");
    let report = IndexService::new(fixture.context())
        .scan_workspace()
        .expect("扫描");
    assert!(report.updated >= 1, "文件回来应当被识别为恢复：{report:?}");

    let restored = fixture.notes().get(&note.id).expect("读回笔记");
    assert_eq!(
        restored.note.status,
        personal_knowledge_os_lib::domain::note::NoteStatus::Active,
        "文件回来后必须恢复成正常笔记（原实现会永远停在「缺失」）"
    );
    assert!(
        restored.content.contains("玄武岩索引"),
        "正文要读回来：{}",
        restored.content
    );

    // 恢复后必须重新可检索（缺失时被移出了索引）
    let hits = SearchService::new(fixture.context())
        .search(SearchQuery {
            keyword: "玄武岩索引".to_string(),
            page: Some(1),
            page_size: Some(10),
            ..Default::default()
        })
        .expect("检索");
    assert!(
        hits.hits.iter().any(|hit| hit.note.id == note.id),
        "恢复后应当重新进入检索索引"
    );
}

/// 回收站恢复时的路径冲突：原路径已经被**别的文件**占用时，绝不能覆盖它。
///
/// 场景：删除一篇笔记（进回收站 `/.trash/原路径`）→ 用户或外部工具在原路径放了一个新文件 →
/// 恢复该笔记。期望：那个新文件**原封不动**，笔记被恢复到唯一的新路径并保持可用。
#[test]
fn restoring_from_trash_does_not_clobber_a_file_that_took_the_old_path() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "先删后恢复的笔记".to_string(),
            category_id: None,
            initial_content: Some("原始正文".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("新建笔记")
        .note;
    let original_path = note.file_path.clone();

    fixture.notes().delete(&note.id).expect("移入回收站");
    assert!(!fixture.workspace.path().join(&original_path).exists());

    // 在原路径写上「别人的文件」
    let occupied = fixture.workspace.path().join(&original_path);
    if let Some(parent) = occupied.parent() {
        std::fs::create_dir_all(parent).expect("建目录");
    }
    std::fs::write(&occupied, "这是别的文件，不能被覆盖").expect("占位文件");

    let restored = fixture.notes().restore(&note.id).expect("从回收站恢复");
    assert_ne!(
        restored.note.file_path, original_path,
        "原路径被占用时必须换一个唯一路径，而不是覆盖"
    );
    assert_eq!(
        std::fs::read_to_string(&occupied).expect("读占位文件"),
        "这是别的文件，不能被覆盖",
        "占位文件被覆盖了——这是静默数据丢失"
    );
    assert_eq!(
        restored.note.status,
        personal_knowledge_os_lib::domain::note::NoteStatus::Active
    );
    let restored_file = fixture
        .workspace
        .path()
        .join(restored.note.file_path.replace('\\', "/"));
    assert!(restored_file.exists(), "笔记文件要真的落在新路径上");
    assert!(
        std::fs::read_to_string(&restored_file)
            .expect("读恢复后的文件")
            .contains("原始正文"),
        "恢复的应当是笔记自己的内容"
    );
}
