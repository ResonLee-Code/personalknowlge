//! Ask M1 集成测试：作用域、证据不足、精确引用区间与默认零外发。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use personal_knowledge_os_lib::application::ai::ask::AskService;
use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::category_service::CategoryService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::tag_service::TagService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::{
    AskHistoryTurn, AskQuestionInput, AskSaveInput, AskScope, UpdateAiPolicyInput,
};
use personal_knowledge_os_lib::domain::category::CreateCategoryInput;
use personal_knowledge_os_lib::domain::note::{
    CreateNoteInput, NoteListQuery, SaveNoteInput, SetSourceInput,
};
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
            .create("Ask 测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self {
            _data: data,
            _workspace: workspace,
            app,
        }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn enable_ai(&self) {
        AiPolicyService::new(self.context())
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                ..Default::default()
            })
            .expect("打开 AI 总开关");
    }

    fn note(&self, title: &str, content: &str) -> String {
        self.note_in_scope(title, content, None, None)
    }

    fn note_in_scope(
        &self,
        title: &str,
        content: &str,
        category_id: Option<String>,
        tags: Option<Vec<String>>,
    ) -> String {
        NoteService::new(self.context())
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id,
                initial_content: Some(content.to_string()),
                tags,
                parent_id: None,
            })
            .expect("新建笔记")
            .note
            .id
    }

    fn category(&self, name: &str) -> String {
        CategoryService::new(self.context())
            .create(CreateCategoryInput {
                name: name.to_string(),
                parent_id: None,
                create_directory: None,
            })
            .expect("创建分类")
            .id
    }

    fn tag(&self, name: &str) -> String {
        TagService::new(self.context())
            .get_or_create(name)
            .expect("创建标签")
            .id
    }

    fn ask(&self) -> AskService {
        AskService::new(self.context())
    }

    fn configure_local_model(&self, endpoint: &str) {
        AiPolicyService::new(self.context())
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                local_model_endpoint: Some(endpoint.to_string()),
                local_model_name: Some("stub:latest".to_string()),
                local_model_compose: Some(true),
                ..Default::default()
            })
            .expect("配置本地模型");
    }
}

fn spawn_model_server(reply: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定模型端口");
    let port = listener.local_addr().expect("模型端口").port();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = handle_model_request(&mut stream, reply);
            let _ = counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    (format!("http://localhost:{port}"), calls)
}

fn handle_model_request(stream: &mut TcpStream, reply: &str) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header == "\r\n" {
            break;
        }
        if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;
    let payload = serde_json::json!({ "model": "stub:latest", "response": reply }).to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn request(question: &str, scope: AskScope, scope_id: Option<String>) -> AskQuestionInput {
    AskQuestionInput {
        question: question.to_string(),
        scope,
        scope_id,
        ..Default::default()
    }
}

fn count_notes(fixture: &Fixture) -> i64 {
    NoteService::new(fixture.context())
        .list(NoteListQuery {
            page_size: Some(500),
            ..Default::default()
        })
        .expect("笔记列表")
        .total
}

#[test]
fn ask_is_gated_and_never_sends_when_remote_is_not_authorized() {
    let fixture = Fixture::new();
    let note = fixture.note(
        "SQLite 索引设计",
        "# FTS5\n\nSQLite FTS5 使用倒排索引，并对中文采用双字短语。",
    );
    let service = fixture.ask();

    let disabled = service
        .answer(request("SQLite 索引有什么结论？", AskScope::All, None))
        .expect_err("AI 关闭时必须拒绝");
    assert_eq!(disabled.code.as_str(), "AI_DISABLED");

    fixture.enable_ai();
    AiPolicyService::new(fixture.context())
        .update(UpdateAiPolicyInput {
            remote_model_base_url: Some("https://example.invalid/v1".to_string()),
            remote_model_name: Some("test-model".to_string()),
            remote_model_compose: Some(true),
            ..Default::default()
        })
        .expect("配置在线模型但不授权");

    let answer = service
        .answer(request(
            "这篇笔记关于 SQLite 索引说了什么？",
            AskScope::Note,
            Some(note.clone()),
        ))
        .expect("未授权时仍应模板降级");
    assert_eq!(answer.tier, "template", "未授权在线档不能参与");
    assert_eq!(answer.provider, "template.ask");
    assert!(answer.evidence_sufficient);
    assert!(!answer.citations.is_empty());
    assert_eq!(answer.citations[0].note_id, note, "引用必须带来源笔记");
    let audit = AiPolicyService::new(fixture.context())
        .audit_tail(20)
        .expect("读取审计");
    assert!(
        audit.iter().all(|entry| !entry.outbound),
        "未授权的在线模型不能产生任何外发记录：{audit:?}"
    );
}

