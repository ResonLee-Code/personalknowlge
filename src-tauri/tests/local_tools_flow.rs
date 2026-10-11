//! 本地只读工具与快速切换（0 信任：把「不受闸门限制、也绝不外发」这个边界钉死）。
//!
//! `kb_search` / `note_read` 是**纯本地只读**工具（复用 FTS5 与文件读取），
//! 设计上不经过 AI 总开关——否则「AI 关着就连搜都不能搜」会很怪。
//! 但反过来必须保证：它们**永远不会**产生 `outbound=true` 的审计记录。

use std::sync::Arc;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::tools::AiTools;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::search_service::SearchService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
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
            .create("本地工具测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { _data: data, _workspace: workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn tools(&self) -> AiTools {
        AiTools::new(self.context())
    }

    fn note(&self, title: &str, content: &str) -> String {
        NoteService::new(self.context())
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(content.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("新建笔记")
            .note
            .id
    }
}

#[test]
fn local_read_tools_work_with_the_ai_switch_off_and_never_go_outbound() {
    let fixture = Fixture::new();
    let note_id = fixture.note("本地检索目标", "正文里写着工作窃取队列与批量唤醒。");
    // 默认口径：AI 总开关关闭
    assert!(!AiPolicyService::new(fixture.context()).policy().enabled);

    let hits = fixture.tools().kb_search("工作窃取", 5).expect("本地检索不应被闸门挡住");
    assert!(
        hits.iter().any(|hit| hit.note_id == note_id),
        "应当命中本地笔记：{hits:?}"
    );
    assert!(!hits[0].snippet.is_empty(), "命中要带片段");

    let (title, content) = fixture.tools().note_read(&note_id).expect("读笔记");
    assert_eq!(title, "本地检索目标");
    assert!(content.contains("工作窃取"));

    // 关键边界：只读工具绝不外发——审计里不允许出现 outbound=true
    let audit = AiPolicyService::new(fixture.context())
        .audit_tail(50)
        .expect("审计");
    assert!(
        audit.iter().all(|entry| !entry.outbound),
        "本地只读工具不得产生外发记录：{audit:?}"
    );
}

#[test]
fn kb_search_validates_keyword_and_clamps_limit() {
    let fixture = Fixture::new();
    fixture.note("第一条", "内容一");
    fixture.note("第二条", "内容二");

    let empty = fixture.tools().kb_search("   ", 5).expect_err("空关键词要报错");
    assert_eq!(empty.code.as_str(), "INVALID_INPUT");

    // limit 会被夹到 1–50：0 → 1，999 → 50（不报错、也不超发）
    assert_eq!(fixture.tools().kb_search("内容", 0).expect("limit=0").len(), 1);
    let wide = fixture.tools().kb_search("内容", 999).expect("limit=999");
    assert!(wide.len() <= 50, "limit 上限 50：{}", wide.len());
}

#[test]
fn quick_search_clamps_limit_and_skips_trashed_notes() {
    let fixture = Fixture::new();
    let keep = fixture.note("快速切换目标", "内容");
    let trashed = fixture.note("快速切换目标（将删除）", "内容");
    NoteService::new(fixture.context())
        .delete(&trashed)
        .expect("移入回收站");

    let search = SearchService::new(fixture.context());
    let hits = search.quick_search("快速切换目标", 10).expect("快速检索");
    assert!(hits.iter().any(|item| item.id == keep), "应命中活跃笔记");
    assert!(
        hits.iter().all(|item| item.id != trashed),
        "回收站里的笔记不应出现在快速切换里"
    );

    // limit 夹取：0 → 1
    assert_eq!(search.quick_search("快速切换目标", 0).expect("limit=0").len(), 1);
    assert!(search.quick_search("快速切换目标", 999).expect("limit=999").len() <= 30);

    // 列表接口同样看不到回收站里的笔记（与快速切换口径一致）
    let listed = NoteService::new(fixture.context())
        .list(NoteListQuery { page_size: Some(50), ..Default::default() })
        .expect("列表");
    assert!(listed.items.iter().all(|item| item.id != trashed));
}
