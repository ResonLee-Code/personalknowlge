use std::sync::Arc;

use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::writing::WritingService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::error::ErrorCode;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SaveNoteInput};
use personal_knowledge_os_lib::domain::writing::{
    WritingApplyInput, WritingMode, WritingRequest, WritingSegment, WritingSummarySaveInput,
};
use personal_knowledge_os_lib::infrastructure::credential_store;
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
            .create("写作测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        AiPolicyService::new(app.context().expect("上下文"))
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                ..Default::default()
            })
            .expect("开启 AI 以使用模板 / 本地降级");
        Self {
            _data: data,
            _workspace: workspace,
            app,
        }
    }

    fn notes(&self) -> NoteService {
        NoteService::new(self.app.context().expect("上下文"))
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("上下文")
    }

    fn writing(&self) -> WritingService {
        WritingService::new(self.app.context().expect("上下文"))
    }

    fn note(&self, title: &str, body: &str) -> String {
        self.notes()
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(body.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("创建笔记")
            .note
            .id
    }
}

#[test]
fn five_writing_modes_are_cited_and_apply_requires_confirmation() {
    let fixture = Fixture::new();
    let target = fixture.note(
        "写作原则",
        "# 写作原则\n\n本地优先是核心边界。引用必须能回到原文。所有生成内容先预览。",
    );
    let _source = fixture.note(
        "引用规范",
        "# 引用规范\n\n每一个生成段落都要附来源。引用必须能回到原文区间。",
    );
    let current = fixture.notes().get(&target).expect("读取目标").content;

    for mode in [
        WritingMode::Outline,
        WritingMode::Continue,
        WritingMode::Rewrite,
        WritingMode::Shorten,
        WritingMode::Expand,
    ] {
        let draft = fixture
            .writing()
            .draft(WritingRequest {
                note_id: target.clone(),
                mode: Some(mode),
                instruction: "保持事实边界，引用规范".to_string(),
                selection: Some("本地优先是核心边界。".to_string()),
                selection_start: Some(8),
                selection_end: Some(20),
                confirm_outbound: false,
            })
            .expect("生成草稿");
        assert!(!draft.segments.is_empty(), "{mode:?}");
        assert!(
            draft
                .segments
                .iter()
                .all(|segment| !segment.citations.is_empty()),
            "{mode:?} 的每个段落都必须有引用"
        );
        assert_eq!(
            fixture.notes().get(&target).expect("草稿只读").content,
            current,
            "生成阶段不得写文件"
        );
    }

    let draft = fixture
        .writing()
        .draft(WritingRequest {
            note_id: target.clone(),
            mode: Some(WritingMode::Outline),
            instruction: "整理为大纲".to_string(),
            confirm_outbound: false,
            ..Default::default()
        })
        .expect("生成待应用草稿");
    let base = draft.base_content_hash.clone();
    let apply_input = WritingApplyInput {
        note_id: target.clone(),
        base_content_hash: base.clone(),
        mode: WritingMode::Outline,
        target_start: draft.target_start,
        target_end: draft.target_end,
        accepted_segments: vec![draft.segments[0].clone()],
        confirm: false,
    };
    let preview = fixture
        .writing()
        .apply_preview(&apply_input)
        .expect("写前预览");
    assert!(preview.content.contains("写作来源"));
    assert_eq!(
        fixture.notes().get(&target).expect("预览只读").content,
        current,
        "apply_preview 不能写文件"
    );
    let rejected = fixture
        .writing()
        .apply(apply_input.clone())
        .expect_err("未确认必须拒绝");
    assert_eq!(rejected.code, ErrorCode::InvalidInput);

    let applied = fixture
        .writing()
        .apply(WritingApplyInput {
            confirm: true,
            ..apply_input
        })
        .expect("确认后写入");
    assert!(applied.detail.content.contains("写作来源"));
    assert!(applied.accepted_segments >= 1);

    let stale = WritingApplyInput {
        note_id: target,
        base_content_hash: base,
        mode: WritingMode::Rewrite,
        target_start: 0,
        target_end: 0,
        accepted_segments: vec![WritingSegment {
            id: "W1".to_string(),
            text: "过期草稿 [1]".to_string(),
            citations: draft.citations.clone(),
        }],
        confirm: true,
    };
    let conflict = fixture
        .writing()
        .apply_preview(&stale)
        .expect_err("目标笔记哈希变化后必须拒绝");
    assert_eq!(conflict.code, ErrorCode::NoteConflict);
}