#[test]
fn note_scope_returns_paragraph_anchors_that_slice_back_to_the_original_body() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let content =
        "# FTS5\n\nSQLite FTS5 使用倒排索引，并对中文采用双字短语。\n\n## 查询\n\n标题命中优先。";
    let note = fixture.note("SQLite 索引设计", content);

    let answer = fixture
        .ask()
        .answer(request(
            "这篇笔记关于 SQLite 索引说了什么？",
            AskScope::Note,
            Some(note.clone()),
        ))
        .expect("当前笔记问答");

    assert!(answer.evidence_sufficient);
    assert!(answer.retrieved_count >= 1);
    assert_eq!(answer.channels[0].id, "bm25");
    assert!(
        answer.text.contains("[1]"),
        "答案必须带可解析引用：{}",
        answer.text
    );
    let chars: Vec<char> = content.chars().collect();
    for citation in &answer.citations {
        assert_eq!(citation.note_id, note);
        assert!(citation.char_start >= 0 && citation.char_end > citation.char_start);
        let start = citation.char_start as usize;
        let end = citation.char_end as usize;
        assert!(end <= chars.len());
        let source = chars[start..end].iter().collect::<String>();
        assert_eq!(source.trim(), citation.snippet);
        assert!(!citation.heading.is_empty(), "引用应带标题路径");
    }
}

#[test]
fn note_scope_finds_a_late_section_instead_of_only_reading_the_first_chunks() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let mut content = String::new();
    for index in 1..=12 {
        if index == 12 {
            content.push_str(
                "# 实验记录\n\n量子纠缠实验的结论是：当前样本只支持相关性观察，不能证明因果关系。",
            );
        } else {
            content.push_str(&format!(
                "# 第 {index} 节\n\n这是普通的背景记录，与后面的实验结论无关。\n\n"
            ));
        }
    }
    let note = fixture.note("长笔记", &content);

    let answer = fixture
        .ask()
        .answer(request(
            "量子纠缠实验的结论是什么？",
            AskScope::Note,
            Some(note),
        ))
        .expect("长笔记后段问答");
    assert!(answer.evidence_sufficient);
    assert!(
        answer
            .citations
            .iter()
            .any(|citation| citation.snippet.contains("量子纠缠实验的结论")),
        "应定位到后段而不是只取前 8 块：{:?}",
        answer.citations
    );
    assert!(
        answer
            .citations
            .iter()
            .any(|citation| citation.chunk_index >= 8),
        "引用应来自靠后的分块：{:?}",
        answer.citations
    );
}

#[test]
fn all_scope_uses_query_expansion_and_returns_insufficient_for_missing_material() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    fixture.note(
        "SQLite 全文检索结论",
        "# 结论\n\nSQLite FTS5 适合本地全文检索，中文需要双字短语扩展。",
    );
    fixture.note(
        "Graph 布局记录",
        "# 图谱\n\n力导向布局需要控制节点数量和弹簧参数。",
    );

    let matched = fixture
        .ask()
        .answer(request(
            "SQLite FTS5 全文检索有什么结论？",
            AskScope::All,
            None,
        ))
        .expect("全库问答");
    assert!(matched.evidence_sufficient);
    assert!(!matched.citations.is_empty());
    assert!(matched.citations.iter().any(|citation| {
        citation.title.contains("SQLite") && citation.snippet.contains("FTS5")
    }));

    let chinese = fixture
        .ask()
        .answer(request(
            "之前关于全文检索的结论有哪些？",
            AskScope::All,
            None,
        ))
        .expect("纯中文问句应通过中文双字词扩展命中");
    assert!(chinese.evidence_sufficient, "{}", chinese.evidence_note);
    assert!(chinese
        .citations
        .iter()
        .any(|citation| citation.snippet.contains("全文检索")));

    let missing = fixture
        .ask()
        .answer(request(
            "库里有没有关于量子纠缠实验的结论？",
            AskScope::All,
            None,
        ))
        .expect("没有材料也应正常返回");
    assert!(!missing.evidence_sufficient);
    assert!(missing.citations.is_empty());
    assert!(missing.text.contains("证据不足"));
}

