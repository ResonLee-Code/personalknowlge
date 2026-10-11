//! 工作区注册表（0 信任：`workspace_forget` 这类命令名字听起来「很危险」，必须用测试钉死语义）。
//!
//! 约定：`forget` 只从「最近打开」列表里移除，**不动工作区里的任何数据**；
//! 之后仍然可以重新打开，笔记与数据库完好。

use std::sync::Arc;

use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, NoteListQuery};
use personal_knowledge_os_lib::state::AppInner;

#[test]
fn forgetting_a_workspace_only_touches_the_registry() {
    let data = tempfile::tempdir().expect("应用数据目录");
    let first = tempfile::tempdir().expect("工作区 A");
    let second = tempfile::tempdir().expect("工作区 B");
    let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink)).expect("应用状态");
    let service = WorkspaceService::new(Arc::clone(&app));

    let created = service
        .create("工作区 A", &first.path().to_string_lossy())
        .expect("创建 A");

    // 此刻活动工作区是 A：先在里面留一篇笔记，稍后验证「移除」不会动数据
    let notes = NoteService::new(app.context().expect("上下文 A"));
    let note = notes
        .create(CreateNoteInput {
            title: "移除前留下的笔记".to_string(),
            category_id: None,
            initial_content: Some("正文".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("新建笔记")
        .note;
    let note_path = first.path().join(note.file_path.replace('\\', "/"));
    assert!(note_path.exists());
    let database = first.path().join(".knowledge").join("database.sqlite");
    assert!(database.exists());

    // 再建 B：活动工作区切走，A 只剩「最近列表」里的记录
    service
        .create("工作区 B", &second.path().to_string_lossy())
        .expect("创建 B");
    assert_eq!(
        app.registry_snapshot().workspaces.len(),
        2,
        "两个工作区都应在最近列表里"
    );

    service.forget(&created.workspace.id).expect("从最近列表移除");
    assert_eq!(
        app.registry_snapshot().workspaces.len(),
        1,
        "移除后最近列表只剩一个"
    );
    assert!(
        app.registry_snapshot()
            .workspaces
            .iter()
            .all(|entry| entry.id != created.workspace.id),
        "被移除的工作区不应还在列表里"
    );

    // 数据必须原封不动
    assert!(note_path.exists(), "移除最近记录不能删除笔记文件");
    assert!(database.exists(), "移除最近记录不能删除数据库");
    assert!(
        first.path().join("00_Inbox").is_dir(),
        "工作区目录结构要保留"
    );

    // 还能重新打开，笔记也还在
    service.close().expect("关闭 B");
    service
        .open(&first.path().to_string_lossy())
        .expect("重新打开 A");
    let reopened = NoteService::new(app.context().expect("上下文"))
        .list(NoteListQuery { page_size: Some(100), ..Default::default() })
        .expect("笔记列表");
    assert!(
        reopened.items.iter().any(|item| item.id == note.id),
        "重新打开后笔记应当还在"
    );
    // 打开会重新登记到最近列表（这是「打开」的正常行为，不是 forget 的副作用）
    assert_eq!(app.registry_snapshot().workspaces.len(), 2);
}
