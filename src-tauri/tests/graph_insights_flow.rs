use std::sync::Arc;

use personal_knowledge_os_lib::application::graph_insights::GraphInsightsService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::graph::GraphInsightsQuery;
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
            .create("Graph 洞察测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self {
            _data: data,
            _workspace: workspace,
            app,
        }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("工作区已打开"))
    }

    fn insights(&self) -> GraphInsightsService {
        GraphInsightsService::new(self.app.context().expect("工作区已打开"))
    }

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
fn returns_explainable_neighbors_and_read_only_duplicate_previews() {
    let fixture = Fixture::new();
    let focus = fixture.note(
        "索引设计",
        "# 索引设计\n\n全文检索使用倒排索引和分块。参考 [[检索优化]]。",
    );
    let neighbor = fixture.note(
        "检索优化",
        "# 检索优化\n\n全文检索需要倒排索引和分块策略，兼顾召回与延迟。",
    );
    let duplicate = fixture.note(
        "索引设计旧版",
        "# 索引设计\n\n全文检索使用倒排索引和分块。参考 [[检索优化]]。",
    );
    let orphan = fixture.note("孤立想法", "# 孤立想法\n\n尚未建立任何连接。");

    let before = fixture.notes().get(&focus).expect("读取焦点笔记").content;
    let report = fixture
        .insights()
        .insights(GraphInsightsQuery {
            focus_note_id: Some(focus.clone()),
            duplicate_threshold: Some(0.6),
            ..Default::default()
        })
        .expect("生成洞察");

    let nearest = report
        .neighbors
        .iter()
        .find(|item| item.note_id == neighbor)
        .expect("直接引用且主题相近的笔记应进入近邻");
    assert!(!nearest.evidence.is_empty(), "近邻必须至少有一条原因");
    assert!(
        nearest
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "reference"),
        "直接引用必须作为证据：{:?}",
        nearest.evidence
    );
    assert!(
        nearest
            .evidence
            .iter()
            .any(|evidence| evidence.kind == "sharedTopic"),
        "正文共同主题必须作为证据：{:?}",
        nearest.evidence
    );

    let cluster = report
        .duplicate_clusters
        .iter()
        .find(|cluster| {
            cluster
                .members
                .iter()
                .any(|member| member.note_id == duplicate)
                && cluster.members.iter().any(|member| member.note_id == focus)
        })
        .expect("相同正文应生成近重复预览");
    assert!(cluster.requires_confirmation, "重复合并不能默认静默执行");
    assert!(cluster
        .members
        .iter()
        .all(|member| !member.excerpt.is_empty()));
    assert!(report.orphan_notes.iter().any(|note| note.id == orphan));

    let after = fixture
        .notes()
        .get(&focus)
        .expect("再次读取焦点笔记")
        .content;
    assert_eq!(before, after, "洞察必须是只读计算");
}

#[test]
fn twenty_near_duplicate_pairs_separate_opposite_conclusions() {
    let fixture = Fixture::new();
    let mut exact_pairs = Vec::new();
    let mut opposite_pairs = Vec::new();
    for index in 0..10 {
        let marker = format!("证据标记{index:02}");
        let enable_body =
            format!("# 缓存策略 {index:02}\n\n建议启用写入缓存，{marker}，以提升吞吐并降低延迟。");
        let enable = fixture.note(&format!("缓存策略 {index:02}"), &enable_body);
        let copy = fixture.note(&format!("缓存策略 {index:02} 副本"), &enable_body);
        let reject = fixture.note(
            &format!("缓存策略 {index:02} 反例"),
            &format!("# 缓存策略 {index:02}\n\n不建议启用写入缓存，{marker}，因为可能丢失数据。"),
        );
        exact_pairs.push((enable.clone(), copy));
        opposite_pairs.push((enable, reject));
    }

    let report = fixture
        .insights()
        .insights(GraphInsightsQuery {
            focus_note_id: exact_pairs.first().map(|pair| pair.0.clone()),
            duplicate_threshold: Some(0.6),
            ..Default::default()
        })
        .expect("生成洞察");

    let member_ids: Vec<std::collections::HashSet<&str>> = report
        .duplicate_clusters
        .iter()
        .map(|cluster| {
            cluster
                .members
                .iter()
                .map(|item| item.note_id.as_str())
                .collect()
        })
        .collect();
    for (left, right) in exact_pairs {
        assert!(
            member_ids
                .iter()
                .any(|members| members.contains(left.as_str()) && members.contains(right.as_str())),
            "相同正文必须进入同一近重复簇：{left} / {right}"
        );
    }
    for (enable, reject) in opposite_pairs {
        assert!(
            !member_ids
                .iter()
                .any(|members| members.contains(enable.as_str())
                    && members.contains(reject.as_str())),
            "相反结论不能被合并为近重复：{enable} / {reject}"
        );
    }
}