#[test]
fn ten_queries_cover_note_category_tag_and_all_scopes() {
    use std::collections::HashSet;

    let fixture = Fixture::new();
    fixture.enable_ai();
    let cat_a = fixture.category("作用域 A");
    let cat_b = fixture.category("作用域 B");
    let cat_c = fixture.category("作用域 C");
    let cat_d = fixture.category("作用域 D");
    let alpha = fixture.tag("scope-alpha");
    let beta = fixture.tag("scope-beta");

    let notes = [
        fixture.note_in_scope(
            "范围 0",
            "# 范围 0\n\nscopeproof0 属于分类 A。",
            Some(cat_a.clone()),
            Some(vec!["scope-alpha".into()]),
        ),
        fixture.note_in_scope(
            "范围 1",
            "# 范围 1\n\nscopeproof1 属于分类 A。",
            Some(cat_a.clone()),
            Some(vec!["scope-alpha".into()]),
        ),
        fixture.note_in_scope(
            "范围 2",
            "# 范围 2\n\nscopeproof2 属于分类 A。",
            Some(cat_a.clone()),
            Some(vec!["scope-beta".into()]),
        ),
        fixture.note_in_scope(
            "范围 3",
            "# 范围 3\n\nscopeproof3 属于分类 B。",
            Some(cat_b.clone()),
            Some(vec!["scope-beta".into()]),
        ),
        fixture.note_in_scope(
            "范围 4",
            "# 范围 4\n\nscopeproof4 属于分类 B。",
            Some(cat_b.clone()),
            None,
        ),
        fixture.note_in_scope(
            "范围 5",
            "# 范围 5\n\nscopeproof5 属于分类 C。",
            Some(cat_c.clone()),
            Some(vec!["scope-alpha".into()]),
        ),
        fixture.note_in_scope(
            "范围 6",
            "# 范围 6\n\nscopeproof6 属于分类 C。",
            Some(cat_c.clone()),
            None,
        ),
        fixture.note_in_scope(
            "范围 7",
            "# 范围 7\n\nscopeproof7 属于分类 D。",
            Some(cat_d),
            Some(vec!["scope-beta".into()]),
        ),
        fixture.note_in_scope(
            "范围 8",
            "# 范围 8\n\nscopeproof8 只在标签 alpha。",
            None,
            Some(vec!["scope-alpha".into()]),
        ),
        fixture.note_in_scope("范围 9", "# 范围 9\n\nscopeproof9 只在全库。", None, None),
    ];
    let service = fixture.ask();
    let cases = [
        ("scopeproof0", AskScope::Note, notes[0].clone(), vec![notes[0].as_str()]),
        ("scopeproof5", AskScope::Note, notes[5].clone(), vec![notes[5].as_str()]),
        (
            "scopeproof2",
            AskScope::Category,
            cat_a,
            vec![notes[0].as_str(), notes[1].as_str(), notes[2].as_str()],
        ),
        (
            "scopeproof4",
            AskScope::Category,
            cat_b,
            vec![notes[3].as_str(), notes[4].as_str()],
        ),
        (
            "scopeproof6",
            AskScope::Category,
            cat_c,
            vec![notes[5].as_str(), notes[6].as_str()],
        ),
        (
            "scopeproof8",
            AskScope::Tag,
            alpha,
            vec![notes[0].as_str(), notes[1].as_str(), notes[5].as_str(), notes[8].as_str()],
        ),
        (
            "scopeproof3",
            AskScope::Tag,
            beta,
            vec![notes[2].as_str(), notes[3].as_str(), notes[7].as_str()],
        ),
        (
            "scopeproof9",
            AskScope::All,
            String::new(),
            notes.iter().map(String::as_str).collect(),
        ),
        (
            "scopeproof1",
            AskScope::Note,
            notes[1].clone(),
            vec![notes[1].as_str()],
        ),
        (
            "scopeproof7",
            AskScope::All,
            String::new(),
            notes.iter().map(String::as_str).collect(),
        ),
    ];

    for (keyword, scope, scope_id, allowed) in cases {
        let answer = service
            .answer(request(
                &format!("{keyword} 有什么结论？"),
                scope,
                if matches!(scope, AskScope::All) {
                    None
                } else {
                    Some(scope_id)
                },
            ))
            .expect("作用域问答");
        assert!(answer.evidence_sufficient, "{keyword}: {}", answer.evidence_note);
        assert!(!answer.citations.is_empty(), "{keyword} 应有引用");
        let allowed: HashSet<&str> = allowed.into_iter().collect();
        assert!(
            answer
                .citations
                .iter()
                .all(|citation| allowed.contains(citation.note_id.as_str())),
            "{keyword} 返回了作用域外引用：{:?}",
            answer.citations.iter().map(|item| &item.note_id).collect::<Vec<_>>()
        );
    }
}

