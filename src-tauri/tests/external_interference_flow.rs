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

/// 「父文档晚于子文档出现」：子文档先被外部拷进来（`parent:` 指向还不存在的父文档），
/// 父文档在后一轮扫描才出现。此时层级必须在**同一轮**里补上，而不是等子文档下次被编辑。
///
/// 回归背景：已知限制第 7 条——`parent_id` 解析失败后线索就丢了，
/// 因此新增派生列 `declared_parent` 保存声明原文，扫描末尾对「待解析」的子文档重试一次。
#[test]
fn child_adopted_before_parent_regains_its_level_in_the_next_round() {
    let fixture = Fixture::new();
    let root = fixture.workspace.path();
    let index = IndexService::new(fixture.context());

    // 1) 子文档先到：声明父文档「标题」（父文档此刻不存在）
    std::fs::write(
        root.join("子文档.md"),
        "---\ntitle: 子文档\nparent: 父文档\n---\n子文档正文\n",
    )
    .expect("写入子文档");
    let first = index.scan_workspace().expect("第一轮扫描");
    assert_eq!(first.added, 1);

    let child = fixture
        .notes()
        .list(NoteListQuery { keyword: Some("子文档".to_string()), ..Default::default() })
        .expect("列出笔记")
        .items
        .into_iter()
        .find(|item| item.title == "子文档")
        .expect("子文档已被采纳");
    assert_eq!(
        child.parent_id, None,
        "父文档还没出现时先按顶层展示（这条不变）"
    );

    // 2) 父文档后到：同一轮扫描内应当把子文档挂回去
    std::fs::write(root.join("父文档.md"), "---\ntitle: 父文档\n---\n父文档正文\n")
        .expect("写入父文档");
    let second = index.scan_workspace().expect("第二轮扫描");
    assert_eq!(second.added, 1);

    let parent = fixture
        .notes()
        .list(NoteListQuery { keyword: Some("父文档".to_string()), ..Default::default() })
        .expect("列出笔记")
        .items
        .into_iter()
        .find(|item| item.title == "父文档")
        .expect("父文档已被采纳");
    let reloaded = fixture.notes().get(&child.id).expect("重新读取子文档");
    assert_eq!(
        reloaded.note.parent_id.as_deref(),
        Some(parent.id.as_str()),
        "父文档在同一轮出现后，子文档层级要立刻补上（不必等它下次被编辑）"
    );
    assert_eq!(
        reloaded.note.declared_parent.as_deref(),
        Some("父文档"),
        "声明原文要留在库里（解析失败时它是唯一的重试线索）"
    );
}

/// 同一条不变量走**文件监听**入口（轮询 / 原生事件走的是 `apply_file_changes`）。
#[test]
fn child_adopted_before_parent_regains_its_level_via_file_watch() {
    use personal_knowledge_os_lib::infrastructure::watcher::{
        FileChange, FileChangeBatch, FileChangeKind,
    };

    let fixture = Fixture::new();
    let root = fixture.workspace.path();
    let index = IndexService::new(fixture.context());

    std::fs::write(
        root.join("子文档.md"),
        "---\ntitle: 子文档\nparent: 父文档\n---\n子文档正文\n",
    )
    .expect("写入子文档");
    index
        .apply_file_changes(FileChangeBatch {
            changes: vec![FileChange {
                path: "子文档.md".to_string(),
                kind: FileChangeKind::Created,
            }],
            directories: Vec::new(),
            attachments_changed: false,
        })
        .expect("监听：采纳子文档");

    std::fs::write(root.join("父文档.md"), "---\ntitle: 父文档\n---\n父文档正文\n")
        .expect("写入父文档");
    index
        .apply_file_changes(FileChangeBatch {
            changes: vec![FileChange {
                path: "父文档.md".to_string(),
                kind: FileChangeKind::Created,
            }],
            directories: Vec::new(),
            attachments_changed: false,
        })
        .expect("监听：采纳父文档");

    let items = fixture
        .notes()
        .list(NoteListQuery::default())
        .expect("列出笔记")
        .items;
    let child = items.iter().find(|item| item.title == "子文档").expect("子文档");
    let parent = items.iter().find(|item| item.title == "父文档").expect("父文档");
    assert_eq!(
        child.parent_id.as_deref(),
        Some(parent.id.as_str()),
        "监听路径也要在同一轮把层级补上"
    );
}