#[test]
fn source_changed_after_generation_blocks_apply() {
    let fixture = Fixture::new();
    let target = fixture.note("扩写目标", "# 扩写目标\n\n需要解释引用的可追溯性。");
    let source = fixture.note(
        "来源事实",
        "# 来源事实\n\n可追溯引用必须保留原文区间与来源笔记标题。",
    );
    let draft = fixture
        .writing()
        .draft(WritingRequest {
            note_id: target.clone(),
            mode: Some(WritingMode::Expand),
            instruction: "可追溯引用 来源事实".to_string(),
            selection: Some("需要解释引用的可追溯性。".to_string()),
            selection_start: Some(8),
            selection_end: Some(22),
            confirm_outbound: false,
        })
        .expect("生成草稿");
    let source_citation = draft
        .citations
        .iter()
        .find(|citation| citation.note_id == source)
        .cloned()
        .expect("相关来源应成为可引用材料");
    fixture
        .notes()
        .save(SaveNoteInput {
            id: source,
            content: "# 来源事实\n\n来源已被改写，旧引用区间不再有效。".to_string(),
            base_content_hash: None,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("修改来源");
    let error = fixture
        .writing()
        .apply_preview(&WritingApplyInput {
            note_id: target,
            base_content_hash: draft.base_content_hash,
            mode: WritingMode::Expand,
            target_start: draft.target_start,
            target_end: draft.target_end,
            accepted_segments: vec![WritingSegment {
                id: "W1".to_string(),
                text: format!("扩写内容 [{}]", source_citation.citation_id),
                citations: vec![source_citation.clone()],
            }],
            confirm: true,
        })
        .expect_err("来源漂移必须阻止写入");
    assert_eq!(error.code, ErrorCode::NoteConflict);

    let summary_error = fixture
        .writing()
        .save_summary(WritingSummarySaveInput {
            title: "聚类摘要".to_string(),
            segments: vec![WritingSegment {
                id: "W1".to_string(),
                text: format!("摘要内容 [{}]", source_citation.citation_id),
                citations: vec![source_citation],
            }],
            confirm: true,
        })
        .expect_err("聚类摘要也必须复核来源");
    assert_eq!(summary_error.code, ErrorCode::NoteConflict);
}

/// 真实模型五模式冒烟：默认不运行、不联网。
///
/// ```text
/// PKOS_LIVE_REMOTE_KEY=sk-xxx \
/// cargo test --offline --test writing_flow live_models_cover_five_writing_modes \
///   -- --ignored --nocapture
/// ```
#[test]
#[ignore = "真机评测：需要 PKOS_LIVE_REMOTE_KEY；本地模型需 Ollama qwen2.5:7b"]
fn live_models_cover_five_writing_modes() {
    let fixture = Fixture::new();
    let target = fixture.note(
        "写作原则",
        "# 写作原则\n\n本地优先是核心边界，正文不能未经确认上传。\
         引用必须能回到原文区间和来源笔记。所有生成内容先预览，再逐段确认。\
         冲突时保留用户现有内容，不允许静默覆盖。",
    );
    fixture.note(
        "引用规范",
        "# 引用规范\n\n每个生成段落都要附来源。引用必须能回到原文区间。\
         来源变化后旧草稿失效，必须重新生成。",
    );

    let modes = [
        (
            WritingMode::Outline,
            "把写作原则整理成三层大纲，保留事实边界。",
        ),
        (
            WritingMode::Continue,
            "承接现有段落，继续说明预览和确认的关系。",
        ),
        (
            WritingMode::Rewrite,
            "在不改变事实含义的前提下改写，语气更简洁。",
        ),
        (
            WritingMode::Shorten,
            "压缩为两句话，只保留可由材料支持的信息。",
        ),
        (
            WritingMode::Expand,
            "扩写本地优先、引用和确认边界，不得增加新事实。",
        ),
    ];
    let mut evaluated = 0usize;

    if let Some(key) = std::env::var("PKOS_LIVE_REMOTE_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        let base = std::env::var("PKOS_LIVE_REMOTE_BASE")
            .unwrap_or_else(|_| "https://api.deepseek.com".into());
        let model =
            std::env::var("PKOS_LIVE_REMOTE_MODEL").unwrap_or_else(|_| "deepseek-flash".into());
        credential_store::set_remote_model_key(fixture._data.path(), Some(&key))
            .expect("写入临时在线模型 Key");
        AiPolicyService::new(fixture.context())
            .update(UpdateAiPolicyInput {
                remote_model_base_url: Some(base),
                remote_model_name: Some(model),
                remote_model_compose: Some(true),
                local_model_compose: Some(false),
                confirm_remote_model: Some(true),
                ..Default::default()
            })
            .expect("授权在线写作模型");
        run_live_writing_modes(&fixture, &target, &modes, "online", "online");
        evaluated += modes.len();
    } else {
        eprintln!("跳过在线模型：未设置 PKOS_LIVE_REMOTE_KEY");
    }

    if std::env::var("PKOS_LIVE_SKIP_LOCAL").as_deref() != Ok("1") {
        let endpoint = std::env::var("PKOS_LIVE_LOCAL_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:11434".into());
        let model = std::env::var("PKOS_LIVE_LOCAL_MODEL").unwrap_or_else(|_| "qwen2.5:7b".into());
        AiPolicyService::new(fixture.context())
            .update(UpdateAiPolicyInput {
                remote_model_compose: Some(false),
                local_model_endpoint: Some(endpoint),
                local_model_name: Some(model),
                local_model_compose: Some(true),
                ..Default::default()
            })
            .expect("配置本地写作模型");
        run_live_writing_modes(&fixture, &target, &modes, "local", "local");
        evaluated += modes.len();
    }

    assert!(evaluated > 0, "没有配置任何真实写作模型");
}

fn run_live_writing_modes(
    fixture: &Fixture,
    target: &str,
    modes: &[(WritingMode, &str)],
    provider_label: &str,
    expected_tier: &str,
) {
    for (mode, instruction) in modes {
        let started = std::time::Instant::now();
        let draft = fixture
            .writing()
            .draft(WritingRequest {
                note_id: target.to_string(),
                mode: Some(*mode),
                instruction: (*instruction).to_string(),
                confirm_outbound: true,
                ..Default::default()
            })
            .unwrap_or_else(|error| panic!("{provider_label} {mode:?} 生成失败：{error}"));
        assert_eq!(
            draft.tier, expected_tier,
            "{provider_label} {mode:?} 未使用目标模型，provider={} warnings={:?}",
            draft.provider, draft.warnings
        );
        assert!(
            draft
                .segments
                .iter()
                .all(|segment| !segment.citations.is_empty()),
            "{provider_label} {mode:?} 存在无引用段落"
        );
        println!(
            "PKOS_LIVE_WRITING provider={} mode={mode:?} tier={} segments={} citations={} chars={} took_ms={} wall_ms={}\n{}",
            draft.provider,
            draft.tier,
            draft.segments.len(),
            draft.citations.len(),
            draft.text.chars().count(),
            draft.took_ms,
            started.elapsed().as_millis(),
            draft.text
        );
    }
}

/// 真实 DeepSeek 已配置但未授权：Writing 必须零外发并降级模板。
#[test]
#[ignore = "真机抓包：需要 PKOS_LIVE_REMOTE_KEY；可设置 PKOS_LIVE_HOLD_MS"]
fn live_unauthorized_deepseek_writing_keeps_zero_outbound() {
    let Some(key) = std::env::var("PKOS_LIVE_REMOTE_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        eprintln!("跳过：未设置 PKOS_LIVE_REMOTE_KEY");
        return;
    };
    let base = std::env::var("PKOS_LIVE_REMOTE_BASE")
        .unwrap_or_else(|_| "https://api.deepseek.com".into());
    let model = std::env::var("PKOS_LIVE_REMOTE_MODEL").unwrap_or_else(|_| "deepseek-flash".into());
    let fixture = Fixture::new();
    let target = fixture.note(
        "未授权写作",
        "# 未授权写作\n\n本地优先，引用必须可追溯，写入前必须预览并确认。",
    );
    credential_store::set_remote_model_key(fixture._data.path(), Some(&key))
        .expect("写入临时在线模型 Key");
    AiPolicyService::new(fixture.context())
        .update(UpdateAiPolicyInput {
            enabled: Some(true),
            remote_model_base_url: Some(base),
            remote_model_name: Some(model),
            remote_model_compose: Some(true),
            ..Default::default()
        })
        .expect("保存在线模型配置");
    let policy = AiPolicyService::new(fixture.context()).policy();
    assert!(policy.remote_model_key_set);
    assert!(
        policy.remote_model_authorized_at.is_none(),
        "本测试故意不授权在线模型"
    );

    let ready_ms = std::env::var("PKOS_LIVE_CAPTURE_READY_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    println!(
        "PKOS_LIVE_WRITING_ZERO_OUTBOUND pid={} stage=before-draft capture_ready_ms={}",
        std::process::id(),
        ready_ms
    );
    if ready_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(ready_ms));
    }

    let draft = fixture
        .writing()
        .draft(WritingRequest {
            note_id: target,
            mode: Some(WritingMode::Outline),
            instruction: "整理为三层大纲".to_string(),
            confirm_outbound: true,
            ..Default::default()
        })
        .expect("未授权写作应降级模板而不是失败");
    assert_eq!(draft.tier, "template");
    assert!(!draft.segments.is_empty());
    let audit = serde_json::to_string(
        &AiPolicyService::new(fixture.context())
            .audit_tail(100)
            .expect("审计"),
    )
    .expect("审计 JSON");
    assert!(audit.contains("REMOTE_MODEL_UNCONFIRMED"));
    assert!(!audit.contains(&key));

    let hold_ms = std::env::var("PKOS_LIVE_HOLD_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    println!(
        "PKOS_LIVE_WRITING_ZERO_OUTBOUND pid={} stage=after-draft hold_ms={} result=template",
        std::process::id(),
        hold_ms
    );
    if hold_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(hold_ms));
    }
}