#[test]
fn five_follow_ups_never_treat_prior_answers_as_facts() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let service = fixture.ask();

    for index in 0..5 {
        let marker = format!("GHOST_FACT_{index}");
        let mut input = request(&format!("{marker} 是什么？"), AskScope::All, None);
        input.history = vec![AskHistoryTurn {
            question: "上一轮说了什么？".to_string(),
            answer: format!("{marker} 已经被确认是事实。"),
        }];
        let answer = service.answer(input).expect("追问应正常返回");
        assert!(!answer.evidence_sufficient, "{marker} 没有本轮证据");
        assert!(answer.citations.is_empty(), "{marker} 不得把历史回答当引用");
        assert!(answer.text.contains("证据不足"), "{}", answer.text);
        assert!(
            !answer.text.contains(&marker),
            "上一轮答案里的虚构事实不得进入本轮结论：{}",
            answer.text
        );
    }
}

#[test]
fn follow_up_uses_history_for_retrieval_but_still_requires_fresh_evidence() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    fixture.note("Tokio 运行时记录", "# 限制\n\n阻塞调用会拖垮调度线程。");

    let first = fixture
        .ask()
        .answer(request("Tokio 阻塞调用有什么影响？", AskScope::All, None))
        .expect("第一轮");
    assert!(first.evidence_sufficient);

    let without_history = fixture
        .ask()
        .answer(request("它会导致什么后果？", AskScope::All, None))
        .expect("无上下文追问");
    assert!(
        !without_history.evidence_sufficient,
        "没有上下文时本轮检索不应凭空命中"
    );

    let follow_up = fixture
        .ask()
        .answer(AskQuestionInput {
            history: vec![AskHistoryTurn {
                question: first.question.clone(),
                answer: first.text.clone(),
            }],
            ..request("它会导致什么后果？", AskScope::All, None)
        })
        .expect("带上下文追问");
    assert!(follow_up.evidence_sufficient, "{}", follow_up.evidence_note);
    assert!(!follow_up.citations.is_empty());
    assert!(follow_up
        .citations
        .iter()
        .any(|citation| citation.snippet.contains("阻塞调用")));
}

#[test]
fn selected_text_is_used_to_locate_evidence() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let note = fixture.note(
        "实验结论",
        "# 结果\n\n量子纠缠实验只支持相关性观察，不能证明因果关系。",
    );

    let answer = fixture
        .ask()
        .answer(AskQuestionInput {
            selection: Some("量子纠缠实验只支持相关性观察".to_string()),
            ..request("这段内容是否被过度解读？", AskScope::Note, Some(note))
        })
        .expect("选中文字提问");
    assert!(answer.evidence_sufficient);
    assert!(answer
        .citations
        .iter()
        .any(|citation| citation.snippet.contains("不能证明因果关系")));
}

#[test]
fn oversized_context_falls_back_to_template_without_calling_the_model() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let note = fixture.note(
        "SQLite 索引设计",
        "# 结论\n\nSQLite FTS5 使用倒排索引，并对中文采用双字短语。",
    );
    let (endpoint, calls) = spawn_model_server("不应被调用 [1]");
    fixture.configure_local_model(&endpoint);
    let history = (0..6)
        .map(|index| AskHistoryTurn {
            question: format!("第 {index} 轮：{}", "问".repeat(500)),
            answer: format!("{} [1]", "答".repeat(800)),
        })
        .collect();

    let answer = fixture
        .ask()
        .answer(AskQuestionInput {
            history,
            ..request("SQLite 索引有什么结论？", AskScope::Note, Some(note))
        })
        .expect("超预算时仍用模板回答");
    assert_eq!(answer.tier, "template");
    assert_eq!(calls.load(Ordering::SeqCst), 0, "超预算前不能调用模型");
    assert!(
        answer
            .warnings
            .iter()
            .any(|warning| warning.contains("输入上限")),
        "必须明确提示预算超限：{:?}",
        answer.warnings
    );
}

