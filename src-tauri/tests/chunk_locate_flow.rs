//! 分块索引与引用定位（N5）集成测试。
//!
//! 验收口径：**检索命中能跳回原文的精确区间**。这里把它变成可证伪的断言：
//! 命中带的区间，按同样的口径切回正文，必须**正好是命中的那个词**，且落在对的分块上。

use std::sync::Arc;

use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::ai::retrieval::{retrieve, Bm25Retriever};
use personal_knowledge_os_lib::application::ai::traits::Retriever;
use personal_knowledge_os_lib::application::index_service::IndexService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::repositories::{ChunkRepository, SqliteChunkRepository};
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
            .create("分块测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    /// 新建一篇笔记，返回它的**文件路径**（用于按同一口径切正文验证区间）。
    fn note(&self, title: &str, body: &str) -> String {
        personal_knowledge_os_lib::application::note_service::NoteService::new(self.context())
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(body.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("新建笔记")
            .note
            .file_path
    }

    fn search(&self, keyword: &str) -> personal_knowledge_os_lib::domain::search::SearchResponse {
        SearchService::new(self.context())
            .search(SearchQuery { keyword: keyword.to_string(), ..Default::default() })
            .expect("搜索")
    }

    /// 按同一口径（`char` 下标）从正文里切出区间，用于验证区间是真的。
    fn slice_body(&self, file_path: &str, start: i64, end: i64) -> String {
        let body = self.context().storage.read_note(file_path).expect("读正文").body;
        body.chars()
            .skip(start.max(0) as usize)
            .take((end - start).max(0) as usize)
            .collect()
    }
}

#[test]
fn search_hit_carries_the_exact_source_range_and_heading() {
    let fixture = Fixture::new();
    fixture.note(
        "索引设计",
        "# 索引设计\n\n倒排索引与分词策略。\n\n## 查询策略\n\n按双字短语匹配，查询优化要注意召回率。\n",
    );

    let response = fixture.search("查询优化");
    let hit = response.hits.first().expect("应当有命中");
    let location = hit.location.as_ref().expect("命中必须带精确区间");

    assert_eq!(location.heading, "索引设计 > 查询策略", "区间要落在对的分块上");
    assert_eq!(
        fixture.slice_body(&hit.note.file_path, location.char_start, location.char_end),
        "查询优化",
        "区间切回正文必须正好是命中的词"
    );
}

#[test]
fn hits_before_any_heading_locate_into_the_preamble_chunk() {
    let fixture = Fixture::new();
    fixture.note("前言笔记", "开头就提到分块索引这件事。\n\n# 正文\n\n其余内容。\n");

    let response = fixture.search("分块索引");
    let hit = response.hits.first().expect("应当有命中");
    let location = hit.location.as_ref().expect("命中必须带精确区间");

    assert_eq!(location.heading, "", "标题之前的内容属于前言块");
    assert_eq!(
        fixture.slice_body(&hit.note.file_path, location.char_start, location.char_end),
        "分块索引"
    );
}

#[test]
fn location_points_at_the_first_matching_chunk_not_the_note_start() {
    let fixture = Fixture::new();
    // 命中的词只出现在**后面**的块里：区间不能偷懒地给整篇开头
    fixture.note(
        "长文",
        "# 第一段\n\n这里只有别的内容。\n\n# 第二段\n\n这里才提到稀缺关键词。\n",
    );

    let response = fixture.search("稀缺关键词");
    let hit = response.hits.first().expect("应当有命中");
    let location = hit.location.as_ref().expect("命中必须带精确区间");

    assert_eq!(location.heading, "第二段");
    assert!(location.char_start > 10, "区间应当指向后半段，而不是开头");
    assert_eq!(
        fixture.slice_body(&hit.note.file_path, location.char_start, location.char_end),
        "稀缺关键词"
    );
}

#[test]
fn index_rebuild_restores_chunks_from_files() {
    let fixture = Fixture::new();
    fixture.note("重建", "# 标题\n\n重建之后仍要能定位关键词。\n");
    assert!(fixture.search("关键词").hits[0].location.is_some());

    // 分块是**派生数据**：把它清掉（模拟"派生数据丢了"）再重建，必须能从文件恢复
    fixture
        .context()
        .db
        .with_conn(|conn| SqliteChunkRepository.clear(conn))
        .expect("清空分块");
    assert!(
        fixture.search("关键词").hits[0].location.is_none(),
        "清掉分块后应当定位不到（证明确实用的分块表，而不是别的捷径）"
    );

    let status = IndexService::new(fixture.context()).rebuild().expect("重建索引");
    assert!(status.indexed_notes >= 1, "重建应当索引到笔记：{status:?}");

    let response = fixture.search("关键词");
    let hit = response.hits.first().expect("重建后仍应有命中");
    let location = hit.location.as_ref().expect("重建后仍要能精确定位");
    assert_eq!(
        fixture.slice_body(&hit.note.file_path, location.char_start, location.char_end),
        "关键词"
    );
}

/// 升级路径：老库「有笔记索引、没有分块表」时，**重开工作区应当自动补齐**——
/// 否则用户升级后检索能用、但"跳回原文精确区间"会静默失效。
#[test]
fn reopening_a_workspace_rebuilds_missing_chunks() {
    let fixture = Fixture::new();
    fixture.note("升级", "# 标题\n\n老库升级后仍要能定位关键词。\n");

    // 模拟从旧版本升级上来：笔记索引在、分块表空
    fixture
        .context()
        .db
        .with_conn(|conn| SqliteChunkRepository.clear(conn))
        .expect("清空分块");
    let status = IndexService::new(fixture.context()).status().expect("索引状态");
    assert_eq!(
        status.state,
        personal_knowledge_os_lib::domain::search::IndexState::Stale,
        "缺分块要如实报「落后」，界面才会提示/触发重建"
    );

    let root = fixture.workspace.path().to_string_lossy().to_string();
    WorkspaceService::new(Arc::clone(&fixture.app)).close().expect("关闭工作区");
    WorkspaceService::new(Arc::clone(&fixture.app)).open(&root).expect("重新打开工作区");

    let response = fixture.search("关键词");
    let hit = response.hits.first().expect("重开后仍应有命中");
    assert!(hit.location.is_some(), "重开工作区应当自动补齐分块，精确定位恢复");
}

/// D3 / N7 第一步：BM25 通道把 FTS 包成 `Retriever`，并带上 D1 的**精确区间**与频道状态。
#[test]
fn bm25_retriever_returns_chunks_with_exact_ranges_and_channel_status() {
    let fixture = Fixture::new();
    let file_path = fixture.note(
        "索引设计",
        "# 索引设计\n\n倒排索引与分词策略。\n\n## 查询策略\n\n按双字短语匹配，查询优化要注意召回率。\n",
    );

    let retriever = Bm25Retriever::new(fixture.context());
    let providers: Vec<&dyn Retriever> = vec![&retriever];
    let outcome = retrieve(&providers, "查询优化", 10);

    assert_eq!(outcome.channels.len(), 1, "目前只有 bm25 一个通道");
    let channel = &outcome.channels[0];
    assert_eq!(channel.id, "bm25");
    assert!(channel.available, "关键词检索永远可用：{}", channel.note);

    let hit = outcome.hits.first().expect("应当有命中");
    assert_eq!(hit.providers, vec!["bm25".to_string()], "通道标注要如实");
    assert_eq!(
        hit.chunk.heading_path,
        vec!["索引设计".to_string(), "查询策略".to_string()],
        "融合结果要保留分块标题路径"
    );
    assert_eq!(
        fixture.slice_body(&file_path, hit.chunk.char_start, hit.chunk.char_end),
        "查询优化",
        "通道化之后仍要带**原文精确区间**"
    );
}
