//! Organize P0：建议只读生成、确认应用、字节级撤销、冲突拦截与持久化批任务。

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

use personal_knowledge_os_lib::application::ai::embeddings::{
    EmbeddingProvider, RemoteEmbeddingProvider,
};
use personal_knowledge_os_lib::application::ai::organize::OrganizeService;
use personal_knowledge_os_lib::application::ai::policy::AiPolicyService;
use personal_knowledge_os_lib::application::ai::vector_index::VectorIndexService;
use personal_knowledge_os_lib::application::note_service::NoteService;
use personal_knowledge_os_lib::application::relation_service::RelationService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::core::hash::sha256_hex_str;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::{CreateNoteInput, SaveNoteInput};
use personal_knowledge_os_lib::domain::organize::{
    AiJobStatus, AiSuggestionPayload, SuggestionKind, SuggestionStatus,
};
use personal_knowledge_os_lib::infrastructure::credential_store;
use personal_knowledge_os_lib::repositories::ai_organize_repository::{
    AiOrganizeRepository, SqliteAiOrganizeRepository,
};
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

struct Fixture {
    data: TempDir,
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
            .create("Organize 测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self {
            data,
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

    fn organize(&self) -> OrganizeService {
        OrganizeService::new(self.context())
    }

    fn root(&self) -> String {
        self._workspace.path().to_string_lossy().to_string()
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

    fn configure_remote_embedding(&self, endpoint: &str) {
        credential_store::set_remote_model_key(self.data.path(), Some("sk-test"))
            .expect("写嵌入凭据");
        AiPolicyService::new(self.context())
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                remote_model_base_url: Some(endpoint.to_string()),
                remote_model_name: Some("chat-model".to_string()),
                remote_embedding_model: Some("fake-embed".to_string()),
                vector_enabled: Some(true),
                confirm_vector: Some(true),
                confirm_each_outbound: Some(true),
                ..Default::default()
            })
            .expect("配置远端嵌入");
    }
}

fn spawn_model_server(reply: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定端口");
    let port = listener.local_addr().expect("端口").port();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = handle_model_request(&mut stream, reply);
            let _ = counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    (format!("http://127.0.0.1:{port}"), calls)
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

fn spawn_embedding_server() -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("绑定嵌入端口");
    let port = listener.local_addr().expect("嵌入端口").port();
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = handle_embedding_request(&mut stream);
            let _ = counter.fetch_add(1, Ordering::SeqCst);
        }
    });
    (format!("http://localhost:{port}"), calls)
}

fn handle_embedding_request(stream: &mut TcpStream) -> std::io::Result<()> {
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
    let payload = serde_json::json!({
        "data": [{ "index": 0, "embedding": [1.0, 0.0] }]
    })
    .to_string();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()
}

struct StoredEmbedding;

impl EmbeddingProvider for StoredEmbedding {
    fn id(&self) -> String {
        RemoteEmbeddingProvider::provider_id("fake-embed")
    }

    fn embed(
        &self,
        texts: &[String],
    ) -> personal_knowledge_os_lib::core::error::Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|_| vec![1.0, 0.0]).collect())
    }
}

