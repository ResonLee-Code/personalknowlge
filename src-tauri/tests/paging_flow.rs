//! 排序与分页的确定性（用户清单里的「检索 / 列表排序分页抖动治理」）。
//!
//! 同一秒内写入的笔记在排序键（`updated_at` / 相关度得分）上完全相同，
//! 没有次级键时 SQLite 的返回顺序不确定——表现就是分页「同一篇出现两次」
//! 或者「有一篇怎么翻都看不到」。排序键里已经追加了 `n.id`，这里用真实分页把它钉死。

use std::sync::Arc;

use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, NoteListQuery};
use personal_knowledge_os_lib::domain::search::SearchQuery;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

const TOTAL: usize = 30;
const PAGE: usize = 7;

struct Fixture {
    _data: TempDir,
    _workspace: TempDir,
    app: Arc<AppInner>,
}

impl Fixture {
    fn new() -> Self {
        let data = tempfile::tempdir().expect("应用数据目录");
        let workspace = tempfile::tempdir().expect("工作区目录");
        let app =
            AppInner::new(data.path().to_path_buf(), Arc::new(NoopEventSink)).expect("应用状态");
        WorkspaceService::new(Arc::clone(&app))
            .create("分页测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }
}

/// 建 30 篇标题与更新时间都「撞在一起」的笔记（同一秒内写入，排序键完全相同）。
fn seed(fixture: &Fixture) {
    let notes = NoteService::new(fixture.context());
    for index in 0..TOTAL {
        notes
            .create(CreateNoteInput {
                // 标题故意前缀相同、序号占两位，方便按标题核对顺序
                title: format!("分页笔记 {index:02}"),
                category_id: None,
                initial_content: Some(format!("分页关键词 第 {index} 篇")),
                tags: None,
                parent_id: None,
            })
            .expect("建笔记");
    }
}

fn list_page(fixture: &Fixture, page: i64, page_size: i64) -> Vec<String> {
    NoteService::new(fixture.context())
        .list(NoteListQuery {
            // 只取本用例建的笔记：工作区初始化会放一篇「欢迎使用…」
            keyword: Some("分页笔记".to_string()),
            page: Some(page),
            page_size: Some(page_size),
            ..Default::default()
        })
        .expect("列表")
        .items
        .into_iter()
        .map(|item| item.id)
        .collect()
}

fn search_page(fixture: &Fixture, page: i64, page_size: i64) -> Vec<String> {
    SearchService::new(fixture.context())
        .search(SearchQuery {
            keyword: "分页关键词".to_string(),
            page: Some(page),
            page_size: Some(page_size),
            ..Default::default()
        })
        .expect("检索")
        .hits
        .into_iter()
        .map(|hit| hit.note.id)
        .collect()
}

#[test]
fn note_list_paging_is_stable_and_covers_every_note() {
    let fixture = Fixture::new();
    seed(&fixture);

    let single_items = NoteService::new(fixture.context())
        .list(NoteListQuery {
            keyword: Some("分页笔记".to_string()),
            page: Some(1),
            page_size: Some(TOTAL as i64 + 5),
            ..Default::default()
        })
        .expect("列表")
        .items;
    let single: Vec<String> = single_items.iter().map(|item| item.id.clone()).collect();
    assert_eq!(single.len(), TOTAL, "应能一次取到全部 {TOTAL} 篇");

    // 顺序必须**显式**等于 (updated_at DESC, id DESC)：只断言「分页与一次性取回一致」是不够的
    // ——SQLite 在缺次级键时给的顺序也自洽，但会随查询计划漂移。这里把口径写死，
    // 去掉 `n.id` 次级键就会立刻失败。
    let mut expected: Vec<(String, String)> = single_items
        .iter()
        .map(|item| (item.updated_at.clone(), item.id.clone()))
        .collect();
    expected.sort_by(|left, right| right.0.cmp(&left.0).then(right.1.cmp(&left.1)));
    let expected_ids: Vec<String> = expected.into_iter().map(|(_, id)| id).collect();
    assert_eq!(
        single, expected_ids,
        "列表顺序必须是 (updated_at DESC, id DESC)：同日/同秒写入的笔记不能靠运气排序"
    );

    // 逐页翻：不重复、不遗漏、顺序与一次性取回一致
    let mut paged: Vec<String> = Vec::new();
    for page in 1..=(TOTAL / PAGE + 1) {
        paged.extend(list_page(&fixture, page as i64, PAGE as i64));
    }
    assert_eq!(paged.len(), TOTAL, "分页取回的条数要对得上：{}", paged.len());
    let mut unique = paged.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), TOTAL, "同一篇笔记不能出现在两页里");
    assert_eq!(paged, single, "分页顺序必须与一次性取回完全一致（没有抖动）");

    // 同一页取两次，结果必须完全一样（SQLite 在没有次级键时给不出这个保证）
    assert_eq!(
        list_page(&fixture, 3, PAGE as i64),
        list_page(&fixture, 3, PAGE as i64),
        "同一页重复请求必须得到同样的顺序"
    );
}

#[test]
fn search_paging_is_stable_and_covers_every_hit() {
    let fixture = Fixture::new();
    seed(&fixture);

    let single_hits = SearchService::new(fixture.context())
        .search(SearchQuery {
            keyword: "分页关键词".to_string(),
            page: Some(1),
            page_size: Some(TOTAL as i64 + 5),
            ..Default::default()
        })
        .expect("检索")
        .hits;
    let single: Vec<String> = single_hits.iter().map(|hit| hit.note.id.clone()).collect();
    assert_eq!(single.len(), TOTAL, "30 篇都含关键词");

    let mut expected: Vec<(f64, String, String)> = single_hits
        .iter()
        .map(|hit| (hit.score, hit.note.updated_at.clone(), hit.note.id.clone()))
        .collect();
    expected.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(right.1.cmp(&left.1))
            .then(right.2.cmp(&left.2))
    });
    let expected_ids: Vec<String> = expected.into_iter().map(|(_, _, id)| id).collect();
    assert_eq!(
        single, expected_ids,
        "检索顺序必须是 (得分 ASC, updated_at DESC, id DESC)"
    );

    let mut paged: Vec<String> = Vec::new();
    for page in 1..=(TOTAL / PAGE + 1) {
        paged.extend(search_page(&fixture, page as i64, PAGE as i64));
    }
    assert_eq!(paged.len(), TOTAL, "检索分页取回的条数要对得上：{}", paged.len());
    let mut unique = paged.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), TOTAL, "同一篇命中的笔记不能出现在两页里");
    assert_eq!(paged, single, "检索分页顺序必须与一次性取回一致（没有抖动）");
    assert_eq!(
        search_page(&fixture, 2, PAGE as i64),
        search_page(&fixture, 2, PAGE as i64),
        "同一页重复检索必须得到同样的顺序"
    );
}
