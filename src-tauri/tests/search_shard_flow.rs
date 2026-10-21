use std::sync::Arc;

use personal_knowledge_os_lib::application::index_service::IndexService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::infrastructure::fts::SEARCH_SHARDS;
use personal_knowledge_os_lib::state::AppInner;

#[test]
fn rebuild_routes_documents_across_all_stable_shards_and_search_stays_complete() {
    let data = tempfile::tempdir().expect("应用数据目录");
    let workspace = tempfile::tempdir().expect("工作区目录");
    let app =
        AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink)).expect("初始化应用状态");
    WorkspaceService::new(Arc::clone(&app))
        .create("分片测试", &workspace.path().to_string_lossy())
        .expect("创建工作区");
    let context = app.context().expect("上下文");
    let notes = NoteService::new(Arc::clone(&context));
    for index in 0..80 {
        notes
            .create(CreateNoteInput {
                title: format!("分片笔记 {index}"),
                category_id: None,
                initial_content: Some(format!(
                    "# 分片笔记 {index}\n\n稳定分片检索 shardtoken{index}。"
                )),
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记");
    }
    let status = IndexService::new(Arc::clone(&context))
        .rebuild()
        .expect("重建分片索引");
    assert_eq!(status.indexed_notes, status.total_notes);

    let shard_counts: Vec<i64> = context
        .db
        .with_conn(|conn| {
            let mut counts = Vec::new();
            for (shard, table) in SEARCH_SHARDS.iter().enumerate() {
                let count: i64 =
                    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })?;
                let mapped: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM search_documents_map WHERE shard = ?1",
                    rusqlite::params![shard as i64],
                    |row| row.get(0),
                )?;
                assert_eq!(count, mapped, "分片 {shard} 的表行数应与映射一致");
                counts.push(count);
            }
            Ok(counts)
        })
        .expect("检查分片");
    assert!(
        shard_counts.iter().filter(|count| **count > 0).count() >= 3,
        "80 篇笔记应分布到至少 3 个稳定分片：{shard_counts:?}"
    );

    let result = SearchService::new(Arc::clone(&context))
        .search(SearchQuery {
            keyword: "shardtoken42".to_string(),
            ..Default::default()
        })
        .expect("跨分片检索");
    assert!(
        result
            .hits
            .iter()
            .any(|hit| hit.note.title == "分片笔记 42"),
        "跨分片 UNION 检索必须找回目标笔记"
    );
}
