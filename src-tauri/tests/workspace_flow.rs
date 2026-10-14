//! 端到端集成测试：覆盖设计文档 22.2 的集成场景。

use std::path::PathBuf;
use std::sync::Arc;

use personal_knowledge_os_lib::application::backup_service::BackupService;
use personal_knowledge_os_lib::application::category_service::CategoryService;
use personal_knowledge_os_lib::application::context::WorkspaceContext;
use personal_knowledge_os_lib::application::index_service::IndexService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::tag_service::TagService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::error::ErrorCode;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::infrastructure::watcher::{
    start_best_watcher, FileChange, FileChangeBatch, FileChangeKind,
};
use personal_knowledge_os_lib::repositories::{NoteRepository, SqliteNoteRepository};
use personal_knowledge_os_lib::domain::backup::{
    BackupCreateInput, ExportMarkdownInput, RestoreBackupInput,
};
use personal_knowledge_os_lib::domain::category::{CategoryDeleteStrategy, CreateCategoryInput, DeleteCategoryInput};
use personal_knowledge_os_lib::domain::note::{
    CreateNoteInput, NoteListQuery, NoteStatus, SaveNoteInput,
};
use personal_knowledge_os_lib::domain::search::{SearchQuery, SearchSort};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

/// 测试环境：临时应用数据目录 + 临时工作区目录。
struct Harness {
    _data: TempDir,
    _workspace_dir: TempDir,
    app: Arc<AppInner>,
    root: PathBuf,
}

impl Harness {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("创建应用数据目录");
        let workspace_dir = tempfile::tempdir().expect("创建工作区目录");
        let app = AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink))
            .expect("初始化应用状态");
        let root = workspace_dir.path().to_path_buf();
        Self { _data: data, _workspace_dir: workspace_dir, app, root }
    }

    /// 创建并打开一个全新工作区。
    fn create_workspace(&self) -> Arc<WorkspaceContext> {
        WorkspaceService::new(Arc::clone(&self.app))
            .create("测试知识库", &self.root.to_string_lossy())
            .expect("创建工作区");
        self.context()
    }

    fn context(&self) -> Arc<WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.context())
    }

    fn categories(&self) -> CategoryService {
        CategoryService::new(self.context())
    }

    fn tags(&self) -> TagService {
        TagService::new(self.context())
    }

    fn search(&self) -> SearchService {
        SearchService::new(self.context())
    }

    fn index(&self) -> IndexService {
        IndexService::new(self.context())
    }

    fn backups(&self) -> BackupService {
        BackupService::new(self.context())
    }

    fn absolute(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }
}

fn list_all(notes: &NoteService) -> Vec<personal_knowledge_os_lib::domain::note::NoteSummary> {
    notes
        .list(NoteListQuery { page_size: Some(500), ..Default::default() })
        .expect("列出笔记")
        .items
}

#[test]
fn creates_workspace_with_layout_and_welcome_note() {
    let harness = Harness::new();
    let context = harness.create_workspace();

    // 目录结构（设计文档 12.1）
    for relative in [
        ".knowledge",
        ".knowledge/database.sqlite",
        ".knowledge/config.json",
        "attachments",
        "00_Inbox",
        "01_技术",
        "99_Archive",
        ".trash",
    ] {
        assert!(harness.absolute(relative).exists(), "{relative} 应存在");
    }

    // 默认分类来自目录约定
    let categories = harness.categories().list().expect("分类列表");
    assert!(categories.iter().any(|category| category.name == "技术"));
    assert!(categories.iter().any(|category| category.dir_name == "01_技术"));

    // 欢迎笔记
    let notes = list_all(&harness.notes());
    assert_eq!(notes.len(), 1);
    assert!(notes[0].title.contains("欢迎"));
    assert!(harness.absolute(&notes[0].file_path).exists());
    assert!(notes[0].file_path.starts_with("00_Inbox/"));

    // 索引状态
    let status = harness.search().status().expect("索引状态");
    assert_eq!(status.indexed_notes, 1);
    assert_eq!(status.total_notes, 1);
    assert!(context.settings().autosave_enabled);
}

