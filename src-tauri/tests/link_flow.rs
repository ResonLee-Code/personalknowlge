//! 双向链接（M1-2）的集成测试。

use std::sync::Arc;

use personal_knowledge_os_lib::application::category_service::CategoryService;
use personal_knowledge_os_lib::application::context::WorkspaceContext;
use personal_knowledge_os_lib::application::link_service::LinkService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::link::LinkType;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, NoteDetail, SaveNoteInput};
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
        let data = tempfile::tempdir().expect("数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app =
            AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink)).expect("应用状态");
        let root = workspace.path().to_path_buf();
        WorkspaceService::new(Arc::clone(&app))
            .create("链接测试", &root.to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app, root }
    }

    fn context(&self) -> Arc<WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.context())
    }

    fn links(&self) -> LinkService {
        LinkService::new(self.context())
    }

    fn category(&self, name: &str) -> String {
        CategoryService::new(self.context())
            .create(CreateCategoryInput {
                name: name.to_string(),
                parent_id: None,
                create_directory: Some(true),
            })
            .expect("创建分类")
            .id
    }

    fn create_note(&self, title: &str, content: &str) -> NoteDetail {
        self.notes()
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(content.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记")
    }

    fn body_of(&self, detail: &NoteDetail) -> String {
        self.context()
            .storage
            .read_note(&detail.note.file_path)
            .expect("读取文件")
            .body
    }
}

#[test]
fn resolves_links_and_reports_backlinks() {
    let fixture = Fixture::new();
    let target = fixture.create_note("设计文档", "# 设计文档\n\n索引策略。");
    let source = fixture.create_note(
        "参考笔记",
        "参考 [[设计文档]]，以及 [[还没有的笔记]]。",
    );

    let outgoing = fixture.links().outgoing(&source.note.id).expect("出链");
    assert_eq!(outgoing.len(), 2, "{outgoing:?}");
    assert_eq!(outgoing[0].target_title, "设计文档");
    assert!(outgoing[0].target_exists, "应解析到已有笔记");
    assert_eq!(outgoing[0].target_note_id.as_deref(), Some(target.note.id.as_str()));
    assert_eq!(outgoing[0].link_type, LinkType::Wikilink);
    assert!(outgoing[0].context.contains("[[设计文档]]"));

    assert_eq!(outgoing[1].target_title, "还没有的笔记");
    assert!(!outgoing[1].target_exists, "未解析链接应标记为不存在");

    let backlinks = fixture.links().backlinks(&target.note.id).expect("反链");
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].note_id, source.note.id);
    assert_eq!(backlinks[0].title, "参考笔记");
    assert!(backlinks[0].context.contains("设计文档"));

    let counts = fixture.links().counts(&source.note.id).expect("计数");
    assert_eq!(counts.backlinks, 0);
    assert_eq!(counts.outgoing, 2);
    assert_eq!(counts.unresolved, 1);

    let unresolved = fixture.links().unresolved().expect("未解析");
    assert_eq!(unresolved.len(), 1);
    assert_eq!(unresolved[0].target_title, "还没有的笔记");
    assert_eq!(unresolved[0].source_title, "参考笔记");
}

#[test]
fn creating_the_target_resolves_pending_links() {
    let fixture = Fixture::new();
    let source = fixture.create_note("来源", "待整理：[[新笔记]]");
    assert_eq!(fixture.links().unresolved().expect("未解析").len(), 1);

    let created = fixture.create_note("新笔记", "创建后应自动建立连接。");
    assert!(
        fixture.links().unresolved().expect("未解析").is_empty(),
        "创建目标后未解析链接应自动绑定"
    );

    let backlinks = fixture.links().backlinks(&created.note.id).expect("反链");
    assert_eq!(backlinks.len(), 1);
    assert_eq!(backlinks[0].note_id, source.note.id);
}

#[test]
fn renaming_reconnects_links_and_rewrites_source_text() {
    let fixture = Fixture::new();
    let target = fixture.create_note("旧标题", "正文");
    let source = fixture.create_note("来源", "见 [[旧标题]] 与 [[旧标题|别名]]。");

    fixture
        .notes()
        .rename(&target.note.id, "新标题", true)
        .expect("重命名");

    // 来源笔记正文里的链接目标被改写
    let body = fixture.body_of(&source);
    assert!(body.contains("[[新标题]]"), "{body}");
    assert!(body.contains("[[新标题|别名]]"), "{body}");
    assert!(!body.contains("[[旧标题"), "旧标题不应残留：{body}");

    // 重命名后仍保持连接，且重建索引后一致
    let backlinks = fixture.links().backlinks(&target.note.id).expect("反链");
    assert_eq!(backlinks.len(), 1);
    assert_eq!(fixture.links().unresolved().expect("未解析").len(), 0);

    let report = fixture.links().rebuild().expect("重建");
    assert!(report.duration_ms < 30_000);
    let backlinks_after = fixture.links().backlinks(&target.note.id).expect("重建后反链");
    assert_eq!(backlinks_after.len(), 1, "重建后链接关系应与正文一致");
}