#[test]
fn generation_is_gated_and_never_changes_markdown_until_accept() {
    let fixture = Fixture::new();
    let note_id = fixture.note(
        "SQLite 索引设计",
        "# FTS5\n\nSQLite FTS5 使用倒排索引。中文检索需要双字短语扩展。",
    );
    let notes = NoteService::new(fixture.context());
    let detail = notes.get(&note_id).expect("读取笔记");
    let path = detail.note.file_path.clone();
    let before = fixture
        .context()
        .storage
        .read_raw(&path)
        .expect("读取原文件");
    let service = fixture.organize();

    let blocked = service
        .generate(&note_id, SuggestionKind::Summary)
        .expect_err("AI 关闭时生成建议应被拒绝");
    assert_eq!(blocked.code.as_str(), "AI_DISABLED");

    fixture.enable_ai();
    let suggestion = service
        .generate(&note_id, SuggestionKind::Summary)
        .expect("生成摘要建议");
    assert_eq!(suggestion.status, SuggestionStatus::Pending);
    assert!(!suggestion.evidence.is_empty());
    assert_eq!(
        fixture
            .context()
            .storage
            .read_raw(&path)
            .expect("生成后读取"),
        before,
        "未点击接受前文件必须字节不变"
    );

    let accepted = service.accept(&suggestion.id).expect("接受摘要");
    assert!(accepted.undo_available);
    let after_accept = fixture
        .context()
        .storage
        .read_raw(&path)
        .expect("接受后读取");
    assert_ne!(after_accept, before);
    assert!(after_accept.contains("## 摘要"));

    let undone = service.undo(&suggestion.id).expect("撤销摘要");
    assert!(!undone.undo_available);
    let restored = fixture
        .context()
        .storage
        .read_raw(&path)
        .expect("撤销后读取");
    assert_eq!(restored, before, "撤销必须字节级恢复");
}

#[test]
fn configured_local_model_refines_summary_and_records_its_provider() {
    let fixture = Fixture::new();
    let note_id = fixture.note("索引设计", "SQLite FTS5 使用倒排索引。");
    let (endpoint, calls) = spawn_model_server(
        r#"{"summary":"SQLite FTS5 通过倒排索引支持全文检索。","bullets":["使用倒排索引","支持中文双字短语"]}"#,
    );
    fixture.configure_local_model(&endpoint);
    let suggestion = fixture
        .organize()
        .generate(&note_id, SuggestionKind::Summary)
        .expect("模型生成摘要建议");
    let AiSuggestionPayload::Summary { summary, .. } = suggestion.payload else {
        panic!("摘要建议类型不符");
    };
    assert!(summary.contains("倒排索引"));
    assert_eq!(suggestion.model, "local.ollama:stub:latest");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn stale_suggestion_is_rejected_after_external_content_change() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let note_id = fixture.note("索引设计", "# 结论\n\n倒排索引适合全文检索。");
    let service = fixture.organize();
    let suggestion = service
        .generate(&note_id, SuggestionKind::Summary)
        .expect("生成建议");
    let notes = NoteService::new(fixture.context());
    let detail = notes.get(&note_id).expect("读取笔记");
    notes
        .save(SaveNoteInput {
            id: note_id.clone(),
            content: "# 结论\n\n文件已由外部逻辑修改。".to_string(),
            base_content_hash: detail.note.content_hash,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("修改来源");

    let error = service
        .accept(&suggestion.id)
        .expect_err("过期建议必须拒绝");
    assert_eq!(error.code.as_str(), "NOTE_CONFLICT");
    assert!(error.message.contains("重新生成"));
}

#[test]
fn tag_suggestion_reuses_the_normal_tag_write_path_and_can_be_undone() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let note_id = fixture.note("SQLite 索引设计", "# SQLite\n\nFTS5 使用倒排索引。");
    let service = fixture.organize();
    let suggestion = service
        .generate(&note_id, SuggestionKind::Tags)
        .expect("生成标签建议");
    let AiSuggestionPayload::Tags { tags } = &suggestion.payload else {
        panic!("标签建议类型不符");
    };
    assert!(tags.iter().any(|tag| tag.tag == "SQLite"));
    let before = fixture
        .context()
        .storage
        .read_raw(
            &NoteService::new(fixture.context())
                .get(&note_id)
                .unwrap()
                .note
                .file_path,
        )
        .expect("原文件");

    service.accept(&suggestion.id).expect("接受标签");
    let tagged = NoteService::new(fixture.context())
        .get(&note_id)
        .expect("接受后");
    assert!(tagged.tags.iter().any(|tag| tag.name == "SQLite"));
    service.undo(&suggestion.id).expect("撤销标签");
    let restored = fixture
        .context()
        .storage
        .read_raw(
            &NoteService::new(fixture.context())
                .get(&note_id)
                .unwrap()
                .note
                .file_path,
        )
        .expect("撤销后");
    assert_eq!(sha256_hex_str(&restored), sha256_hex_str(&before));
}

#[test]
fn relation_suggestion_applies_typed_relation_with_reason_and_undoes_exactly() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let source_id = fixture.note(
        "SQLite 索引设计",
        "# 对比\n\nSQLite 索引需要评估写入放大与查询性能。",
    );
    let target_id = fixture.note("SQLite 索引优化", "# 优化\n\n覆盖索引可以减少回表查询。");
    let link_targets = (1..=3)
        .map(|index| {
            fixture.note(
                &format!("SQLite 索引设计补充 {index}"),
                "# 索引\n\nSQLite 索引需要评估写入放大与查询性能。",
            )
        })
        .collect::<Vec<_>>();
    let service = fixture.organize();
    let suggestion = service
        .generate(&source_id, SuggestionKind::Relations)
        .expect("生成关联建议");
    let AiSuggestionPayload::Relations { relations } = &suggestion.payload else {
        panic!("关联建议类型不符");
    };
    assert!(relations
        .iter()
        .any(|item| item.target_note_id == target_id));
    assert!(
        relations
            .iter()
            .any(|item| item.relation_type == "WIKILINK"),
        "关联建议应同时覆盖普通 Wiki 链接形态：{relations:?}"
    );
    let before = fixture
        .context()
        .storage
        .read_raw(
            &NoteService::new(fixture.context())
                .get(&source_id)
                .unwrap()
                .note
                .file_path,
        )
        .expect("原文件");

    service.accept(&suggestion.id).expect("接受关联");
    let view = RelationService::new(fixture.context())
        .list(&source_id)
        .expect("读取关系");
    assert!(view
        .outgoing
        .iter()
        .any(|item| item.relation_type == "RELATED_TO" && item.note.is_some()));
    let source_after_accept = NoteService::new(fixture.context())
        .get(&source_id)
        .expect("接受后的来源笔记");
    let target_titles = link_targets
        .iter()
        .filter_map(|target| NoteService::new(fixture.context()).get(target).ok())
        .map(|note| note.note.title)
        .collect::<Vec<_>>();
    assert!(
        target_titles
            .iter()
            .any(|title| source_after_accept.content.contains(&format!("[[{title}]]"))),
        "接受普通链接建议后必须写入目标 Wiki 链接：{}",
        source_after_accept.content
    );
    service.undo(&suggestion.id).expect("撤销关联");
    let restored = fixture
        .context()
        .storage
        .read_raw(
            &NoteService::new(fixture.context())
                .get(&source_id)
                .unwrap()
                .note
                .file_path,
        )
        .expect("撤销后");
    assert_eq!(restored, before);
}

