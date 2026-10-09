//! 本地模板的集成测试（V1.1 M2-2 / F8）。

use std::path::PathBuf;
use std::sync::Arc;

use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::template_service::{
    RenderTemplateInput, SaveTemplateInput, TemplateService,
};
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        let root = workspace.path().to_path_buf();
        WorkspaceService::new(Arc::clone(&app))
            .create("模板测试", &root.to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app, root }
    }

    fn templates(&self) -> TemplateService {
        TemplateService::new(self.app.context().expect("工作区已打开"))
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }

    fn write_template(&self, file_name: &str, content: &str) {
        let dir = self.root.join(".knowledge").join("templates");
        std::fs::create_dir_all(&dir).expect("创建模板目录");
        std::fs::write(dir.join(file_name), content).expect("写入模板");
    }
}

#[test]
fn lists_only_markdown_templates_sorted_by_name() {
    let fixture = Fixture::new();
    // 工作区创建时会写入一个示例模板，保证「模板」能力可被发现
    let starter = fixture.templates().list().expect("列出模板");
    assert_eq!(starter.len(), 1);
    assert_eq!(starter[0].name, "会议记录");
    assert!(starter[0].bytes > 0);

    fixture.write_template("会议记录.md", "# {{title}}\n\n- 时间：{{datetime}}\n");
    fixture.write_template("读书笔记.MARKDOWN", "## 书名\n");
    fixture.write_template("readme.txt", "不是模板");

    let templates = fixture.templates().list().expect("列出模板");
    let names: Vec<&str> = templates.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, vec!["会议记录", "读书笔记"]);
    // 标题里只有占位符时回落到模板名，避免选择器显示 `{{title}}`
    assert_eq!(templates[0].title, "会议记录");

    let rendered = fixture
        .templates()
        .render(RenderTemplateInput {
            name: "会议记录".into(),
            title: Some("周会记录".into()),
        })
        .expect("渲染模板");
    assert!(rendered.content.starts_with("# 周会记录"));
    assert!(rendered.content.contains("- 时间：20"));
}

#[test]
fn rendered_content_can_be_created_as_a_note() {
    let fixture = Fixture::new();
    fixture.write_template("日报.md", "# {{title}}\n\n## 今天\n\n## 明天\n");

    let rendered = fixture
        .templates()
        .render(RenderTemplateInput { name: "日报.md".into(), title: Some("10 月 7 日".into()) })
        .expect("渲染模板");
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "10 月 7 日".into(),
            category_id: None,
            initial_content: Some(rendered.content.clone()),
            tags: None,
            parent_id: None,
        })
        .expect("按模板新建笔记");

    let detail = fixture.notes().get(&note.note.id).expect("读取笔记");
    assert!(detail.content.contains("## 明天"));
    assert!(detail.content.contains("# 10 月 7 日"));
}

#[test]
fn rejects_missing_and_unsafe_template_names() {
    let fixture = Fixture::new();
    fixture.write_template("会议记录.md", "内容");

    for name in ["不存在", "", "../config", "a/b"] {
        let result = fixture
            .templates()
            .render(RenderTemplateInput { name: name.into(), title: None });
        assert!(result.is_err(), "模板名 {name:?} 应被拒绝");
    }
}

#[test]
fn saves_note_body_as_template_and_reuses_it() {
    let fixture = Fixture::new();

    // 另存为模板：正文没有一级标题时，自动补 `# {{title}}`
    let saved = fixture
        .templates()
        .save(SaveTemplateInput {
            name: Some("周报".into()),
            content: "## 本周完成\n\n- \n\n## 下周计划\n\n- ".into(),
            overwrite: false,
        })
        .expect("另存为模板");
    assert_eq!(saved.name, "周报");
    assert_eq!(saved.file_name, "周报.md");
    assert_eq!(saved.title, "周报", "标题取首个 `#`，占位符标题会被跳过");
    let path = fixture.root.join(".knowledge").join("templates").join("周报.md");
    let raw = std::fs::read_to_string(&path).expect("读取模板文件");
    assert!(raw.starts_with("# {{title}}\n"), "应补上标题占位符：\n{raw}");
    assert!(raw.contains("## 本周完成"));

    // 模板可直接被「新建笔记 → 模板」使用（占位符替换）
    let rendered = fixture
        .templates()
        .render(RenderTemplateInput { name: "周报".into(), title: Some("第 42 周".into()) })
        .expect("套用模板");
    assert!(rendered.content.contains("# 第 42 周"));
    assert!(rendered.content.contains("## 本周完成"));

    // 同名模板默认拒绝覆盖，勾选覆盖后才写入
    let duplicate = fixture.templates().save(SaveTemplateInput {
        name: Some("周报".into()),
        content: "新内容".into(),
        overwrite: false,
    });
    assert!(duplicate.is_err(), "同名模板应要求显式覆盖");
    fixture
        .templates()
        .save(SaveTemplateInput {
            name: Some("周报".into()),
            content: "# {{title}}\n\n新内容".into(),
            overwrite: true,
        })
        .expect("覆盖写入");
    let updated = std::fs::read_to_string(&path).expect("读取模板文件");
    assert!(updated.contains("新内容"));
    assert!(!updated.contains("## 本周完成"), "覆盖后应只剩新内容");
}

#[test]
fn rejects_unsafe_template_names() {
    let fixture = Fixture::new();
    for bad in ["", "   ", "../逃逸", "子目录/模板"] {
        let result = fixture.templates().save(SaveTemplateInput {
            name: Some(bad.into()),
            content: "x".into(),
            overwrite: true,
        });
        assert!(result.is_err(), "不合法模板名应被拒绝：{bad}");
    }
}