#[test]
fn creates_saves_and_reads_note_with_frontmatter() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();

    let detail = notes
        .create(CreateNoteInput {
            title: "SQLite 全文搜索学习笔记".to_string(),
            category_id: None,
            initial_content: Some("第一行\n\n第二行".to_string()),
            tags: Some(vec!["sqlite".to_string(), "Search".to_string()]),
            parent_id: None,
        })
        .expect("创建笔记");

    assert_eq!(detail.note.title, "SQLite 全文搜索学习笔记");
    assert_eq!(detail.content, "第一行\n\n第二行");
    assert_eq!(detail.tags.len(), 2);

    let raw = std::fs::read_to_string(harness.absolute(&detail.note.file_path)).expect("读取文件");
    assert!(raw.starts_with("---\n"));
    assert!(raw.contains("id: note_"));
    assert!(raw.contains("title: SQLite 全文搜索学习笔记"));
    // 标签使用设计文档 12.3 的缩进块序列风格
    assert!(raw.contains("tags:\n  - sqlite\n  - Search\n"), "{raw}");
    assert!(raw.contains("第一行"));
    assert!(raw.ends_with('\n'));

    // 保存新内容
    let saved = notes
        .save(SaveNoteInput {
            id: detail.note.id.clone(),
            content: "更新后的内容：中文全文检索验证。".to_string(),
            base_content_hash: detail.note.content_hash.clone(),
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("保存笔记");
    assert!(saved.index_updated);
    assert_ne!(saved.content_hash, detail.note.content_hash.unwrap_or_default());

    let reloaded = notes.get(&detail.note.id).expect("重新读取");
    assert!(reloaded.content.contains("中文全文检索验证"));
    assert!(reloaded.note.word_count > 0);

    let search = harness.search().search(SearchQuery {
        keyword: "全文检索".to_string(),
        ..Default::default()
    });
    assert_eq!(search.expect("搜索").hits.len(), 1);
}

#[test]
fn rejects_save_when_file_changed_externally() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();

    let detail = notes
        .create(CreateNoteInput {
            title: "冲突检测".to_string(),
            category_id: None,
            initial_content: Some("原始内容".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");

    // 模拟外部编辑器修改文件
    let path = harness.absolute(&detail.note.file_path);
    let external = format!("{}\n\n外部追加内容", std::fs::read_to_string(&path).unwrap());
    std::fs::write(&path, external).expect("外部写入");

    let error = notes
        .save(SaveNoteInput {
            id: detail.note.id.clone(),
            content: "应用内新内容".to_string(),
            base_content_hash: detail.note.content_hash.clone(),
            title: None,
            category_id: None,
            tags: None,
        })
        .expect_err("应检测到冲突");
    assert_eq!(error.code, ErrorCode::NoteConflict);
    let details = error.details.expect("冲突详情");
    assert!(details.get("currentContent").is_some());

    // 外部内容未被覆盖
    let after = std::fs::read_to_string(&path).unwrap();
    assert!(after.contains("外部追加内容"));
}

#[test]
fn renames_moves_and_duplicates_notes() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();
    let categories = harness.categories();

    let tech = categories
        .create(CreateCategoryInput {
            name: "数据库".to_string(),
            parent_id: harness
                .categories()
                .list()
                .unwrap()
                .into_iter()
                .find(|category| category.name == "技术")
                .map(|category| category.id),
            create_directory: Some(true),
        })
        .expect("创建子分类");

    let detail = notes
        .create(CreateNoteInput {
            title: "初稿".to_string(),
            category_id: None,
            initial_content: Some("内容".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");
    let original_path = detail.note.file_path.clone();

    // 重命名（同步文件名）
    let renamed = notes.rename(&detail.note.id, "SQLite 索引设计", true).expect("重命名");
    assert_eq!(renamed.note.title, "SQLite 索引设计");
    assert_ne!(renamed.note.file_path, original_path);
    assert!(renamed.note.file_path.contains("SQLite-索引设计"));
    assert!(!harness.absolute(&original_path).exists());
    assert!(harness.absolute(&renamed.note.file_path).exists());

    // 移动到子分类：文件应进入对应目录
    let moved = notes
        .move_to_category(&detail.note.id, Some(&tech.id))
        .expect("移动笔记");
    assert!(moved.note.file_path.starts_with("01_技术/"), "{}", moved.note.file_path);
    assert!(moved.note.file_path.contains("数据库"), "{}", moved.note.file_path);
    assert!(harness.absolute(&moved.note.file_path).exists());
    assert_eq!(moved.category_path, "技术/数据库");
    assert!(moved.tags.is_empty());

    // 复制
    let duplicate = notes.duplicate(&detail.note.id).expect("复制笔记");
    assert!(duplicate.note.title.contains("副本"));
    assert_ne!(duplicate.note.id, detail.note.id);
    assert!(harness.absolute(&duplicate.note.file_path).exists());
    assert_eq!(list_all(&notes).len(), 3); // 欢迎笔记 + 原笔记 + 副本
}

#[test]
fn deletes_restores_and_purges_notes() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();

    let detail = notes
        .create(CreateNoteInput {
            title: "待删除".to_string(),
            category_id: None,
            initial_content: Some("临时内容".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");
    let file_path = detail.note.file_path.clone();

    notes.delete(&detail.note.id).expect("删除笔记");
    assert!(!harness.absolute(&file_path).exists(), "原文件应移出工作区");
    assert!(harness.absolute(".trash").join(&file_path).exists(), "文件应进入回收站");

    let deleted = notes.list_deleted().expect("回收站列表");
    assert_eq!(deleted.len(), 1);
    assert_eq!(deleted[0].status, NoteStatus::Deleted);
    // 删除后不再出现在搜索结果中
    assert!(harness
        .search()
        .search(SearchQuery { keyword: "临时内容".into(), ..Default::default() })
        .unwrap()
        .hits
        .is_empty());

    let restored = notes.restore(&detail.note.id).expect("恢复笔记");
    assert_eq!(restored.note.status, NoteStatus::Active);
    assert!(harness.absolute(&restored.note.file_path).exists());
    assert!(notes.list_deleted().unwrap().is_empty());

    notes.delete(&detail.note.id).expect("再次删除");
    notes.purge(&detail.note.id).expect("永久删除");
    assert!(notes.get(&detail.note.id).is_err());
    assert!(notes.list_deleted().unwrap().is_empty());
}

#[test]
fn searches_chinese_english_and_ranks_titles_first() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();

    let title_hit = notes
        .create(CreateNoteInput {
            title: "全文搜索设计".to_string(),
            category_id: None,
            initial_content: Some("讨论索引与排序".to_string()),
            tags: Some(vec!["搜索".to_string()]),
            parent_id: None,
        })
        .expect("创建笔记");
    notes
        .create(CreateNoteInput {
            title: "零散记录".to_string(),
            category_id: None,
            initial_content: Some("这里也提到了全文搜索的实现，但没有出现在标题中。".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");
    notes
        .create(CreateNoteInput {
            title: "Rust ownership notes".to_string(),
            category_id: None,
            initial_content: Some("Borrow checker and lifetimes".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");

    // 中文双字短语
    let chinese = harness
        .search()
        .search(SearchQuery { keyword: "全文搜索".into(), ..Default::default() })
        .expect("中文搜索");
    assert_eq!(chinese.hits.len(), 2, "应命中标题与正文");
    assert_eq!(chinese.hits[0].note.id, title_hit.note.id, "标题命中优先");
    assert!(chinese.hits[0].title_matched);
    assert!(!chinese.hits[0].snippet.is_empty(), "应生成摘要");

    // 中文单字
    let single = harness
        .search()
        .search(SearchQuery { keyword: "索".into(), ..Default::default() })
        .expect("单字搜索");
    assert!(!single.hits.is_empty());

    // 英文前缀
    let english = harness
        .search()
        .search(SearchQuery { keyword: "borr".into(), ..Default::default() })
        .expect("英文搜索");
    assert_eq!(english.hits.len(), 1);

    // 无结果
    let empty = harness
        .search()
        .search(SearchQuery { keyword: "不存在的关键词".into(), ..Default::default() })
        .expect("空结果搜索");
    assert!(empty.hits.is_empty());
    assert_eq!(empty.total, 0);

    // 空关键词回退为按时间浏览
    let browse = harness
        .search()
        .search(SearchQuery { keyword: "".into(), ..Default::default() })
        .expect("空关键词");
    assert_eq!(browse.total, 4);
}

#[test]
fn search_supports_filters_tags_and_highlights() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();
    let categories = harness.categories();

    let category = categories
        .create(CreateCategoryInput {
            name: "检索".to_string(),
            parent_id: None,
            create_directory: Some(true),
        })
        .expect("创建分类");

    let target = notes
        .create(CreateNoteInput {
            title: "带标签的笔记".to_string(),
            category_id: Some(category.id.clone()),
            initial_content: Some("关键词内容：向量检索与倒排索引".to_string()),
            tags: Some(vec!["检索".to_string()]),
            parent_id: None,
        })
        .expect("创建笔记");
    notes
        .create(CreateNoteInput {
            title: "其他笔记".to_string(),
            category_id: None,
            initial_content: Some("关键词内容：向量检索与倒排索引".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");

    let tags = harness.tags().list().expect("标签列表");
    let tag = tags.iter().find(|tag| tag.name == "检索").expect("标签已创建");
    assert_eq!(tag.note_count, 1);

    // 关键词 + 标签过滤
    let filtered = harness
        .search()
        .search(SearchQuery {
            keyword: "向量检索".into(),
            tag_id: Some(tag.id.clone()),
            ..Default::default()
        })
        .expect("带标签过滤的搜索");
    assert_eq!(filtered.hits.len(), 1);
    assert_eq!(filtered.hits[0].note.id, target.note.id);

    // 分类过滤
    let by_category = harness
        .search()
        .search(SearchQuery {
            keyword: "向量检索".into(),
            category_id: Some(category.id.clone()),
            ..Default::default()
        })
        .expect("带分类过滤的搜索");
    assert_eq!(by_category.hits.len(), 1);

    // 摘要高亮
    let segments = &by_category.hits[0].snippet;
    let highlighted: String = segments
        .iter()
        .filter(|segment| segment.matched)
        .map(|segment| segment.text.clone())
        .collect();
    assert_eq!(highlighted, "向量检索");

    // 收藏过滤
    notes.set_favorite(&target.note.id, true).expect("收藏");
    let favorites = harness
        .search()
        .search(SearchQuery {
            keyword: "向量检索".into(),
            favorite_only: true,
            ..Default::default()
        })
        .expect("收藏过滤");
    assert_eq!(favorites.hits.len(), 1);

    // 标签删除不影响笔记正文
    harness.tags().delete(&tag.id).expect("删除标签");
    assert!(notes.get(&target.note.id).expect("笔记仍存在").content.contains("向量检索"));
}

#[test]
fn adopts_external_files_and_marks_missing_ones() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();

    // 外部新建 Markdown（放在分类目录中）
    let external_relative = "01_技术/外部笔记.md";
    std::fs::write(
        harness.absolute(external_relative),
        "---\nid: note_external_1\ntitle: 外部笔记\ncreated_at: 2026-09-23T10:00:00+08:00\nupdated_at: 2026-09-23T10:00:00+08:00\ntags:\n  - 外部\n---\n\n这是外部创建的内容，包含 中文检索 关键词。\n",
    )
    .expect("外部写入");

    let report = harness.index().scan_workspace().expect("扫描工作区");
    assert_eq!(report.added, 1);

    let adopted = notes.get("note_external_1").expect("按 frontmatter id 采纳");
    assert_eq!(adopted.note.title, "外部笔记");
    assert_eq!(adopted.category_path, "技术", "目录应映射到分类");
    assert_eq!(adopted.tags.len(), 1);
    assert_eq!(adopted.note.word_count > 0, true);
    assert!(!harness
        .search()
        .search(SearchQuery { keyword: "中文检索".into(), ..Default::default() })
        .unwrap()
        .hits
        .is_empty());

    // 外部修改
    std::fs::write(
        harness.absolute(external_relative),
        "---\nid: note_external_1\ntitle: 外部笔记（已改）\n---\n\n更新后的正文。\n",
    )
    .unwrap();
    let report = harness.index().scan_workspace().expect("再次扫描");
    assert_eq!(report.updated, 1);
    let updated = notes.get("note_external_1").unwrap();
    assert_eq!(updated.note.title, "外部笔记（已改）");
    assert!(updated.content.contains("更新后的正文"));

    // 外部删除
    std::fs::remove_file(harness.absolute(external_relative)).unwrap();
    let report = harness.index().scan_workspace().expect("第三次扫描");
    assert_eq!(report.missing, 1);
    assert_eq!(notes.list_missing().unwrap().len(), 1);
    assert!(harness
        .search()
        .search(SearchQuery { keyword: "更新后的正文".into(), ..Default::default() })
        .unwrap()
        .hits
        .is_empty());
}

#[test]
fn manages_categories_with_directory_rename_and_delete_strategies() {
    let harness = Harness::new();
    harness.create_workspace();
    let categories = harness.categories();
    let notes = harness.notes();

    let parent = categories
        .create(CreateCategoryInput {
            name: "资料".into(),
            parent_id: None,
            create_directory: Some(true),
        })
        .expect("创建分类");
    let child = categories
        .create(CreateCategoryInput {
            name: "论文".into(),
            parent_id: Some(parent.id.clone()),
            create_directory: Some(true),
        })
        .expect("创建子分类");

    let detail = notes
        .create(CreateNoteInput {
            title: "论文摘要".into(),
            category_id: Some(child.id.clone()),
            initial_content: Some("摘要内容".into()),
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");
    assert!(detail.note.file_path.contains("资料"), "{}", detail.note.file_path);
    assert!(detail.note.file_path.contains("论文"), "{}", detail.note.file_path);

    // 重命名分类 -> 目录同步改名，笔记路径更新
    let renamed = categories.rename(&parent.id, "参考资料").expect("重命名分类");
    assert_eq!(renamed.name, "参考资料");
    let note_after = notes.get(&detail.note.id).unwrap();
    assert!(note_after.note.file_path.contains("参考资料"), "{}", note_after.note.file_path);
    assert!(harness.absolute(&note_after.note.file_path).exists());

    // 分类树结构
    let tree = harness.categories().tree().expect("分类树");
    let parent_node = tree
        .iter()
        .find(|node| node.category.id == parent.id)
        .expect("父节点");
    assert_eq!(parent_node.children.len(), 1);
    assert_eq!(parent_node.children[0].category.id, child.id);
    assert_eq!(parent_node.children[0].depth, 1);

    // 删除分类（移动到 Inbox）
    let report = categories
        .delete(DeleteCategoryInput {
            id: parent.id.clone(),
            strategy: CategoryDeleteStrategy::MoveToInbox,
        })
        .expect("删除分类");
    assert_eq!(report.deleted_categories, 2);
    assert_eq!(report.moved_notes, 1);
    let moved = notes.get(&detail.note.id).unwrap();
    assert!(moved.note.category_id.is_none());
    assert!(moved.note.file_path.starts_with("00_Inbox/"));
    assert!(categories.list().unwrap().iter().all(|c| c.id != parent.id));
}

#[test]
fn rebuilds_index_and_reports_status() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();
    for index in 0..5 {
        notes
            .create(CreateNoteInput {
                title: format!("索引笔记 {index}"),
                category_id: None,
                initial_content: Some(format!("第 {index} 篇内容，用于索引重建测试。")),
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记");
    }

    // 手动清空索引内容，模拟索引损坏
    let context = harness.context();
    context
        .db
        .with_conn(|conn| {
            conn.execute("DELETE FROM search_documents_fts", [])?;
            Ok(())
        })
        .expect("清空索引");

    let before = harness.search().status().expect("状态");
    assert_eq!(before.indexed_notes, 0);
    assert_eq!(before.total_notes, 6);

    let rebuilt = harness.search().rebuild_index().expect("重建索引");
    assert_eq!(rebuilt.indexed_notes, 6);
    assert_eq!(rebuilt.total_notes, 6);
    assert!(rebuilt.last_rebuilt_at.is_some());
    assert!(harness
        .search()
        .search(SearchQuery { keyword: "索引重建".into(), ..Default::default() })
        .unwrap()
        .hits
        .len()
        >= 1);
}

#[test]
fn creates_validates_and_restores_backup_and_exports_markdown() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();
    notes
        .create(CreateNoteInput {
            title: "备份内容".to_string(),
            category_id: None,
            initial_content: Some("需要被备份的正文".to_string()),
            tags: Some(vec!["备份".to_string()]),
            parent_id: None,
        })
        .expect("创建笔记");

    let backup = harness
        .backups()
        .create(BackupCreateInput { target_dir: None, note: None })
        .expect("创建备份");
    assert!(backup.file_name.starts_with("KnowledgeBackup_"));
    assert!(PathBuf::from(&backup.path).exists());
    assert_eq!(backup.note_count, 2);
    assert!(backup.includes_database);
    assert!(backup.size_bytes > 0);

    let validation = harness.backups().validate(&backup.path).expect("校验备份");
    assert!(validation.valid, "校验细节：{:?}", validation.checks);
    assert!(validation.checks.iter().all(|check| check.passed));
    assert_eq!(validation.note_count, 2);

    let listed = harness.backups().list().expect("备份列表");
    assert_eq!(listed.len(), 1);

    // 恢复到新目录
    let restore_target = harness._data.path().join("restored");
    let report = harness
        .backups()
        .restore(RestoreBackupInput {
            archive_path: backup.path.clone(),
            target_dir: restore_target.to_string_lossy().to_string(),
        })
        .expect("恢复备份");
    assert!(report.extracted_files >= 3);
    assert!(restore_target.join(".knowledge/database.sqlite").exists());
    assert!(restore_target.join("00_Inbox").is_dir());

    // 恢复到非空目录必须被拒绝
    let error = harness
        .backups()
        .restore(RestoreBackupInput {
            archive_path: backup.path.clone(),
            target_dir: restore_target.to_string_lossy().to_string(),
        })
        .expect_err("非空目录应被拒绝");
    assert_eq!(error.code, ErrorCode::BackupFailed);

    // 导出 Markdown
    let export_target = harness._data.path().join("exported");
    let exported = harness
        .backups()
        .export_markdown(ExportMarkdownInput {
            target_dir: export_target.to_string_lossy().to_string(),
            include_attachments: true,
        })
        .expect("导出 Markdown");
    assert_eq!(exported.note_count, 2);
    assert!(export_target.join("export-manifest.json").exists());
    let exported_files: Vec<_> = walkdir::WalkDir::new(&export_target)
        .into_iter()
        .flatten()
        .filter(|entry| entry.path().extension().map(|ext| ext == "md").unwrap_or(false))
        .collect();
    assert_eq!(exported_files.len(), 2);
}

#[test]
fn closes_and_reopens_workspace_persisting_state() {
    let harness = Harness::new();
    let context = harness.create_workspace();
    let notes = harness.notes();

    notes
        .create(CreateNoteInput {
            title: "持久化验证".to_string(),
            category_id: None,
            initial_content: Some("关闭再打开后应能找回".to_string()),
            tags: Some(vec!["持久化".to_string()]),
            parent_id: None,
        })
        .expect("创建笔记");
    let workspace_id = context.workspace.id.clone();

    let service = WorkspaceService::new(Arc::clone(&harness.app));
    service.close().expect("关闭工作区");
    assert!(harness.app.active().is_none());
    assert!(harness.app.context().is_err());

    // 通过注册表重新打开
    let reopened = service.switch(&workspace_id).expect("切换工作区");
    assert_eq!(reopened.workspace.id, workspace_id);
    assert_eq!(reopened.stats.note_count, 2);
    let listed = list_all(&harness.notes());
    assert!(listed.iter().any(|note| note.title == "持久化验证"));
    assert!(listed
        .iter()
        .any(|note| note.tags.iter().any(|tag| tag == "持久化")));

    // 工作区列表包含当前标记
    let workspaces = service.list();
    assert_eq!(workspaces.len(), 1);
    assert!(workspaces[0].is_active);
    assert!(workspaces[0].is_available);
}

#[test]
fn settings_and_path_safety_behave_as_documented() {
    let harness = Harness::new();
    harness.create_workspace();

    let service = WorkspaceService::new(Arc::clone(&harness.app));
    let updated = service
        .update_settings(personal_knowledge_os_lib::domain::settings::UpdateSettingsInput {
            autosave_delay_ms: Some(1200),
            font_size: Some(18),
            editor_mode: Some("preview".into()),
            ..Default::default()
        })
        .expect("更新设置");
    assert_eq!(updated.autosave_delay_ms, 1200);
    assert_eq!(updated.font_size, 18);
    assert_eq!(updated.editor_mode, "preview");

    // 非法数值被限制在合理范围
    let clamped = service
        .update_settings(personal_knowledge_os_lib::domain::settings::UpdateSettingsInput {
            autosave_delay_ms: Some(1),
            font_size: Some(200),
            ..Default::default()
        })
        .expect("边界值设置");
    assert_eq!(clamped.autosave_delay_ms, 200);
    assert_eq!(clamped.font_size, 32);

    // 路径穿越必须被拒绝
    let context = harness.context();
    assert!(context.storage.read_raw("../../etc/passwd").is_err());
    assert!(context.storage.read_raw("/etc/passwd").is_err());
    assert!(context.storage.write_atomic("../escape.md", "内容").is_err());

    // 文件名非法字符被清理
    let notes = harness.notes();
    let detail = notes
        .create(CreateNoteInput {
            title: "非法/字符:测试*标题?".to_string(),
            category_id: None,
            initial_content: None,
            tags: None,
            parent_id: None,
        })
        .expect("创建带特殊字符标题的笔记");
    assert!(!detail.note.file_path.contains(':'));
    assert!(!detail.note.file_path.contains('*'));
    assert!(harness.absolute(&detail.note.file_path).exists());

    // 排序选项可用
    let by_title = notes
        .list(NoteListQuery {
            sort: Some(personal_knowledge_os_lib::domain::note::NoteSort::TitleAsc),
            ..Default::default()
        })
        .expect("按标题排序");
    assert!(by_title.items.len() >= 2);
}

#[test]
fn search_sorting_and_paging_work() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();
    for index in 0..8 {
        notes
            .create(CreateNoteInput {
                title: format!("分页笔记 {index}"),
                category_id: None,
                initial_content: Some("共同关键词：分页测试".to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记");
    }

    let first_page = harness
        .search()
        .search(SearchQuery {
            keyword: "分页测试".into(),
            page: Some(1),
            page_size: Some(3),
            sort: Some(SearchSort::UpdatedDesc),
            ..Default::default()
        })
        .expect("第一页");
    assert_eq!(first_page.hits.len(), 3);
    assert_eq!(first_page.total, 8);

    let second_page = harness
        .search()
        .search(SearchQuery {
            keyword: "分页测试".into(),
            page: Some(2),
            page_size: Some(3),
            ..Default::default()
        })
        .expect("第二页");
    assert_eq!(second_page.hits.len(), 3);
    assert_ne!(first_page.hits[0].note.id, second_page.hits[0].note.id);

    let by_title = harness
        .search()
        .search(SearchQuery {
            keyword: "分页测试".into(),
            sort: Some(SearchSort::TitleAsc),
            page_size: Some(20),
            ..Default::default()
        })
        .expect("按标题排序");
    let titles: Vec<String> = by_title.hits.iter().map(|hit| hit.note.title.clone()).collect();
    let mut sorted = titles.clone();
    sorted.sort();
    assert_eq!(titles, sorted);
}

#[test]
fn ignores_self_writes_and_applies_external_events() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();

    let detail = notes
        .create(CreateNoteInput {
            title: "事件去重".to_string(),
            category_id: None,
            initial_content: Some("应用内写入的内容".to_string()),
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");

    // 应用自身写入会产生文件系统事件，必须被识别为回声并忽略（设计文档 19.2）
    let self_batch = FileChangeBatch {
        changes: vec![FileChange {
            path: detail.note.file_path.clone(),
            kind: FileChangeKind::Modified,
        }],
        directories: Vec::new(),
        attachments_changed: false,
    };
    let report = harness.index().apply_file_changes(self_batch).expect("处理自身写入事件");
    assert_eq!(report.unchanged, 1);
    assert_eq!(report.updated, 0);

    // 外部真实修改应被采纳
    let path = harness.absolute(&detail.note.file_path);
    std::fs::write(
        &path,
        "---\nid: note_echo\ntitle: 事件去重\ntags:\n  - 外部修改\n---\n\n外部修改后的内容 with english text\n",
    )
    .expect("外部写入");
    let external_batch = FileChangeBatch {
        changes: vec![FileChange {
            path: detail.note.file_path.clone(),
            kind: FileChangeKind::Modified,
        }],
        directories: Vec::new(),
        attachments_changed: false,
    };
    let report = harness.index().apply_file_changes(external_batch).expect("处理外部事件");
    assert_eq!(report.updated, 1);
    let reloaded = notes.get(&detail.note.id).expect("重新读取");
    assert!(reloaded.content.contains("外部修改后的内容"));
    assert_eq!(reloaded.tags.len(), 1);

    // 外部删除事件
    std::fs::remove_file(&path).expect("外部删除");
    let removed_batch = FileChangeBatch {
        changes: vec![FileChange {
            path: detail.note.file_path.clone(),
            kind: FileChangeKind::Removed,
        }],
        directories: Vec::new(),
        attachments_changed: false,
    };
    let report = harness.index().apply_file_changes(removed_batch).expect("处理删除事件");
    assert_eq!(report.missing, 1);
    assert_eq!(notes.list_missing().unwrap().len(), 1);
}

#[test]
fn file_watcher_reports_external_changes() {
    let harness = Harness::new();
    let context = harness.create_workspace();
    let index = IndexService::new(Arc::clone(&context));

    let (sender, receiver) = std::sync::mpsc::channel();
    let weak = Arc::downgrade(&context);
    let mut watcher = start_best_watcher(context.paths.root().to_path_buf(), move |batch| {
        if let Some(context) = weak.upgrade() {
            let service = IndexService::new(context);
            let report = service.apply_file_changes(batch).expect("处理监听事件");
            let _ = sender.send(report);
        }
    })
    .expect("启动文件监听")
    .0;

    // 外部创建文件
    std::fs::write(
        harness.absolute("00_Inbox/监听测试.md"),
        "---\nid: note_watch_1\ntitle: 监听测试\n---\n\n监听器应自动采纳这篇笔记。\n",
    )
    .expect("外部创建文件");

    // 事件粒度可能是文件或目录，这里断言可观察结果：外部文件最终成为笔记
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut adopted = false;
    while std::time::Instant::now() < deadline {
        let _ = receiver.recv_timeout(std::time::Duration::from_secs(5));
        adopted = context
            .db
            .with_conn(|conn| SqliteNoteRepository.get(conn, "note_watch_1"))
            .map(|note| note.is_some())
            .unwrap_or(false);
        if adopted {
            break;
        }
    }
    assert!(adopted, "监听器应采纳外部新建的文件");
    let _ = index;
    watcher.stop();
}

#[test]
fn layout_settings_round_trip_and_clamp() {
    use personal_knowledge_os_lib::domain::settings::UpdateSettingsInput;

    let harness = Harness::new();
    harness.create_workspace();
    let service = WorkspaceService::new(Arc::clone(&harness.app));

    // 分栏宽度与分屏比例应原样保存（前端已取整 / 保留三位小数）
    let updated = service
        .update_settings(UpdateSettingsInput {
            sidebar_width: Some(301),
            list_width: Some(432),
            editor_split_ratio: Some(0.512),
            editor_mode: Some("split".into()),
            list_sort: Some("manual".into()),
            theme: Some("dark".into()),
            // 大纲位置 / 选中后自动显示大纲（V1.3.0 布局调整）也是工作区级偏好
            outline_placement: Some("side".into()),
            auto_outline_on_select: Some(true),
            ..Default::default()
        })
        .expect("更新设置");
    assert_eq!(updated.sidebar_width, 301);
    assert_eq!(updated.list_width, 432);
    assert!((updated.editor_split_ratio - 0.512).abs() < 1e-6);
    assert_eq!(updated.editor_mode, "split");
    assert_eq!(updated.list_sort, "manual", "「自定义顺序」应被记住");
    assert_eq!(updated.outline_placement, "side", "大纲位置应被记住");
    assert!(updated.auto_outline_on_select, "「选中后自动显示大纲」应被记住");

    // 未知的**枚举类**设置同样不落库：主题 / 新建位置 / 编辑器模式都只接受已知值。
    // 判据不是「看起来像不像」，而是「界面拿着它查表能不能查到」。
    let rejected_enums = service
        .update_settings(UpdateSettingsInput {
            editor_theme: Some("solarized".into()),
            default_new_location: Some("somewhere".into()),
            editor_mode: Some("SPLIT".into()), // 大小写不同也算未知值
            outline_placement: Some("top".into()), // 只认 list / side
            ..Default::default()
        })
        .expect("未知枚举值应被忽略而不是报错");
    assert_eq!(rejected_enums.editor_theme, "light", "主题保持上一个有效值");
    assert_eq!(rejected_enums.default_new_location, "inbox");
    assert_eq!(rejected_enums.editor_mode, "split", "编辑器模式保持上一个有效值");
    assert_eq!(rejected_enums.outline_placement, "side", "大纲位置保持上一个有效值");

    // 未知排序值不落库（避免脏数据写进设置）
    let rejected = service
        .update_settings(UpdateSettingsInput {
            list_sort: Some("bogus".into()),
            ..Default::default()
        })
        .expect("未知排序应被忽略而不是报错");
    assert_eq!(rejected.list_sort, "manual");

    // 越界的分屏比例应被夹取
    let clamped = service
        .update_settings(UpdateSettingsInput {
            editor_split_ratio: Some(0.95),
            ..Default::default()
        })
        .expect("越界值应被夹取");
    assert!((clamped.editor_split_ratio - 0.7).abs() < 1e-6);

    // 重新打开工作区后应恢复布局设置
    service.close().expect("关闭工作区");
    let reopened = WorkspaceService::new(Arc::clone(&harness.app))
        .open(&harness.root.to_string_lossy())
        .expect("重新打开工作区");
    assert_eq!(reopened.settings.sidebar_width, 301);
    assert_eq!(reopened.settings.list_width, 432);
    assert_eq!(reopened.settings.editor_mode, "split");
    assert_eq!(reopened.settings.list_sort, "manual", "重开工作区后排序偏好保留");
    assert_eq!(reopened.settings.outline_placement, "side", "重开工作区后大纲位置保留");
    assert!(reopened.settings.auto_outline_on_select, "重开工作区后自动显示开关保留");
}

#[test]
fn applies_note_links_migration() {
    let harness = Harness::new();
    harness.create_workspace();
    let context = harness.context();

    // 006 = note_links / note_assets / note_file_stamp / search_rowid_map，
    // 007 = notes.parent_id 子文档层级，008 = notes.sort_order 手动排序，009 = notes.pinned 置顶，
    // 010 = notes.declared_parent，011 = note_chunks（N5 分块索引），
    // 012 = decision_cache（D2 决策缓存）
    assert_eq!(context.db.schema_version().expect("schema 版本"), 13);

    // 004：来源/时效列与类型化关系表
    let (has_relations, source_column) = context
        .db
        .with_conn(|conn| {
            let table: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='note_relations'",
                [],
                |row| row.get(0),
            )?;
            let column: i64 = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name='source_reliability'",
                [],
                |row| row.get(0),
            )?;
            Ok((table, column))
        })
        .expect("检查 004 迁移结果");
    assert_eq!(has_relations, 1, "note_relations 表应存在");
    assert_eq!(source_column, 1, "notes.source_reliability 列应存在");

    // 005：文件快照列（扫描短路用）
    let stamp_columns: i64 = context
        .db
        .with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name IN ('file_size','file_mtime')",
                [],
                |row| row.get(0),
            )?;
            Ok(count)
        })
        .expect("检查 005 迁移结果");
    assert_eq!(stamp_columns, 2, "notes 应有 file_size / file_mtime 列");

    // 006：FTS rowid 映射表（避免按 UNINDEXED 的 note_id 做全表删除）
    let map_table: i64 = context
        .db
        .with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='search_documents_map'",
                [],
                |row| row.get(0),
            )?;
            Ok(count)
        })
        .expect("检查 006 迁移结果");
    assert_eq!(map_table, 1, "search_documents_map 表应存在");

    let (table, indexes) = context
        .db
        .with_conn(|conn| {
            let table: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='note_links'",
                [],
                |row| row.get(0),
            )?;
            let indexes: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name LIKE 'idx_note_links_%'",
                [],
                |row| row.get(0),
            )?;
            Ok((table, indexes))
        })
        .expect("查询 schema");

    assert_eq!(table, 1, "note_links 表应随迁移 003 建立");
    assert_eq!(indexes, 4, "note_links 的四个索引都应建立");
}

#[test]
fn stats_report_words_recent_updates_and_pins() {
    let harness = Harness::new();
    harness.create_workspace();
    let notes = harness.notes();

    // 工作区预置了欢迎笔记：先记下基线
    let baseline = WorkspaceService::new(Arc::clone(&harness.app))
        .stats()
        .expect("统计");
    assert!(baseline.note_count >= 1);
    assert!(baseline.total_words > 0, "欢迎笔记也应计入字数");

    for (title, body) in [("甲", "一二三"), ("乙", "四五六七")] {
        notes
            .create(CreateNoteInput {
                title: title.into(),
                category_id: None,
                initial_content: Some(body.into()),
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记");
    }
    let pinned = notes
        .create(CreateNoteInput {
            title: "置顶笔记".into(),
            category_id: None,
            initial_content: Some("内容".into()),
            tags: None,
            parent_id: None,
        })
        .expect("创建笔记");
    notes.set_pinned(&pinned.note.id, true).expect("置顶");

    let stats = WorkspaceService::new(Arc::clone(&harness.app))
        .stats()
        .expect("统计");
    assert_eq!(stats.note_count, baseline.note_count + 3);
    assert!(stats.total_words > baseline.total_words, "字数应累计");
    // 刚创建的笔记都算「近期更新」
    assert_eq!(stats.updated_last_7d, stats.note_count);
    assert_eq!(stats.updated_last_30d, stats.note_count);
    assert_eq!(stats.pinned_count, 1);

    // 写作活动（近 14 天）：刚创建的笔记应落在今天这一格
    assert!(!stats.activity.is_empty(), "应有活跃度数据");
    let today = &stats.activity[stats.activity.len() - 1];
    assert!(today.count >= 3, "今天应有至少 3 篇更新：{today:?}");
    assert!(stats.activity.iter().all(|day| day.count > 0), "只返回有更新的日子");
}