/// 非 UTF-8（GBK）的 Markdown 直接放进工作区时：扫描**明确跳过并告警**，
/// 不会静默消失，也不会把整篇解成乱码偷偷入库。
///
/// 想把它变成正常笔记，走「导入」——导入会按 GBK 解码并写成 UTF-8 的新笔记。
#[test]
fn a_non_utf8_markdown_file_is_skipped_with_a_visible_warning() {
    let fixture = Fixture::new();
    let root = fixture.workspace.path();
    // 「标题：中文内容」的 GBK 字节，前面加上 GBK 的 frontmatter 分隔行
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"---\nid: note_gbk_0001\ntitle: ");
    bytes.extend_from_slice(&[0xB1, 0xEA, 0xCC, 0xE2]); // 标题
    bytes.extend_from_slice(b"\n---\n");
    bytes.extend_from_slice(&[0xD6, 0xD0, 0xCE, 0xC4, 0xC4, 0xDA, 0xC8, 0xDD]); // 中文内容
    bytes.push(b'\n');
    std::fs::write(root.join("00_Inbox/gbk-笔记.md"), &bytes).expect("写入 GBK 笔记");

    let report = IndexService::new(fixture.context())
        .scan_workspace()
        .expect("扫描不应因为一个读不了的文件而失败");

    assert!(
        report
            .warnings
            .iter()
            .any(|line| line.contains("gbk-笔记.md")),
        "要如实告警是哪份文件被跳过：{:?}",
        report.warnings
    );
    let notes = fixture
        .notes()
        .list(NoteListQuery { page_size: Some(200), ..Default::default() })
        .expect("列表");
    assert!(
        !notes.items.iter().any(|item| item.title.contains("标题")),
        "读不了的 GBK 文件不该被静默当成乱码入库"
    );
}

/// 回归（2026-10-11 真机验收，P0）：**读不出文件时不能假装「正文为空」**。
///
/// 现场：切笔记时编辑区在 A/B 之间频闪，并伴随正文被写空（四篇笔记）。
/// 追到底是 `NoteService::detail()` 把一次读取失败 `Err(_) => None` 吞掉、
/// 于是返回空正文——前端把空正文同步进编辑器，下一次保存就把空内容写回了文件。
///
/// 期望：正常笔记的文件读不到时**如实报错**；只有扫描器已经标记为 `missing` 的笔记
/// 才允许按空正文返回（界面据此显示「笔记缺失」）。
#[test]
fn a_note_whose_file_disappeared_must_error_instead_of_returning_empty_content() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "文件被移走的笔记".to_string(),
            category_id: None,
            initial_content: Some("正文不能变成空的：玄武岩索引".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("新建笔记")
        .note;

    let absolute = fixture.workspace.path().join(note.file_path.replace('\\', "/"));
    let parked = fixture._data.path().join("parked-empty-body.md");
    std::fs::rename(&absolute, &parked).expect("把笔记文件移出工作区");

    // 还没扫描：状态仍是 active，此时读取必须失败，而不是回一个空正文
    let error = fixture
        .notes()
        .get(&note.id)
        .expect_err("文件读不到时应当报错，而不是返回空正文");
    assert!(
        !error.message.is_empty(),
        "错误信息要说清楚（界面上会显示它）：{error:?}"
    );

    // 扫描把状态标成 missing 之后：按空正文返回（界面显示「笔记缺失」），这是既有行为
    IndexService::new(fixture.context()).scan_workspace().expect("扫描");
    let missing = fixture.notes().get(&note.id).expect("缺失状态的笔记仍可读");
    assert_eq!(
        missing.note.status,
        personal_knowledge_os_lib::domain::note::NoteStatus::Missing
    );
    assert!(missing.content.is_empty(), "缺失笔记没有正文可读");
}

