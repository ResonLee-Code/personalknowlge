//! 元数据保全（0 信任 / 数据安全）。
//!
//! 任何一次「保存」都可能悄悄丢掉 frontmatter 里的字段——而 frontmatter 就是用户的元数据。
//! 这里用一篇「字段拉满」的笔记逐条盯住：
//! 来源与时效（N1）、父子层级、手动顺序、置顶、类型化关系（N2）、用户自定义字段（`extra`）、
//! 标签与分类路径、创建时间。
//!
//! 覆盖三条会写回文件的路径：`note_save`、`note_set_source`、以及 AI 的
//! `research_write`（追加模式，它内部就是 `note_save`）。

use std::sync::Arc;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::research::ResearchService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::{
    CreateNoteInput, SaveNoteInput, SetSourceInput,
};
use personal_knowledge_os_lib::domain::research::{
    ResearchRequest, ResearchWriteInput, WRITE_MODE_APPEND,
};
use personal_knowledge_os_lib::infrastructure::frontmatter;
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
            .create("元数据保全测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.context())
    }

    fn read_frontmatter(&self, relative: &str) -> frontmatter::ParsedDocument {
        let path = self.workspace.path().join(relative.replace('\\', "/"));
        let raw = std::fs::read_to_string(&path).expect("读笔记文件");
        frontmatter::parse(&raw)
    }
}