#[test]
fn ask_refuses_weak_only_and_stale_only_evidence() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let note = fixture.note(
        "SQLite 索引设计",
        "# 结论\n\nSQLite FTS5 使用倒排索引。",
    );
    let notes = NoteService::new(fixture.context());
    notes
        .set_source(SetSourceInput {
            id: note.clone(),
            source_type: Some("personal".to_string()),
            source_reliability: Some(1),
            ..Default::default()
        })
        .expect("标为不可用线索");
    let weak = fixture
        .ask()
        .answer(request(
            "SQLite 索引有什么结论？",
            AskScope::Note,
            Some(note.clone()),
        ))
        .expect("弱线索也正常返回");
    assert!(!weak.evidence_sufficient);
    assert!(weak.text.contains("不可用线索"));
    assert!(weak.citations.is_empty());

    notes
        .set_source(SetSourceInput {
            id: note.clone(),
            source_type: Some("official".to_string()),
            source_reliability: Some(5),
            valid_until: Some("2020-01-01".to_string()),
            ..Default::default()
        })
        .expect("标为过期");
    let stale = fixture
        .ask()
        .answer(request("SQLite 索引有什么结论？", AskScope::Note, Some(note)))
        .expect("过期材料正常返回");
    assert!(!stale.evidence_sufficient);
    assert!(stale.text.contains("过时"));
    assert!(stale.citations.is_empty());
}

#[test]
fn answer_note_requires_preview_confirmation_and_rechecks_source_freshness() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let source_id = fixture.note(
        "SQLite 索引设计",
        "# 结论\n\nSQLite FTS5 使用倒排索引，并对中文采用双字短语。",
    );
    let answer = fixture
        .ask()
        .answer(request(
            "SQLite 索引有什么结论？",
            AskScope::Note,
            Some(source_id.clone()),
        ))
        .expect("生成答案");
    let service = fixture.ask();
    let preview = service
        .preview_answer_note(AskSaveInput {
            answer: answer.clone(),
            title: Some("SQLite 索引问答".to_string()),
            ..Default::default()
        })
        .expect("生成保存预览");
    assert_eq!(preview.title, "SQLite 索引问答");
    assert!(preview.content.contains("## 来源"));
    assert!(preview.content.contains("[[SQLite 索引设计]]"));
    assert!(preview.content.contains("[1]"));

    let before = count_notes(&fixture);
    let blocked = service
        .save_answer_note(AskSaveInput {
            answer: answer.clone(),
            title: Some("SQLite 索引问答".to_string()),
            ..Default::default()
        })
        .expect_err("未确认不能写入");
    assert_eq!(blocked.code.as_str(), "INVALID_INPUT");
    assert_eq!(count_notes(&fixture), before, "未确认时文件数不能变化");

    let saved = service
        .save_answer_note(AskSaveInput {
            answer: answer.clone(),
            title: Some("SQLite 索引问答".to_string()),
            confirm: true,
            ..Default::default()
        })
        .expect("确认后保存");
    assert_eq!(count_notes(&fixture), before + 1);
    assert_eq!(saved.title, "SQLite 索引问答");
    assert!(saved.file_path.ends_with(".md"));

    let notes = NoteService::new(fixture.context());
    let source = notes.get(&source_id).expect("读取来源");
    notes
        .save(SaveNoteInput {
            id: source_id,
            content: "# 结论\n\n来源内容已经更新。".to_string(),
            base_content_hash: source.note.content_hash,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("修改来源");
    let stale = service
        .preview_answer_note(AskSaveInput {
            answer,
            ..Default::default()
        })
        .expect_err("来源变化后应拒绝旧答案");
    assert_eq!(stale.code.as_str(), "NOTE_CONFLICT");
    assert!(stale.message.contains("重新提问"));
}
