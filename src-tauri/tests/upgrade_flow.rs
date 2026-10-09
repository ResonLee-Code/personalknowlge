//! 升级路径验收（V1.2.0）：旧版本工作区打开后应完成迁移并自动重建搜索索引。
//!
//! 场景：库停留在迁移 005（没有 FTS rowid 映射表，索引里是旧格式记录），
//! 用当前版本打开，期望：schema → 6、映射表建立、FTS 自动重建（已索引 == 总数）、检索可用。

use std::sync::Arc;

use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

#[test]
fn opening_a_v1_1_workspace_migrates_and_rebuilds_search_index() {
    let data = tempfile::tempdir().expect("应用数据目录");
    let workspace: TempDir = tempfile::tempdir().expect("工作区目录");
    let root = workspace.path().to_path_buf();
    let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink)).expect("应用状态");
    WorkspaceService::new(Arc::clone(&app))
        .create("升级测试库", &root.to_string_lossy())
        .expect("创建工作区");

    // 造两篇笔记（当前版本，schema 6）
    let notes = NoteService::new(app.context().unwrap());
    let first = notes
        .create(CreateNoteInput {
            title: "升级保留笔记".into(),
            category_id: None,
            initial_content: Some("迁移前的正文：倒排索引与双字扩展".into()),
            tags: Some(vec!["迁移".into()]),
        })
        .expect("创建笔记");
    notes
        .create(CreateNoteInput {
            title: "第二篇".into(),
            category_id: None,
            initial_content: Some("另一段正文".into()),
            tags: None,
        })
        .expect("创建笔记");

    // 模拟旧版本数据库：去掉 006 的映射表与记录，把版本回退到 5，
    // 并写入一条「没有映射」的旧格式 FTS 记录（升级前就是这样）。
    let db_path = root.join(".knowledge/database.sqlite");
    {
        let context = app.context().expect("上下文");
        context
            .db
            .with_conn(|conn| {
                conn.execute("DROP TABLE IF EXISTS search_documents_map", [])?;
                conn.execute("DELETE FROM search_documents_fts", [])?;
                conn.execute("DELETE FROM search_documents_fts WHERE 1=0", [])?;
                conn.execute(
                    "INSERT INTO search_documents_fts (note_id, title, content, keywords)
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params!["note_legacy", "旧索引记录", "旧正文", "旧"],
                )?;
                // 删除 006 的迁移记录即可让下次打开重跑它（005 的记录保留）
                conn.execute("DELETE FROM schema_migrations WHERE version = 6", [])?;
                Ok(())
            })
            .expect("构造旧库状态");
    }
    WorkspaceService::new(Arc::clone(&app))
        .close()
        .expect("关闭工作区");

    // 用当前版本重新打开：应完成迁移 + 自动重建索引
    let reopened = WorkspaceService::new(Arc::clone(&app))
        .open(&root.to_string_lossy())
        .expect("重新打开工作区");

    let context = app.context().expect("上下文");
    assert_eq!(
        context.db.schema_version().expect("schema 版本"),
        6,
        "应自动迁移到 schema 6"
    );
    let (has_map, fts_rows, map_rows): (i64, i64, i64) = context
        .db
        .with_conn(|conn| {
            let map: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='search_documents_map'",
                [],
                |row| row.get(0),
            )?;
            let fts: i64 = conn.query_row("SELECT COUNT(*) FROM search_documents_fts", [], |row| row.get(0))?;
            let mapped: i64 = conn.query_row("SELECT COUNT(*) FROM search_documents_map", [], |row| row.get(0))?;
            Ok((map, fts, mapped))
        })
        .expect("检查索引状态");
    assert_eq!(has_map, 1, "006 应建立 rowid 映射表");
    assert_eq!(reopened.index_status.indexed_notes, reopened.index_status.total_notes, "打开后应完成索引重建");
    assert_eq!(fts_rows, map_rows, "FTS 记录应与映射表一一对应");
    assert!(
        fts_rows >= 3,
        "旧库里的 1 条 + 重建后的全部笔记，应不少于 3 条（实际 {fts_rows}）"
    );

    // 检索仍可用（迁移后功能不退化）
    let hits = SearchService::new(Arc::clone(&context))
        .search(SearchQuery { keyword: "倒排索引".into(), ..Default::default() })
        .expect("检索");
    assert!(
        hits.hits.iter().any(|hit| hit.note.id == first.note.id),
        "迁移后应能检索到原有笔记"
    );
    let db_size = std::fs::metadata(&db_path).map(|meta| meta.len()).unwrap_or(0);
    assert!(db_size > 0, "数据库文件应存在");
}