#[test]
fn duplicate_detection_is_a_read_only_suggestion() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let source_id = fixture.note("索引设计", "# 说明\n\nSQLite 倒排索引。");
    fixture.note("索引设计（副本）", "# 说明\n\nSQLite 倒排索引，补充版本。");
    let service = fixture.organize();
    let suggestion = service
        .generate(&source_id, SuggestionKind::Duplicate)
        .expect("重复检测");
    let AiSuggestionPayload::Duplicate { candidates } = &suggestion.payload else {
        panic!("重复建议类型不符");
    };
    assert!(!candidates.is_empty());
    let before = fixture
        .context()
        .storage
        .read_raw(
            &NoteService::new(fixture.context())
                .get(&source_id)
                .unwrap()
                .note
                .file_path,
        )
        .expect("原文件");
    service
        .accept(&suggestion.id)
        .expect("接受重复建议只记状态");
    let after = fixture
        .context()
        .storage
        .read_raw(
            &NoteService::new(fixture.context())
                .get(&source_id)
                .unwrap()
                .note
                .file_path,
        )
        .expect("接受后");
    assert_eq!(after, before, "重复检测接受不能改文件");
}

#[test]
fn duplicate_vector_recall_requires_per_send_confirmation_and_is_audited() {
    let fixture = Fixture::new();
    let source_id = fixture.note("索引设计", "# 说明\n\nSQLite 倒排索引。");
    fixture.note("索引设计（副本）", "# 说明\n\nSQLite 倒排索引，补充版本。");
    let (endpoint, calls) = spawn_embedding_server();
    fixture.configure_remote_embedding(&endpoint);
    VectorIndexService::new(fixture.context())
        .build(&StoredEmbedding, true)
        .expect("准备同模型的向量");

    let service = fixture.organize();
    service
        .generate_with_confirmation(&source_id, SuggestionKind::Duplicate, false)
        .expect("未确认时规则降级");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "未逐次确认时嵌入端点必须零请求"
    );
    let denied = AiPolicyService::new(fixture.context())
        .audit_tail(1)
        .expect("读取审计")
        .pop()
        .expect("生成审计");
    assert!(!denied.outbound, "未确认的生成审计不能标成外发");

    service
        .generate_with_confirmation(&source_id, SuggestionKind::Duplicate, true)
        .expect("确认后使用向量通道");
    assert!(calls.load(Ordering::SeqCst) > 0, "逐次确认后应允许查询嵌入");
    let confirmed = AiPolicyService::new(fixture.context())
        .audit_tail(1)
        .expect("读取审计")
        .pop()
        .expect("生成审计");
    assert!(confirmed.outbound, "实际调用嵌入端点后必须如实审计外发");
}

