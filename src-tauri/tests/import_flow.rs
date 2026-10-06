//! 外部文件导入的集成测试。

use std::path::PathBuf;
use std::sync::Arc;

use personal_knowledge_os_lib::application::category_service::CategoryService;
use personal_knowledge_os_lib::application::context::WorkspaceContext;
use personal_knowledge_os_lib::application::import_service::{
    ImportNotesInput, ImportService,
};
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    _source: TempDir,
    app: Arc<AppInner>,
    root: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let source = tempfile::tempdir().expect("导入源目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        let root = workspace.path().to_path_buf();
        let source_dir = source.path().to_path_buf();
        WorkspaceService::new(Arc::clone(&app))
            .create("导入测试", &root.to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, _source: source, app, root, source: source_dir }
    }

    fn context(&self) -> Arc<WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn imports(&self) -> ImportService {
        ImportService::new(self.context())
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.context())
    }

    fn write_source(&self, name: &str, content: &str) -> String {
        let path = self.source.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("创建源目录");
        }
        std::fs::write(&path, content).expect("写入源文件");
        path.to_string_lossy().to_string()
    }
}

#[test]
fn preserves_directory_structure_and_file_names_when_requested() {
    let fixture = Fixture::new();
    let root = fixture.write_source("项目A/设计/架构.md", "# 架构\n\n保留结构");
    let _ = root;
    fixture.write_source("项目A/设计/接口.md", "# 接口");
    fixture.write_source("项目A/README.md", "# 说明");
    let source_root = fixture.source.to_string_lossy().to_string();

    // 平铺导入（默认）：文件直接落在目标目录，文件名带时间戳
    let flat = fixture
        .imports()
        .import(ImportNotesInput {
            paths: vec![source_root.clone()],
            category_id: None,
            preserve_structure: None,
        })
        .expect("平铺导入");
    assert_eq!(flat.imported.len(), 3);
    assert!(flat
        .imported
        .iter()
        .all(|item| !item.file_path.contains("项目A")), "默认不保留目录结构：{}", flat.imported[0].file_path);

    // 保留结构导入：重建子目录并保留原文件名
    let structured = fixture
        .imports()
        .import(ImportNotesInput {
            paths: vec![source_root],
            category_id: None,
            preserve_structure: Some(true),
        })
        .expect("结构化导入");
    assert_eq!(structured.imported.len(), 3);
    let paths: Vec<&str> = structured
        .imported
        .iter()
        .map(|item| item.file_path.as_str())
        .collect();
    assert!(paths.iter().any(|path| path.ends_with("00_Inbox/项目A/设计/架构.md")), "{paths:?}");
    assert!(paths.iter().any(|path| path.ends_with("00_Inbox/项目A/设计/接口.md")), "{paths:?}");
    assert!(paths.iter().any(|path| path.ends_with("00_Inbox/项目A/README.md")), "{paths:?}");

    // 结构保留后，文件真的在磁盘上，并且能被扫描为笔记
    let workspace_root = fixture.root.clone();
    assert!(workspace_root.join("00_Inbox/项目A/设计/架构.md").is_file());
    let listed = fixture.notes().list(Default::default()).expect("列表");
    assert!(listed
        .items
        .iter()
        .any(|item| item.file_path == "00_Inbox/项目A/设计/架构.md"));
}

#[test]
fn imports_markdown_with_frontmatter_preserving_metadata() {
    let fixture = Fixture::new();
    let source = fixture.write_source(
        "external-note.md",
        "---\nid: note_ext_001\ntitle: 外部导入的笔记\ntags:\n  - 导入\n  - sqlite\ncreated_at: 2026-09-01T10:00:00+08:00\nupdated_at: 2026-09-02T11:30:00+08:00\ncustom_field: keep-me\n---\n\n正文包含 中文检索 关键词。\n",
    );

    let report = fixture
        .imports()
        .import(ImportNotesInput { paths: vec![source.clone()], category_id: None , preserve_structure: None })
        .expect("导入应成功");

    assert_eq!(report.imported.len(), 1, "跳过项：{:?}", report.skipped);
    assert_eq!(report.skipped.len(), 0);
    let imported = &report.imported[0];
    assert_eq!(imported.note_id, "note_ext_001", "应保留源 frontmatter 中的 id");
    assert_eq!(imported.title, "外部导入的笔记");
    assert!(imported.file_path.starts_with("00_Inbox/"), "{}", imported.file_path);

    let detail = fixture.notes().get("note_ext_001").expect("读取导入的笔记");
    assert!(detail.content.contains("中文检索"));
    assert_eq!(detail.tags.len(), 2);
    assert_eq!(detail.note.created_at, "2026-09-01T10:00:00+08:00");
    assert_eq!(detail.note.updated_at, "2026-09-02T11:30:00+08:00");

    // frontmatter 的未知字段保留在文件中
    let raw = std::fs::read_to_string(fixture.root.join(&imported.file_path)).expect("读取文件");
    assert!(raw.contains("custom_field: keep-me"), "{raw}");

    // 导入后立即可搜索
    let response = SearchService::new(fixture.context())
        .search(SearchQuery { keyword: "中文检索".into(), ..Default::default() })
        .expect("搜索");
    assert_eq!(response.hits.len(), 1);

    // 源文件保持不变（导入是复制）
    let source_after = std::fs::read_to_string(&source).expect("源文件仍存在");
    assert!(source_after.contains("id: note_ext_001"));
}

