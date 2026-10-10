//! DTO 契约测试。
//!
//! 前端只会发送界面实际用到的字段，后端必须容忍缺省字段，
//! 否则会出现「invalid args」这类只在实际运行中才暴露的错误。
//! 这里的 JSON 片段与 `docs/API_CONTRACT.md` 中记录的调用保持一致。

use personal_knowledge_os_lib::domain::backup::{BackupCreateInput, ExportMarkdownInput};
use personal_knowledge_os_lib::application::import_service::ImportNotesInput;
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::note::{
    CreateNoteInput, NoteListQuery, NoteSort, NoteStatus, SaveNoteInput,
};
use personal_knowledge_os_lib::domain::research::ResearchRequest;
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::domain::settings::UpdateSettingsInput;

#[test]
fn note_list_query_accepts_partial_payload() {
    // 前端刷新列表时只发送排序与分页
    let query: NoteListQuery = serde_json::from_str(r#"{ "sort": "updatedDesc", "pageSize": 300 }"#)
        .expect("列表查询应接受缺省字段");
    assert_eq!(query.page, None);
    assert_eq!(query.page_size, Some(300));
    assert_eq!(query.sort, Some(NoteSort::UpdatedDesc));
    assert!(!query.favorite_only);
    assert!(!query.inbox_only);
    assert_eq!(query.status, None);

    // 空对象也必须可用
    let empty: NoteListQuery = serde_json::from_str("{}").expect("空查询应可用");
    assert_eq!(empty.status, None);
}

#[test]
fn note_list_query_accepts_null_and_status_filters() {
    let query: NoteListQuery = serde_json::from_str(
        r#"{ "categoryId": null, "tagId": null, "status": "deleted", "inboxOnly": true, "keyword": null }"#,
    )
    .expect("分类与标签为空时应可用");
    assert_eq!(query.status, Some(NoteStatus::Deleted));
    assert!(query.inbox_only);
    assert_eq!(query.category_id, None);
    assert_eq!(query.keyword, None);
}

#[test]
fn create_note_input_requires_only_title() {
    let input: CreateNoteInput =
        serde_json::from_str(r#"{ "title": "标题" }"#).expect("只需标题");
    assert_eq!(input.title, "标题");
    assert!(input.category_id.is_none());
    assert!(input.initial_content.is_none());
    assert!(input.tags.is_none());
}

#[test]
fn save_note_input_accepts_null_optionals() {
    let input: SaveNoteInput = serde_json::from_str(
        r#"{ "id": "note_1", "content": "正文", "baseContentHash": null, "title": null, "categoryId": null, "tags": null }"#,
    )
    .expect("保存输入应接受 null 可选字段");
    assert_eq!(input.id, "note_1");
    assert!(input.base_content_hash.is_none());
    assert!(input.tags.is_none());
}

#[test]
fn search_query_accepts_partial_payload() {
    let query: SearchQuery = serde_json::from_str(
        r#"{ "keyword": "全文", "categoryId": null, "favoriteOnly": false, "sort": "relevance", "page": 1, "pageSize": 50 }"#,
    )
    .expect("搜索查询应接受缺省字段");
    assert_eq!(query.keyword, "全文");
    assert_eq!(query.updated_before, None);
    assert_eq!(query.scope_note_id, None);
}

#[test]
fn settings_and_category_inputs_accept_partial_payload() {
    let settings: UpdateSettingsInput =
        serde_json::from_str(r#"{ "autosaveDelayMs": 1200 }"#).expect("设置更新应接受部分字段");
    assert_eq!(settings.autosave_delay_ms, Some(1200));
    assert!(settings.theme.is_none());

    let category: CreateCategoryInput =
        serde_json::from_str(r#"{ "name": "数据库" }"#).expect("分类创建只需名称");
    assert_eq!(category.name, "数据库");
    assert!(category.parent_id.is_none());
    assert!(category.create_directory.is_none());
}

#[test]
fn backup_inputs_accept_partial_payload() {
    let backup: BackupCreateInput =
        serde_json::from_str(r#"{ "targetDir": null, "note": null }"#).expect("备份输入应接受 null");
    assert!(backup.target_dir.is_none());

    let export: ExportMarkdownInput =
        serde_json::from_str(r#"{ "targetDir": "/tmp/export", "includeAttachments": true }"#)
            .expect("导出输入应可解析");
    assert!(export.include_attachments);

    let export_minimal: ExportMarkdownInput =
        serde_json::from_str(r#"{ "targetDir": "/tmp/export" }"#)
            .expect("导出输入应容忍缺省字段");
    assert!(!export_minimal.include_attachments);
}

#[test]
fn import_input_accepts_paths_without_category() {
    // 前端在「导入到 Inbox」时不发送 categoryId
    let input: ImportNotesInput = serde_json::from_str(r#"{ "paths": ["/tmp/a.md", "/tmp/b.txt"] }"#)
        .expect("导入输入应接受缺省分类");
    assert_eq!(input.paths.len(), 2);
    assert!(input.category_id.is_none());

    let with_category: ImportNotesInput =
        serde_json::from_str(r#"{ "paths": ["/tmp/a.md"], "categoryId": "cat_1" }"#)
            .expect("带分类的导入输入应可解析");
    assert_eq!(with_category.category_id.as_deref(), Some("cat_1"));
}

#[test]
fn settings_input_accepts_fractional_pixel_values() {
    // Retina 屏拖拽分栏时 clientX 可能是小数（如 320.5），后端应四舍五入而不是报错
    let input: UpdateSettingsInput = serde_json::from_str(
        r#"{ "sidebarWidth": 320.5, "listWidth": 400.4, "autosaveDelayMs": 1000.0, "editorSplitRatio": 0.512 }"#,
    )
    .expect("小数像素值应被接受");
    assert_eq!(input.sidebar_width, Some(321));
    assert_eq!(input.list_width, Some(400));
    assert_eq!(input.autosave_delay_ms, Some(1000));
    assert_eq!(input.editor_split_ratio, Some(0.512));

    // 整数输入同样可用
    let integers: UpdateSettingsInput =
        serde_json::from_str(r#"{ "sidebarWidth": 248, "editorSplitRatio": 0 }"#)
            .expect("整数应可用");
    assert_eq!(integers.sidebar_width, Some(248));
    assert_eq!(integers.editor_split_ratio, Some(0.0));
}

#[test]
fn research_request_accepts_a_bare_topic() {
    // 研究助手的第一步只有课题是必填，其余（范围/时间窗/写回意图）都可能缺省
    let input: ResearchRequest =
        serde_json::from_str(r#"{ "topic": "Rust 异步运行时调度" }"#).expect("只给课题也要能用");
    assert_eq!(input.topic, "Rust 异步运行时调度");
    assert!(input.scope.is_none());
    assert!(input.time_window_days.is_none());
    assert!(!input.write_back);
    assert!(input.target_category_id.is_none());

    let full: ResearchRequest = serde_json::from_str(
        r#"{ "topic": "Rust 异步运行时调度", "scope": "运行时实现", "timeWindowDays": 365,
             "writeBack": true, "targetCategoryId": "cat_1" }"#,
    )
    .expect("完整输入应可解析");
    assert_eq!(full.time_window_days, Some(365));
    assert!(full.write_back);
    assert_eq!(full.target_category_id.as_deref(), Some("cat_1"));
}