/// **B1 受控复现**：用 `cp` 把「版本历史快照」写回笔记文件后，编辑器下一次落盘为什么会显示「保存失败」。
///
/// 背景（[PLAN_V1.3.0.md](../../docs/PLAN_V1.3.0.md) §4 第 2 条）：V1.2.1 真机验收那晚出现过
/// 头部红字「保存失败」，当时的解释是"我正在用 `cp` 把历史快照写回文件"。那只是**推测**——
/// 这一条把它做成**可复跑的实验**，并顺带钉住三件事：
///
/// 1. 冲突是**按内容**判的（不是按 mtime）：`cp` 写回**同一份内容**（内容相同、时间变了）不该误报；
/// 2. 冲突错误必须把界面需要的东西带上（`currentHash` / `expectedHash` / 当前正文），
///    否则用户只看到「保存失败」却不知道该怎么办；
/// 3. 「重新加载后再存」这条恢复路径真的走得通——不然用户只能选覆盖，等于丢掉中间那次改动。
#[test]
fn cp_restoring_an_old_snapshot_makes_the_next_editor_save_conflict() {
    use personal_knowledge_os_lib::core::error::ErrorCode;

    let fixture = Fixture::new();
    // 关掉文件监听：这一条要的是"编辑器手里的 base 与磁盘内容不一致"这个**确定性**前提，
    // 不让后台采纳来插一脚（真实场景里监听在跑，但冲突判定读的是磁盘，与监听无关）。
    if let Ok(mut guard) = fixture.context().watcher.lock() {
        *guard = None;
    }

    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "会被 cp 回写的笔记".to_string(),
            category_id: None,
            initial_content: Some("第一版正文：玄武岩索引".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("新建笔记")
        .note;
    let absolute = fixture.workspace.path().join(note.file_path.replace('\\', "/"));
    let disk_hash = |fixture: &Fixture| -> String {
        fixture
            .context()
            .storage
            .read_note(&note.file_path)
            .expect("读磁盘上的笔记")
            .content_hash
    };

    // 第一版落盘后的原始字节 = 用户后来会 `cp` 回去的那份「历史快照」
    let snapshot_raw = fixture
        .context()
        .storage
        .read_raw(&note.file_path)
        .expect("读第一版原文");
    let v1_hash = disk_hash(&fixture);

    // 编辑器手里就是第一版；继续编辑并保存第二版（正常路径）
    fixture
        .notes()
        .save(SaveNoteInput {
            id: note.id.clone(),
            content: "第二版正文：花岗岩索引".to_string(),
            base_content_hash: Some(v1_hash.clone()),
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("第二版保存应当成功");
    let v2_hash = disk_hash(&fixture);
    assert_ne!(v1_hash, v2_hash, "两版内容不同，指纹必须不同");

    // —— 受控复现的关键一步：模拟 `cp 历史快照.md 笔记.md`（绕过服务、直接动文件）——
    std::fs::write(&absolute, &snapshot_raw).expect("用 cp 的方式写回旧快照");
    assert_eq!(
        disk_hash(&fixture),
        v1_hash,
        "把快照**原样**写回，指纹必须回到当时那一版——说明指纹是**文件内容**级的，与 mtime 无关"
    );

    // 编辑器此时手里还是第二版的 base：落盘必须被冲突保护拦下
    let conflict = fixture
        .notes()
        .save(SaveNoteInput {
            id: note.id.clone(),
            content: "第三版正文：页岩索引".to_string(),
            base_content_hash: Some(v2_hash.clone()),
            title: None,
            category_id: None,
            tags: None,
        })
        .expect_err("磁盘内容已被换掉，保存必须报冲突而不是静默覆盖");
    assert_eq!(
        conflict.code,
        ErrorCode::NoteConflict,
        "必须是 NOTE_CONFLICT（前端按这个码分支提示）：{conflict:?}"
    );
    let details = conflict.details.clone().expect("冲突要带 details，界面才提示得清楚");
    assert_eq!(details["noteId"], serde_json::json!(note.id));
    assert_eq!(details["filePath"], serde_json::json!(note.file_path));
    assert_eq!(
        details["expectedHash"],
        serde_json::json!(v2_hash),
        "expectedHash 要回显编辑器手里的 base，便于对账"
    );
    assert_eq!(
        details["currentHash"],
        serde_json::json!(v1_hash),
        "currentHash 必须是磁盘上现在那一份（= 被 cp 回去的快照）"
    );
    assert!(
        details["currentContent"]
            .as_str()
            .unwrap_or_default()
            .contains("玄武岩索引"),
        "details 里要带上磁盘上现在到底是什么，用户才能判断：{details:?}"
    );
    // 磁盘必须还是那份快照——冲突时**绝不能**已经写进去
    let on_disk = std::fs::read_to_string(&absolute).expect("读磁盘");
    assert!(
        on_disk.contains("玄武岩索引") && !on_disk.contains("页岩索引"),
        "冲突被拦下时磁盘不应被改动"
    );

    // —— 恢复路径 1：界面上的「重新加载」之后再存，必须能存进去 ——
    fixture.notes().get(&note.id).expect("重新加载");
    let fresh_base = disk_hash(&fixture);
    assert_eq!(fresh_base, v1_hash, "重新加载读到的就是磁盘上那份快照");
    let saved = fixture
        .notes()
        .save(SaveNoteInput {
            id: note.id.clone(),
            content: "第三版正文：页岩索引".to_string(),
            base_content_hash: Some(fresh_base),
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("重新加载后的保存必须成功，否则用户没法继续干活");
    assert!(
        std::fs::read_to_string(&absolute)
            .expect("读磁盘")
            .contains("页岩索引"),
        "恢复路径的落盘结果：{}",
        saved.content_hash
    );

    // —— 恢复路径 2：界面上的「覆盖保存」（不带 base）仍然可用 ——
    fixture
        .notes()
        .save(SaveNoteInput {
            id: note.id.clone(),
            content: "第四版正文：大理岩索引".to_string(),
            base_content_hash: None,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("不带 base 的覆盖保存必须仍然可用");
    assert!(std::fs::read_to_string(&absolute)
        .expect("读磁盘")
        .contains("大理岩索引"));

    // —— 反向验证：`cp` 写回**同一份内容**（内容没变、时间变了）不该误报冲突 ——
    let current_raw = fixture
        .context()
        .storage
        .read_raw(&note.file_path)
        .expect("读当前原文");
    std::fs::write(&absolute, &current_raw).expect("原样再写一遍（模拟 cp 同一份内容）");
    let same_base = disk_hash(&fixture);
    fixture
        .notes()
        .save(SaveNoteInput {
            id: note.id.clone(),
            content: "第五版正文：砂岩索引".to_string(),
            base_content_hash: Some(same_base),
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("内容没变就不该判冲突——冲突是按内容判的，不是按 mtime");
}
