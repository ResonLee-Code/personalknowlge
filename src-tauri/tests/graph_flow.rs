//! 知识图谱查询的集成测试（V1.1 M3）。

use std::sync::Arc;

use personal_knowledge_os_lib::application::graph_service::GraphService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::tag_service::TagService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::graph::GraphQuery;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SaveNoteInput};
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
            .create("图谱测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }

    fn graph(&self) -> GraphService {
        GraphService::new(self.app.context().expect("工作区已打开"))
    }

    fn tags(&self) -> TagService {
        TagService::new(self.app.context().expect("工作区已打开"))
    }

    /// 新建一篇笔记并写入正文（触发链接增量更新）。
    fn note(&self, title: &str, body: &str) -> String {
        let detail = self
            .notes()
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: None,
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记");
        self.notes()
            .save(SaveNoteInput {
                id: detail.note.id.clone(),
                content: body.to_string(),
                base_content_hash: None,
                title: None,
                category_id: None,
                tags: None,
            })
            .expect("保存笔记");
        detail.note.id
    }
}

#[test]
fn builds_nodes_and_edges_from_links() {
    let fixture = Fixture::new();
    let alpha = fixture.note("甲", "链接到 [[乙]] 和 [[丙]]");
    let beta = fixture.note("乙", "回链 [[甲]]");
    let gamma = fixture.note("丙", "");
    let orphan = fixture.note("孤立笔记", "没有任何链接");

    let graph = fixture.graph().graph(GraphQuery::default()).expect("查询图谱");
    // 欢迎笔记也在图中，只断言我们创建的节点
    let find = |id: &str| graph.nodes.iter().find(|node| node.id == id).expect("节点应存在");
    assert_eq!(find(&alpha).degree, 2, "甲的邻居是乙和丙");
    assert_eq!(find(&beta).degree, 1, "乙只连回甲");
    assert_eq!(find(&gamma).degree, 1);
    assert!(find(&orphan).orphan);
    assert!(graph.edges.iter().any(|edge| edge.source == alpha && edge.target == beta));
    assert!(graph.edges.iter().any(|edge| edge.source == beta && edge.target == alpha));
    assert!(graph.edges.iter().all(|edge| edge.source != edge.target), "自链接不入图");
    assert!(graph.nodes.iter().any(|node| node.id == orphan), "默认包含孤立节点");

    let without_orphans = fixture
        .graph()
        .graph(GraphQuery { include_orphans: Some(false), ..Default::default() })
        .expect("查询图谱");
    assert!(without_orphans.nodes.iter().all(|node| !node.orphan));
}

#[test]
fn focus_keeps_only_n_hop_neighbourhood() {
    let fixture = Fixture::new();
    let alpha = fixture.note("节点甲", "指向 [[节点乙]]");
    let beta = fixture.note("节点乙", "指向 [[节点丙]]");
    let gamma = fixture.note("节点丙", "");
    let far = fixture.note("远端节点", "");

    let one_hop = fixture
        .graph()
        .graph(GraphQuery { focus_note_id: Some(alpha.clone()), hops: Some(1), ..Default::default() })
        .expect("1 跳");
    // 顺序不承诺（取决于列表排序），只比较集合
    let mut ids: Vec<&str> = one_hop.nodes.iter().map(|node| node.id.as_str()).collect();
    ids.sort();
    let mut expected = vec![alpha.as_str(), beta.as_str()];
    expected.sort();
    assert_eq!(ids, expected);

    let two_hops = fixture
        .graph()
        .graph(GraphQuery { focus_note_id: Some(alpha.clone()), hops: Some(2), ..Default::default() })
        .expect("2 跳");
    let ids: Vec<&str> = two_hops.nodes.iter().map(|node| node.id.as_str()).collect();
    assert_eq!(ids.len(), 3);
    assert!(ids.contains(&gamma.as_str()));
    assert!(!ids.contains(&far.as_str()));
}

#[test]
fn filters_by_tag_and_keyword() {
    let fixture = Fixture::new();
    let alpha = fixture.note("标签笔记甲", "[[标签笔记乙]]");
    let beta = fixture.note("标签笔记乙", "");
    let tag = fixture.tags().get_or_create("图谱").expect("创建标签");
    fixture
        .tags()
        .set_note_tags(&alpha, &["图谱".to_string()])
        .expect("绑定标签");

    let by_tag = fixture
        .graph()
        .graph(GraphQuery { tag_id: Some(tag.id.clone()), ..Default::default() })
        .expect("按标签查询");
    assert_eq!(by_tag.nodes.len(), 1);
    assert_eq!(by_tag.nodes[0].id, alpha);
    assert!(by_tag.nodes[0].tags.contains(&"图谱".to_string()));
    // 目标不在筛选范围内 → 边被裁掉，节点退化为孤立
    assert!(by_tag.edges.is_empty());
    assert_eq!(by_tag.nodes[0].degree, 0);

    let by_keyword = fixture
        .graph()
        .graph(GraphQuery { keyword: Some("标签笔记乙".into()), ..Default::default() })
        .expect("按关键词查询");
    assert!(by_keyword.nodes.iter().any(|node| node.id == beta));
    assert!(!by_keyword.nodes.iter().any(|node| node.id == alpha));
}