#[test]
fn undo_is_blocked_after_the_accepted_note_is_edited_again() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let note_id = fixture.note("索引设计", "# 结论\n\n倒排索引适合全文检索。");
    let service = fixture.organize();
    let suggestion = service
        .generate(&note_id, SuggestionKind::Summary)
        .expect("生成摘要");
    service.accept(&suggestion.id).expect("接受摘要");

    let notes = NoteService::new(fixture.context());
    let accepted = notes.get(&note_id).expect("接受后的笔记");
    notes
        .save(SaveNoteInput {
            id: note_id.clone(),
            content: "# 新结论\n\n用户在撤销前继续编辑了这篇笔记。".to_string(),
            base_content_hash: accepted.note.content_hash,
            title: None,
            category_id: None,
            tags: None,
        })
        .expect("接受后继续编辑");

    let error = service
        .undo(&suggestion.id)
        .expect_err("旧撤销不能覆盖后续编辑");
    assert_eq!(error.code.as_str(), "NOTE_CONFLICT");
    let current = notes.get(&note_id).expect("当前笔记");
    assert!(current.content.contains("用户在撤销前继续编辑"));
    assert!(!current.content.contains("## 摘要"));
}

#[test]
fn reject_never_writes_and_batch_accept_reports_partial_failure() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let first = fixture.note("批量摘要一", "第一篇摘要材料。");
    let second = fixture.note("批量摘要二", "第二篇摘要材料。");
    let service = fixture.organize();
    let first_before = fixture
        .context()
        .storage
        .read_raw(
            &NoteService::new(fixture.context())
                .get(&first)
                .unwrap()
                .note
                .file_path,
        )
        .expect("第一篇");
    let rejected = service
        .generate(&first, SuggestionKind::Summary)
        .expect("生成第一篇建议");
    service.reject(&rejected.id).expect("拒绝建议");
    let audit = AiPolicyService::new(fixture.context())
        .audit_tail(5)
        .expect("读取审计");
    assert!(
        audit
            .iter()
            .any(|entry| entry.capability == "organize_reject"),
        "拒绝建议也必须留下审计：{audit:?}"
    );
    let first_after = fixture
        .context()
        .storage
        .read_raw(
            &NoteService::new(fixture.context())
                .get(&first)
                .unwrap()
                .note
                .file_path,
        )
        .expect("拒绝后");
    assert_eq!(first_after, first_before, "拒绝不能修改文件");

    let first_suggestion = service
        .generate(&first, SuggestionKind::Summary)
        .expect("重新生成第一篇建议");
    let second_suggestion = service
        .generate(&second, SuggestionKind::Summary)
        .expect("生成第二篇建议");
    let report = service.accept_many(&[
        first_suggestion.id,
        second_suggestion.id,
        "suggestion_missing".to_string(),
    ]);
    assert_eq!(report.requested, 3);
    assert_eq!(report.succeeded, 2);
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].suggestion_id, "suggestion_missing");
}

