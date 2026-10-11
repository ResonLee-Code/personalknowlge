//! 笔记导出（Markdown / HTML / Word / PDF 打印视图）的集成测试。

use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use personal_knowledge_os_lib::application::attachment_service::{
    AttachmentService, SaveAttachmentInput,
};
use personal_knowledge_os_lib::application::context::WorkspaceContext;
use personal_knowledge_os_lib::application::export_service::{
    ExportFormat, ExportNoteInput, ExportService,
};
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::base64;
use personal_knowledge_os_lib::core::error::ErrorCode;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SaveNoteInput};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

const MARKDOWN: &str = "# 全文搜索\n\nSQLite **FTS5** 支持中文双字短语检索。\n\n- 索引策略\n- 查询策略\n\n```rust\nlet a = 1;\n```\n\n| 方案 | 说明 |\n| --- | --- |\n| 双字 | 相邻两字 |\n";

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    _output: TempDir,
    app: Arc<AppInner>,
    output: PathBuf,
    note_id: String,
    file_path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let output = tempfile::tempdir().expect("导出目录");
        let app =
            AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink)).expect("应用状态");
        let root = workspace.path().to_path_buf();
        WorkspaceService::new(Arc::clone(&app))
            .create("导出测试", &root.to_string_lossy())
            .expect("创建工作区");

        let note = NoteService::new(app.context().expect("上下文"))
            .create(CreateNoteInput {
                title: "SQLite 全文搜索学习笔记".into(),
                category_id: None,
                initial_content: Some(MARKDOWN.to_string()),
                tags: Some(vec!["sqlite".into()]),
                parent_id: None,
            })
            .expect("创建笔记");

        let output_dir = output.path().to_path_buf();
        Self {
            _data: data,
            _workspace: workspace,
            _output: output,
            app,
            output: output_dir,
            note_id: note.note.id.clone(),
            file_path: root.join(&note.note.file_path),
        }
    }

    fn context(&self) -> Arc<WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn export(&self, format: ExportFormat, target: Option<PathBuf>) -> personal_knowledge_os_lib::application::export_service::ExportNoteResult {
        ExportService::new(self.context())
            .export_note(ExportNoteInput {
                id: self.note_id.clone(),
                format,
                target_path: target.map(|path| path.to_string_lossy().to_string()),
            })
            .expect("导出应成功")
    }
}

#[test]
fn exports_markdown_byte_for_byte() {
    let fixture = Fixture::new();
    let target = fixture.output.join("note.md");
    let result = fixture.export(ExportFormat::Markdown, Some(target.clone()));

    assert_eq!(result.format, ExportFormat::Markdown);
    assert!(target.exists());
    assert!(!result.print_ready);

    // Markdown 导出应与工作区里的文件完全一致（含 frontmatter）
    let exported = std::fs::read_to_string(&target).expect("读取导出文件");
    let original = std::fs::read_to_string(&fixture.file_path).expect("读取源文件");
    assert_eq!(exported, original);
    assert!(exported.contains("id: note_"));
    assert!(exported.contains("title: SQLite 全文搜索学习笔记"));
}

#[test]
fn exports_self_contained_html() {
    let fixture = Fixture::new();
    let target = fixture.output.join("note.html");
    fixture.export(ExportFormat::Html, Some(target.clone()));

    let html = std::fs::read_to_string(&target).expect("读取 HTML");
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<style>"), "样式应内联，便于直接打印");
    assert!(html.contains("<h1>SQLite 全文搜索学习笔记</h1>"), "应输出标题");
    assert!(html.contains("<h1>全文搜索</h1>") || html.contains("全文搜索"), "正文标题应渲染");
    assert!(html.contains("<strong>FTS5</strong>"), "粗体应转换");
    assert!(html.contains("<table>"), "表格应转换");
    assert!(html.contains("索引策略"));
    assert!(html.contains("let a = 1;"), "代码块应保留");
    assert!(!html.contains("---\nid:"), "正文中不应包含 frontmatter");
}