#[test]
fn imports_plain_text_into_selected_category() {
    let fixture = Fixture::new();
    let category = CategoryService::new(fixture.context())
        .create(CreateCategoryInput {
            name: "导入区".into(),
            parent_id: None,
            create_directory: Some(true),
        })
        .expect("创建分类");
    let source = fixture.write_source("20260901-100000-纯文本文档.txt", "只有正文，没有 frontmatter。\n第二行。");

    let report = fixture
        .imports()
        .import(ImportNotesInput {
            paths: vec![source],
            preserve_structure: None,
            category_id: Some(category.id.clone()),
        })
        .expect("导入应成功");

    assert_eq!(report.imported.len(), 1, "跳过项：{:?}", report.skipped);
    let imported = &report.imported[0];
    // 标题去掉日期前缀，文件落在目标分类目录
    assert_eq!(imported.title, "纯文本文档");
    assert!(imported.file_path.contains("导入区"), "{}", imported.file_path);

    let detail = fixture.notes().get(&imported.note_id).expect("读取笔记");
    assert_eq!(detail.category_path, "导入区");
    assert!(detail.content.contains("只有正文"));
}

#[test]
fn imports_directory_recursively_and_ignores_unsupported_files() {
    let fixture = Fixture::new();
    fixture.write_source("a.md", "---\nid: note_dir_a\ntitle: A\n---\n\nA 正文\n");
    fixture.write_source("sub/b.markdown", "---\nid: note_dir_b\ntitle: B\n---\n\nB 正文\n");
    fixture.write_source("sub/ignored.pdf", "not markdown");
    fixture.write_source(".hidden.md", "---\nid: note_hidden\n---\n\n隐藏文件\n");
    fixture.write_source("sub/deep/c.txt", "C 文本内容");

    let report = fixture
        .imports()
        .import(ImportNotesInput {
            paths: vec![fixture.source.to_string_lossy().to_string()],
            preserve_structure: None,
            category_id: None,
        })
        .expect("导入目录");

    assert_eq!(report.imported.len(), 3, "跳过项：{:?}", report.skipped);
    let ids: Vec<String> = report.imported.iter().map(|item| item.note_id.clone()).collect();
    assert!(ids.contains(&"note_dir_a".to_string()));
    assert!(ids.contains(&"note_dir_b".to_string()));
    // 不支持的类型与隐藏文件不会进入导入范围
    assert!(!ids.contains(&"note_hidden".to_string()));
    assert_eq!(report.skipped.len(), 0);
}

#[test]
fn duplicate_ids_and_repeat_imports_do_not_collide() {
    let fixture = Fixture::new();
    let first = fixture.write_source("one.md", "---\nid: note_dup\ntitle: 第一篇\n---\n\n一\n");
    let second = fixture.write_source("two.md", "---\nid: note_dup\ntitle: 第二篇\n---\n\n二\n");

    let report = fixture
        .imports()
        .import(ImportNotesInput { paths: vec![first, second], category_id: None, preserve_structure: None })
        .expect("同批导入");
    assert_eq!(report.imported.len(), 2);
    assert_ne!(
        report.imported[0].note_id, report.imported[1].note_id,
        "同批内重复 id 应自动分配新 id"
    );
    assert_eq!(report.imported[0].note_id, "note_dup");

    // 再次导入同一文件：源 id 已被占用，应生成新 id 而不是报错
    let again = fixture
        .imports()
        .import(ImportNotesInput {
            paths: vec![fixture.source.join("one.md").to_string_lossy().to_string()],
            preserve_structure: None,
            category_id: None,
        })
        .expect("重复导入");
    assert_eq!(again.imported.len(), 1);
    assert_ne!(again.imported[0].note_id, "note_dup");
}

#[test]
fn reports_missing_and_oversized_files_without_failing_the_batch() {
    let fixture = Fixture::new();
    let ok = fixture.write_source("ok.md", "---\nid: note_ok\ntitle: 正常\n---\n\n正文\n");
    let missing = fixture.source.join("not-exists.md").to_string_lossy().to_string();
    let big = fixture.source.join("big.md");
    std::fs::write(&big, "x".repeat(6 * 1024 * 1024)).expect("写入大文件");

    let report = fixture
        .imports()
        .import(ImportNotesInput {
            paths: vec![ok, missing, big.to_string_lossy().to_string()],
            preserve_structure: None,
            category_id: None,
        })
        .expect("整批不应失败");

    assert_eq!(report.imported.len(), 1);
    assert_eq!(report.skipped.len(), 2);
    assert!(report.skipped.iter().any(|item| item.reason.contains("不存在")));
    assert!(report.skipped.iter().any(|item| item.reason.contains("上限")));
    assert_eq!(report.failed, 2);
}

#[test]
fn rejects_empty_selection_and_unknown_category() {
    let fixture = Fixture::new();
    let error = fixture
        .imports()
        .import(ImportNotesInput { paths: Vec::new(), category_id: None , preserve_structure: None })
        .expect_err("空选择应报错");
    assert_eq!(error.code, personal_knowledge_os_lib::core::error::ErrorCode::InvalidInput);

    let source = fixture.write_source("a.md", "---\nid: note_x\n---\n\n正文\n");
    let error = fixture
        .imports()
        .import(ImportNotesInput { paths: vec![source], category_id: Some("cat_missing".into()) , preserve_structure: None })
        .expect_err("分类不存在应报错");
    assert_eq!(
        error.code,
        personal_knowledge_os_lib::core::error::ErrorCode::CategoryNotFound
    );
}