#[test]
fn hundred_item_jobs_pause_resume_and_cancel_with_persisted_status() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let ids: Vec<String> = (0..100)
        .map(|index| fixture.note(&format!("批量笔记 {index}"), "SQLite 全文检索的简短记录。"))
        .collect();
    let service = fixture.organize();
    let job = service
        .create_job(SuggestionKind::Summary, &ids)
        .expect("创建 100 篇任务");
    assert_eq!(job.total, 100);

    let paused = service.pause_job(&job.id).expect("暂停");
    assert_eq!(paused.status, AiJobStatus::Paused);
    let still_paused = service.run_job(&job.id).expect("直接运行不能覆盖暂停");
    assert_eq!(still_paused.status, AiJobStatus::Paused);
    let completed = service.resume_job(&job.id).expect("恢复并完成");
    assert_eq!(completed.status, AiJobStatus::Completed);
    assert_eq!(completed.completed, 100);
    assert_eq!(completed.failed, 0);

    let cancel_job = service
        .create_job(SuggestionKind::Summary, &ids)
        .expect("创建取消任务");
    let canceled = service.cancel_job(&cancel_job.id).expect("取消");
    assert_eq!(canceled.status, AiJobStatus::Canceled);
    assert_eq!(
        service
            .run_job(&cancel_job.id)
            .expect("终态任务不再执行")
            .status,
        AiJobStatus::Canceled
    );
}

#[test]
fn failed_job_retry_requeues_items_and_completes_after_the_blocker_is_removed() {
    let fixture = Fixture::new();
    let note_id = fixture.note("重试任务", "SQLite 全文检索需要可重试的后台任务。");
    let service = fixture.organize();
    let job = service
        .create_job(SuggestionKind::Summary, &[note_id])
        .expect("创建任务");

    let failed = service.run_job(&job.id).expect("首次执行返回失败状态");
    assert_eq!(failed.status, AiJobStatus::Failed);
    assert_eq!(failed.failed, 1);

    fixture.enable_ai();
    let retried = service.retry_job(&job.id).expect("解除阻塞后重试");
    assert_eq!(retried.status, AiJobStatus::Completed);
    assert_eq!(retried.completed, 1);
    assert_eq!(retried.failed, 0);
}

#[test]
fn reopening_a_workspace_pauses_an_interrupted_job_and_requeues_its_running_item() {
    let fixture = Fixture::new();
    fixture.enable_ai();
    let ids: Vec<String> = (0..3)
        .map(|index| fixture.note(&format!("重启任务 {index}"), "SQLite 索引摘录。"))
        .collect();
    let service = fixture.organize();
    let job = service
        .create_job(SuggestionKind::Summary, &ids)
        .expect("创建任务");
    fixture
        .context()
        .db
        .with_conn(|conn| {
            let repo = SqliteAiOrganizeRepository;
            repo.set_job_status(conn, &job.id, AiJobStatus::Running, None)?;
            let item = repo.next_pending_item(conn, &job.id)?.expect("领取任务项");
            assert_eq!(
                item.status,
                personal_knowledge_os_lib::domain::organize::AiJobItemStatus::Pending
            );
            Ok(())
        })
        .expect("模拟运行中退出");

    let root = fixture.root();
    WorkspaceService::new(Arc::clone(&fixture.app))
        .close()
        .expect("关闭工作区");
    WorkspaceService::new(Arc::clone(&fixture.app))
        .open(&root)
        .expect("重新打开工作区");

    let reopened = OrganizeService::new(fixture.context());
    let paused = reopened.get_job(&job.id).expect("恢复任务状态");
    assert_eq!(paused.status, AiJobStatus::Paused);
    let resumed = reopened.resume_job(&job.id).expect("恢复执行");
    assert_eq!(resumed.status, AiJobStatus::Completed);
    assert_eq!(resumed.completed, 3);
}