#[test]
fn exports_word_document_with_chinese_content() {
    let fixture = Fixture::new();
    let target = fixture.output.join("note.docx");
    let result = fixture.export(ExportFormat::Docx, Some(target.clone()));

    assert!(result.bytes > 1000, "docx 应有实际内容");
    let bytes = std::fs::read(&target).expect("读取 docx");
    assert!(bytes.starts_with(b"PK\x03\x04"), "docx 必须是 zip 包");

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("解析 docx");
    let mut xml = String::new();
    archive
        .by_name("word/document.xml")
        .expect("document.xml")
        .read_to_string(&mut xml)
        .expect("读取 document.xml");
    assert!(xml.contains("SQLite 全文搜索学习笔记"));
    assert!(xml.contains("全文搜索"));
    assert!(xml.contains("双字"));
    assert!(xml.contains(r#"w:val="Heading1""#));
}

#[test]
fn pdf_export_returns_print_view() {
    let fixture = Fixture::new();
    let target = fixture.output.join("note-print.html");
    let result = fixture.export(ExportFormat::Pdf, Some(target.clone()));

    assert!(result.print_ready, "PDF 走打印视图，需要前端打开");
    assert!(target.exists(), "打印视图文件应已写出");
    let html = result.html.expect("应返回 HTML 内容供前端打印");
    assert!(html.contains("全文搜索"));
    assert_eq!(result.path, target.to_string_lossy());
}

#[test]
fn export_without_target_writes_to_temp_directory() {
    let fixture = Fixture::new();
    let result = fixture.export(ExportFormat::Html, None);
    let path = PathBuf::from(&result.path);
    assert!(path.exists(), "缺省应写入临时目录：{}", result.path);
    assert!(result.path.contains("personal-knowledge-os-export"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn exporting_missing_note_fails_clearly() {
    let fixture = Fixture::new();
    let error = ExportService::new(fixture.context())
        .export_note(ExportNoteInput {
            id: "note_missing".into(),
            format: ExportFormat::Markdown,
            target_path: None,
        })
        .expect_err("不存在的笔记应报错");
    assert_eq!(error.code, ErrorCode::NoteNotFound);
}

/// 1×1 的透明 PNG（与 `attachment_flow` 用的是同一张）。
const SAMPLE_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
    0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
    0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
    0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
    0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// HTML / 打印视图宣称「自包含单文件」——工作区里的图片必须内联，
/// 否则导出文件（以及「打印 → 存储为 PDF」的结果）里图片全是破图。
#[test]
fn html_and_print_view_inline_workspace_images() {
    let fixture = Fixture::new();
    let saved = AttachmentService::new(fixture.context())
        .save(SaveAttachmentInput {
            note_id: Some(fixture.note_id.clone()),
            file_name: Some("dot.png".to_string()),
            mime_type: Some("image/png".to_string()),
            data_base64: base64::encode(SAMPLE_PNG),
        })
        .expect("保存附件");
    NoteService::new(fixture.context())
        .save(SaveNoteInput {
            id: fixture.note_id.clone(),
            content: format!("{MARKDOWN}\n\n![点]({})\n", saved.link_path),
            base_content_hash: None,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("把图片写进正文");

    let html_target = fixture.output.join("with-image.html");
    fixture.export(ExportFormat::Html, Some(html_target.clone()));
    let html = std::fs::read_to_string(&html_target).expect("读 HTML");
    assert!(
        html.contains("data:image/png;base64,"),
        "工作区内的图片要内联成 data URL：\n{html}"
    );
    assert!(
        !html.contains(&saved.link_path),
        "不应再留下打不开的相对路径：\n{html}"
    );

    // PDF 走打印视图：同样要内联，否则打印出来是破图
    let result = fixture.export(
        ExportFormat::Pdf,
        Some(fixture.output.join("with-image.print.html")),
    );
    assert!(result.print_ready);
    let print_html = result.html.expect("打印视图 HTML");
    assert!(
        print_html.contains("data:image/png;base64,"),
        "打印视图也要内联工作区图片"
    );
}
