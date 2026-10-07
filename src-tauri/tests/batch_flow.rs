//! 批量操作（V1.1.2）的集成测试。

use std::sync::Arc;

use personal_knowledge_os_lib::application::batch_service::{
    BatchDeleteInput, BatchMoveInput, BatchService, BatchTagInput,
};
use personal_knowledge_os_lib::application::category_service::CategoryService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, NoteListQuery};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("批量测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }

    fn batch(&self) -> BatchService {
        BatchService::new(self.app.context().expect("工作区已打开"))
    }

    fn note(&self, title: &str) -> String {
        self.notes()
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: None,
                tags: None,
            })
            .expect("创建笔记")
            .note
            .id
    }
}

#[test]
fn batch_move_tag_and_delete_notes() {
    let fixture = Fixture::new();
    let a = fixture.note("批量甲");
    let b = fixture.note("批量乙");
    let ids = vec![a.clone(), b.clone(), a.clone()]; // 含重复 id，应去重

    let category = CategoryService::new(fixture.app.context().unwrap())
        .create(CreateCategoryInput {
            name: "批量分类".into(),
            parent_id: None,
            create_directory: None,
        })
        .expect("建分类");

    let moved = fixture
        .batch()
        .move_notes(BatchMoveInput { ids: ids.clone(), category_id: Some(category.id.clone()) })
        .expect("批量移动");
    assert_eq!(moved.succeeded, 2, "重复 id 只处理一次");
    assert!(moved.failures.is_empty());
    assert_eq!(fixture.notes().get(&a).unwrap().note.category_id.as_deref(), Some(category.id.as_str()));

    // 打标签：add 合并、remove 移除
    let added = fixture
        .batch()
        .tag_notes(BatchTagInput {
            ids: ids.clone(),
            tags: vec!["批量".into(), "整理".into()],
            mode: None,
        })
        .expect("批量打标签");
    assert_eq!(added.succeeded, 2);
    let tags: Vec<String> = fixture
        .notes()
        .get(&a)
        .unwrap()
        .tags
        .into_iter()
        .map(|tag| tag.name)
        .collect();
    assert!(tags.contains(&"批量".to_string()));
    assert!(tags.contains(&"整理".to_string()));

    let removed = fixture
        .batch()
        .tag_notes(BatchTagInput { ids: ids.clone(), tags: vec!["整理".into()], mode: Some("remove".into()) })
        .expect("批量移除标签");
    assert_eq!(removed.succeeded, 2);
    let tags: Vec<String> = fixture
        .notes()
        .get(&a)
        .unwrap()
        .tags
        .into_iter()
        .map(|tag| tag.name)
        .collect();
    assert!(!tags.contains(&"整理".to_string()));
    assert!(tags.contains(&"批量".to_string()));

    // 删除：进入回收站
    let deleted = fixture
        .batch()
        .delete_notes(BatchDeleteInput { ids: ids.clone() })
        .expect("批量删除");
    assert_eq!(deleted.succeeded, 2);
    let active = fixture.notes().list(NoteListQuery::default()).expect("列表");
    assert!(active.items.iter().all(|item| item.id != a && item.id != b));
    assert_eq!(fixture.notes().list_deleted().expect("回收站").len(), 2);
}

#[test]
fn failures_are_reported_without_aborting_the_batch() {
    let fixture = Fixture::new();
    let ok = fixture.note("正常笔记");
    let missing = "note_does_not_exist".to_string();

    let report = fixture
        .batch()
        .move_notes(BatchMoveInput { ids: vec![ok.clone(), missing.clone()], category_id: None })
        .expect("批量移动");
    assert_eq!(report.succeeded, 1);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].note_id, missing);
    assert!(!report.failures[0].message.is_empty());
}