#[test]
fn truncates_by_degree_and_reports_totals() {
    let fixture = Fixture::new();
    let hub = fixture.note("中心节点", "");
    for index in 0..6 {
        let leaf = fixture.note(&format!("叶子 {index}"), "指向 [[中心节点]]");
        let _ = leaf;
    }
    // 再补一篇完全孤立的笔记
    let _ = fixture.note("孤立叶子", "");

    let graph = fixture
        .graph()
        .graph(GraphQuery { max_nodes: Some(3), ..Default::default() })
        .expect("截断查询");
    assert!(graph.truncated);
    assert_eq!(graph.nodes.len(), 3);
    assert_eq!(graph.total_nodes, 9, "欢迎笔记 + 中心 + 6 叶子 + 孤立叶子");
    assert!(graph.nodes.iter().any(|node| node.id == hub), "度数最高的节点应保留");
    let hub_node = graph.nodes.iter().find(|node| node.id == hub).unwrap();
    // 截断后只保留 3 个节点 → 中心节点在结果里的邻居数是 2（度数按可见子图重算）
    assert_eq!(hub_node.degree, 2);
}

/// 大库抽样：连接优先 + 分类配额（V1.2.1 C2）。
///
/// 反例设定：分类 A 有 12 篇互相链接（度数高），B / C 各 2 篇低度数。
/// 旧策略「全局按度数取前 N」会把 B / C 全部丢掉（小分类在大库里直接消失）；
/// 新策略按分类配额抽样，且优先把种子节点的邻居一起带进来。
#[test]
fn samples_large_graph_with_category_quotas_and_connectivity() {
    use personal_knowledge_os_lib::application::category_service::CategoryService;
    use personal_knowledge_os_lib::domain::category::CreateCategoryInput;

    let fixture = Fixture::new();
    let context = fixture.app.context().expect("工作区已打开");
    let categories = CategoryService::new(Arc::clone(&context));
    let make_category = |name: &str| {
        categories
            .create(CreateCategoryInput {
                name: name.to_string(),
                parent_id: None,
                create_directory: None,
            })
            .expect("建分类")
            .id
    };
    let dense = make_category("密集分类");
    let small_b = make_category("小分类B");
    let small_c = make_category("小分类C");

    // A：12 篇围成环（每篇度数 2）
    let mut dense_ids = Vec::new();
    for index in 0..12 {
        let id = fixture.note(&format!("密集 {index}"), "");
        fixture
            .notes()
            .save(SaveNoteInput {
                id: id.clone(),
                content: format!("链接到 [[密集 {}]]", (index + 1) % 12),
                base_content_hash: None,
                title: None,
                category_id: Some(dense.clone()),
                tags: None,
            })
            .expect("保存");
        dense_ids.push(id);
    }
    // B / C：各 2 篇，只有分类内 1 条边
    for (index, category) in [(0, &small_b), (1, &small_b), (0, &small_c), (1, &small_c)] {
        let partner = 1 - index;
        let id = fixture.note(&format!("{}-{index}", category), "");
        fixture
            .notes()
            .save(SaveNoteInput {
                id: id.clone(),
                content: format!("[[{}-{partner}]]", category),
                base_content_hash: None,
                title: None,
                category_id: Some(category.to_string()),
                tags: None,
            })
            .expect("保存");
    }

    let graph = fixture
        .graph()
        .graph(GraphQuery {
            max_nodes: Some(8),
            max_edges: Some(50),
            ..Default::default()
        })
        .expect("查询图谱");

    assert!(graph.truncated, "17 篇里只取 8 个节点，应标记截断");
    assert_eq!(graph.nodes.len(), 8);
    assert_eq!(graph.total_nodes, 17, "欢迎笔记 + 12 + 4");

    let categories_in_sample: std::collections::HashSet<Option<String>> =
        graph.nodes.iter().map(|node| node.category_id.clone()).collect();
    for (label, expected) in [
        ("密集分类", Some(dense.clone())),
        ("小分类B", Some(small_b.clone())),
        ("小分类C", Some(small_c.clone())),
        ("未分类（欢迎笔记）", None),
    ] {
        assert!(
            categories_in_sample.contains(&expected),
            "{label} 在抽样后不应消失：{categories_in_sample:?}"
        );
    }

    // 连接优先：密集分类的节点之间应保留内部边
    let dense_edges = graph
        .edges
        .iter()
        .filter(|edge| {
            graph
                .nodes
                .iter()
                .any(|node| node.id == edge.source && node.category_id.as_deref() == Some(dense.as_str()))
                && graph
                    .nodes
                    .iter()
                    .any(|node| node.id == edge.target && node.category_id.as_deref() == Some(dense.as_str()))
        })
        .count();
    assert!(dense_edges > 0, "抽样应保留同一分类内部的连接：{:?}", graph.edges);
    assert!(
        graph.nodes.iter().any(|node| node.degree > 0),
        "样本里应有带连接的节点"
    );
}
