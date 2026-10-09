//! 类型化关系（N2）的集成测试。

use std::sync::Arc;

use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::relation_service::{AddRelationInput, RelationService};
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
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
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        let root = workspace.path().to_path_buf();
        WorkspaceService::new(Arc::clone(&app))
            .create("关系测试", &root.to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app, root }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }

    fn relations(&self) -> RelationService {
        RelationService::new(self.app.context().expect("工作区已打开"))
    }

    fn note(&self, title: &str) -> String {
        self.notes()
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: None,
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记")
            .note
            .id
    }
}

#[test]
fn adds_lists_and_removes_typed_relations() {
    let fixture = Fixture::new();
    let v2 = fixture.note("索引设计 v2");
    let v1 = fixture.note("索引设计 v1");

    let view = fixture
        .relations()
        .add(AddRelationInput {
            source_id: v2.clone(),
            target_id: v1.clone(),
            relation_type: "supersedes".into(),
            note: Some("v2 取代 v1".into()),
        })
        .expect("添加关系");
    assert_eq!(view.outgoing.len(), 1);
    assert_eq!(view.outgoing[0].relation_type, "SUPERSEDES");
    assert_eq!(view.outgoing[0].target_title, "索引设计 v1");

    // 入边可见
    let target_view = fixture.relations().list(&v1).expect("目标视角");
    assert_eq!(target_view.incoming.len(), 1);
    assert_eq!(target_view.incoming[0].source_title, "索引设计 v2");

    // 写进了 frontmatter（文件是唯一事实来源）
    let detail = fixture.notes().get(&v2).expect("读取");
    let raw = std::fs::read_to_string(fixture.root.join(&detail.note.file_path)).expect("读文件");
    assert!(raw.contains("relations:"));
    assert!(raw.contains("type: SUPERSEDES"));
    assert!(raw.contains("target: 索引设计 v1"));

    // 重复添加被拒绝
    assert!(fixture
        .relations()
        .add(AddRelationInput {
            source_id: v2.clone(),
            target_id: v1.clone(),
            relation_type: "SUPERSEDES".into(),
            note: None,
        })
        .is_err());
    // 非法类型被拒绝
    assert!(fixture
        .relations()
        .add(AddRelationInput {
            source_id: v2.clone(),
            target_id: v1.clone(),
            relation_type: "NOT_A_TYPE".into(),
            note: None,
        })
        .is_err());

    let relation_id = view.outgoing[0].id.clone();
    let after = fixture.relations().remove(&relation_id).expect("删除关系");
    assert!(after.outgoing.is_empty());
    let raw = std::fs::read_to_string(fixture.root.join(&detail.note.file_path)).expect("读文件");
    assert!(!raw.contains("SUPERSEDES"), "frontmatter 里的声明也应被移除");
}

#[test]
fn rename_rewrites_source_frontmatter_and_delete_unbinds() {
    let fixture = Fixture::new();
    let source = fixture.note("来源笔记");
    let target = fixture.note("被取代的旧笔记");
    fixture
        .relations()
        .add(AddRelationInput {
            source_id: source.clone(),
            target_id: target.clone(),
            relation_type: "SUPERSEDES".into(),
            note: None,
        })
        .expect("添加关系");

    fixture.notes().rename(&target, "被取代的旧笔记 v2", false).expect("重命名目标");

    let view = fixture.relations().list(&source).expect("来源视角");
    assert_eq!(view.outgoing.len(), 1);
    assert_eq!(view.outgoing[0].target_title, "被取代的旧笔记 v2");
    let detail = fixture.notes().get(&source).expect("读取");
    let raw = std::fs::read_to_string(fixture.root.join(&detail.note.file_path)).expect("读文件");
    assert!(raw.contains("target: 被取代的旧笔记 v2"), "frontmatter 目标标题应被改写");

    // 目标删除后派生关系解绑（frontmatter 声明保留，重建可恢复）
    fixture.notes().delete(&target).expect("删除目标");
    let after = fixture.relations().list(&source).expect("来源视角");
    assert!(after.outgoing.is_empty());
}

#[test]
fn rebuild_restores_relations_from_frontmatter() {
    let fixture = Fixture::new();
    let a = fixture.note("关系甲");
    let b = fixture.note("关系乙");
    fixture
        .relations()
        .add(AddRelationInput {
            source_id: a.clone(),
            target_id: b.clone(),
            relation_type: "depends_on".into(),
            note: None,
        })
        .expect("添加关系");

    // 清空派生表后重建
    let context = fixture.app.context().unwrap();
    context
        .db
        .with_conn(|conn| {
            conn.execute("DELETE FROM note_relations", [])?;
            Ok(())
        })
        .expect("清空");
    let restored = fixture.relations().rebuild().expect("重建");
    assert_eq!(restored, 1);
    assert_eq!(fixture.relations().list(&a).expect("查询").outgoing.len(), 1);
}