#[test]
fn deleting_and_restoring_notes_rebinds_links() {
    let fixture = Fixture::new();
    let target = fixture.create_note("被引用的笔记", "正文");
    let source = fixture.create_note("来源", "引用 [[被引用的笔记]]");

    fixture.notes().delete(&target.note.id).expect("删除");
    let outgoing = fixture.links().outgoing(&source.note.id).expect("出链");
    assert_eq!(outgoing.len(), 1);
    assert!(!outgoing[0].target_exists, "删除后链接应失效");
    assert_eq!(fixture.links().unresolved().expect("未解析").len(), 1);

    fixture.notes().restore(&target.note.id).expect("恢复");
    let outgoing = fixture.links().outgoing(&source.note.id).expect("恢复后出链");
    assert!(outgoing[0].target_exists, "恢复后链接应重新生效");
    assert!(fixture.links().unresolved().expect("未解析").is_empty());
}

#[test]
fn rebuild_ignores_code_blocks_and_counts_links() {
    let fixture = Fixture::new();
    let target = fixture.create_note("目标笔记", "正文");
    fixture.create_note(
        "含代码的笔记",
        "真实链接 [[目标笔记]]\n\n```\n[[目标笔记]]\n```\n\n行内 `[[目标笔记]]` 结束。",
    );

    let report = fixture.links().rebuild().expect("重建");
    // 两篇笔记（欢迎笔记 + 上面的笔记）里只有 1 条真实链接
    assert_eq!(report.links, 1, "{report:?}");
    assert_eq!(report.resolved, 1);
    assert_eq!(report.unresolved, 0);

    let backlinks = fixture.links().backlinks(&target.note.id).expect("反链");
    assert_eq!(backlinks.len(), 1);
}

#[test]
fn resolves_relative_markdown_links() {
    let fixture = Fixture::new();
    // 注意：工作区初始化时已存在「技术」分类（01_技术），这里用独立名称
    let category_id = fixture.category("链接测试分类");
    let target = fixture
        .notes()
        .create(CreateNoteInput {
            title: "设计稿".into(),
            category_id: Some(category_id.clone()),
            initial_content: Some("正文".into()),
            tags: None,
            parent_id: None,
        })
        .expect("创建目标");

    let source = fixture.create_note(
        "引用",
        &format!("见 [设计稿]({})", target.note.file_path),
    );

    let outgoing = fixture.links().outgoing(&source.note.id).expect("出链");
    assert_eq!(outgoing.len(), 1, "{outgoing:?}");
    assert_eq!(outgoing[0].link_type, LinkType::Markdown);
    assert_eq!(outgoing[0].target_note_id.as_deref(), Some(target.note.id.as_str()));

    // 外部链接也会记录，但不算未解析
    fixture
        .notes()
        .save(SaveNoteInput {
            id: source.note.id.clone(),
            content: "见 [官网](https://example.com) 与 [设计稿](./x/y.md)".into(),
            base_content_hash: None,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("保存");
    let outgoing = fixture.links().outgoing(&source.note.id).expect("出链");
    assert_eq!(outgoing.len(), 2);
    assert!(outgoing.iter().any(|link| link.link_type == LinkType::Url));
    let unresolved = fixture.links().unresolved().expect("未解析");
    assert!(
        unresolved.iter().all(|item| item.target_title != "https://example.com"),
        "外链不应进入未解析列表"
    );
}

#[test]
fn external_file_edits_refresh_links() {
    let fixture = Fixture::new();
    let target = fixture.create_note("目标", "正文");
    let source = fixture.create_note("来源", "初始内容");

    // 模拟外部编辑器加入双链
    let path = fixture.root.join(&source.note.file_path);
    let content = std::fs::read_to_string(&path).expect("读取");
    std::fs::write(&path, format!("{content}\n\n外部加入了 [[目标]]。\n")).expect("写入");

    let scan = personal_knowledge_os_lib::application::index_service::IndexService::new(
        fixture.context(),
    )
    .scan_workspace()
    .expect("扫描工作区");
    assert_eq!(scan.updated, 1, "{scan:?}");

    let backlinks = fixture.links().backlinks(&target.note.id).expect("反链");
    assert_eq!(backlinks.len(), 1, "外部新增的双链应被索引");
    assert_eq!(backlinks[0].note_id, source.note.id);
}