/// 造一篇字段拉满的笔记：来源 + 时效 + 层级 + 顺序 + 置顶 + 关系 + 自定义字段 + 标签。
fn seeded_note(fixture: &Fixture) -> (String, String) {
    let notes = fixture.notes();
    let parent = notes
        .create(CreateNoteInput {
            title: "元数据父文档".to_string(),
            category_id: None,
            initial_content: Some("父文档".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("父文档")
        .note;
    let child = notes
        .create(CreateNoteInput {
            title: "元数据子文档".to_string(),
            category_id: None,
            initial_content: Some("正文第一版".to_string()),
            tags: Some(vec!["保全".to_string()]),
            parent_id: None,
        })
        .expect("子文档")
        .note;
    notes
        .set_parent(personal_knowledge_os_lib::domain::note::SetParentInput {
            id: child.id.clone(),
            parent_id: Some(parent.id.clone()),
        })
        .expect("设置父文档");
    notes
        .set_source(SetSourceInput {
            id: child.id.clone(),
            source_type: Some("official".to_string()),
            source_ref: Some("https://example.com/doc".to_string()),
            source_reliability: Some(5),
            valid_until: Some("2027-03-31".to_string()),
        })
        .expect("来源与时效");
    notes.set_pinned(&child.id, true).expect("置顶");
    notes.set_favorite(&child.id, true).expect("收藏");
    notes
        .reorder(personal_knowledge_os_lib::domain::note::ReorderNotesInput {
            ids: vec![child.id.clone()],
        })
        .expect("手动顺序");

    // 手写一段「用户自己的字段」与一条类型化关系（模拟外部编辑器加的东西）
    let path = fixture
        .workspace
        .path()
        .join(child.file_path.replace('\\', "/"));
    let raw = std::fs::read_to_string(&path).expect("读文件");
    let updated = raw.replace(
        "---\n\n",
        "custom_field: keep-me\nrelations:\n  - type: RELATED_TO\n    target: 元数据父文档\n---\n\n",
    );
    std::fs::write(&path, updated).expect("写回文件");

    (child.id, child.file_path)
}

fn assert_fields_survive(fixture: &Fixture, note_id: &str, file_path: &str, why: &str) {
    let parsed = fixture.read_frontmatter(file_path);
    assert!(parsed.parse_error.is_none(), "{why}：frontmatter 必须可解析");
    let front = &parsed.frontmatter;
    assert_eq!(front.id.as_deref(), Some(note_id), "{why}：id 要保持");
    assert_eq!(
        front.source_type.as_deref(),
        Some("official"),
        "{why}：来源类型要保留"
    );
    assert_eq!(
        front.source_ref.as_deref(),
        Some("https://example.com/doc"),
        "{why}：来源引用要保留"
    );
    assert_eq!(front.source_reliability, Some(5), "{why}：可靠性要保留");
    assert_eq!(
        front.valid_until.as_deref(),
        Some("2027-03-31"),
        "{why}：时效要保留"
    );
    assert!(front.parent.is_some(), "{why}：父子层级要保留");
    assert!(front.pinned, "{why}：置顶要保留");
    assert!(front.favorite, "{why}：收藏要保留（与置顶同口径，写在 frontmatter 里）");
    assert!(front.order.is_some(), "{why}：手动顺序要保留");
    assert_eq!(front.tags, vec!["保全".to_string()], "{why}：标签要保留");
    assert!(
        front.relations.iter().any(|relation| relation.target == "元数据父文档"),
        "{why}：类型化关系要保留：{:?}",
        front.relations
    );
    assert!(
        front
            .extra
            .iter()
            .any(|(key, _)| key == "custom_field"),
        "{why}：用户自定义字段要保留：{:?}",
        front.extra.iter().map(|(key, _)| key.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn saving_content_does_not_drop_any_frontmatter_field() {
    let fixture = Fixture::new();
    let (note_id, file_path) = seeded_note(&fixture);
    let before = fixture.read_frontmatter(&file_path);

    fixture
        .notes()
        .save(SaveNoteInput {
            id: note_id.clone(),
            content: "正文第二版（只改了正文）".to_string(),
            base_content_hash: None,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("保存正文");

    assert_fields_survive(&fixture, &note_id, &file_path, "保存正文后");
    let after = fixture.read_frontmatter(&file_path);
    assert!(after.body.contains("正文第二版"), "正文要更新");
    assert_eq!(
        after.frontmatter.created_at, before.frontmatter.created_at,
        "创建时间不能被保存改写"
    );
    // 时间是秒级精度，同一秒内保存会相等；这里只保证「不倒退」
    assert!(
        after.frontmatter.updated_at.as_deref().unwrap_or_default()
            >= before.frontmatter.updated_at.as_deref().unwrap_or_default(),
        "更新时间不能倒退：{:?} → {:?}",
        before.frontmatter.updated_at,
        after.frontmatter.updated_at
    );
}

#[test]
fn writing_source_metadata_does_not_clobber_other_fields() {
    let fixture = Fixture::new();
    let (note_id, file_path) = seeded_note(&fixture);

    fixture
        .notes()
        .set_source(SetSourceInput {
            id: note_id.clone(),
            source_type: Some("book".to_string()),
            source_ref: Some("《某书》第 3 章".to_string()),
            source_reliability: Some(3),
            valid_until: Some("2028-01-01".to_string()),
        })
        .expect("更新来源");

    let parsed = fixture.read_frontmatter(&file_path);
    let front = &parsed.frontmatter;
    assert_eq!(front.source_type.as_deref(), Some("book"), "来源类型应被更新");
    assert_eq!(front.source_reliability, Some(3));
    assert_eq!(front.valid_until.as_deref(), Some("2028-01-01"));
    // 其余字段不能被这次写入顺手抹掉
    assert!(front.parent.is_some(), "父子层级要保留");
    assert!(front.pinned, "置顶要保留");
    assert!(front.order.is_some(), "手动顺序要保留");
    assert_eq!(front.tags, vec!["保全".to_string()], "标签要保留");
    assert!(
        front.relations.iter().any(|relation| relation.target == "元数据父文档"),
        "关系要保留"
    );
    assert!(
        front.extra.iter().any(|(key, _)| key == "custom_field"),
        "自定义字段要保留"
    );
}

#[test]
fn research_append_adds_the_report_and_keeps_every_existing_field() {
    let fixture = Fixture::new();
    let (note_id, file_path) = seeded_note(&fixture);
    // 让「同标题追加」有目标：直接用子文档标题当调研报告的标题（追加模式）
    let policy = AiPolicyService::new(fixture.context());
    policy
        .update(UpdateAiPolicyInput { enabled: Some(true), ..Default::default() })
        .expect("打开 AI");

    let research = ResearchService::new(fixture.context());
    let session = research
        .start(ResearchRequest {
            topic: "元数据子文档".to_string(),
            ..Default::default()
        })
        .expect("立项");
    let session = research.collect(&session.id, false).expect("收集");
    let session = research.verify(&session.id, false).expect("验证");
    let session = research.compose(&session.id, false).expect("汇总");
    let session = research.proposal(&session.id).expect("提案");

    research
        .write(ResearchWriteInput {
            session_id: session.id.clone(),
            mode: WRITE_MODE_APPEND.to_string(),
            target_note_id: Some(note_id.clone()),
            ..Default::default()
        })
        .expect("追加写入");

    assert_fields_survive(&fixture, &note_id, &file_path, "AI 追加写入后");
    let parsed = fixture.read_frontmatter(&file_path);
    assert!(parsed.body.contains("正文第一版"), "原有正文要保留");
    assert!(parsed.body.contains("资料截至"), "报告正文要追加进去");
}

/// 收藏 = frontmatter 里的 `favorite: true`（与置顶同口径）：
/// 开关要真的写进文件，外部手改也要生效——否则「删库重建」会静默丢收藏。
#[test]
fn favorite_round_trips_through_the_file() {
    let fixture = Fixture::new();
    let note = fixture
        .notes()
        .create(CreateNoteInput {
            title: "收藏开关测试".to_string(),
            category_id: None,
            initial_content: Some("正文".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("建笔记")
        .note;

    // 打开收藏：文件里要有 favorite: true
    fixture
        .notes()
        .set_favorite(&note.id, true)
        .expect("收藏");
    let parsed = fixture.read_frontmatter(&note.file_path);
    assert!(parsed.frontmatter.favorite, "收藏要写进 frontmatter");
    assert!(
        std::fs::read_to_string(fixture.workspace.path().join(&note.file_path))
            .expect("读原文")
            .contains("favorite: true"),
        "文件里要能直接看到 favorite: true"
    );

    // 取消收藏：字段要被移除（不留 `favorite: false` 的噪声）
    fixture
        .notes()
        .set_favorite(&note.id, false)
        .expect("取消收藏");
    let raw = std::fs::read_to_string(fixture.workspace.path().join(&note.file_path)).expect("读原文");
    assert!(
        !raw.contains("favorite"),
        "取消收藏后不应再写这个字段：\n{raw}"
    );
    assert!(
        !fixture.notes().get(&note.id).expect("读回").note.favorite,
        "库里也要跟着变成未收藏"
    );

    // 外部手改（用户自己写 favorite: true）→ 扫描后生效
    let raw = raw.replace("status: active", "status: active\nfavorite: true");
    std::fs::write(fixture.workspace.path().join(&note.file_path), raw).expect("外部改文件");
    personal_knowledge_os_lib::application::index_service::IndexService::new(fixture.context())
        .scan_workspace()
        .expect("扫描");
    assert!(
        fixture.notes().get(&note.id).expect("读回").note.favorite,
        "外部手写的 favorite: true 要在扫描后生效"
    );
}
