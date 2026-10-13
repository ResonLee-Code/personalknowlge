//! 向量索引与向量通道（D3 / N7 第二步、第三步）集成测试。
//!
//! 用**确定性的假嵌入**（字符分桶袋 + 归一化）跑通整条链路，
//! 不依赖真实网络，也不受网络抖动影响：
//! 1. 构建是**增量**的：内容没变就不重嵌；
//! 2. **未构建 / 未授权时向量通道如实跳过**，BM25 照常（检索永远可用）；
//! 3. 融合**不会把 BM25 找到的命中弄丢**（RRF 的单调性）；
//! 4. 构建**必须显式确认**——这是把整库内容发出去的一次性动作。

use std::sync::Arc;

use personal_knowledge_os_lib::application::ai::embeddings::EmbeddingProvider;
use personal_knowledge_os_lib::application::ai::retrieval::{retrieve, Bm25Retriever, VectorRetriever};
use personal_knowledge_os_lib::application::ai::traits::Retriever;
use personal_knowledge_os_lib::application::ai::vector_index::VectorIndexService;
use personal_knowledge_os_lib::application::workspace_service::WorkspaceService;
use personal_knowledge_os_lib::core::error::Result;
use personal_knowledge_os_lib::core::events::NoopEventSink;
use personal_knowledge_os_lib::domain::ai::UpdateAiPolicyInput;
use personal_knowledge_os_lib::domain::note::CreateNoteInput;
use personal_knowledge_os_lib::infrastructure::credential_store;
use personal_knowledge_os_lib::state::AppInner;
use tempfile::TempDir;

/// 确定性假嵌入：字符分桶袋 + 归一化。
///
/// 足够验证"共词越多越近"；**不是**真实语义模型，测试里也不假装是
/// （`id()` 明确带 `fake.embeddings` 前缀，写进库也看得出来）。
struct FakeEmbedding {
    dim: usize,
}

impl EmbeddingProvider for FakeEmbedding {
    fn id(&self) -> String {
        format!("fake.embeddings:{}", self.dim)
    }

    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|text| {
                let mut vector = vec![0f32; self.dim];
                for character in text.chars() {
                    vector[(character as usize) % self.dim] += 1.0;
                }
                let norm: f32 = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for value in vector.iter_mut() {
                        *value /= norm;
                    }
                }
                vector
            })
            .collect())
    }
}

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
            .create("向量检索测试", &workspace.path().to_string_lossy())
            .expect("创建工作区");
        Self { data, _workspace: workspace, app }
    }

    fn context(&self) -> Arc<personal_knowledge_os_lib::application::context::WorkspaceContext> {
        self.app.context().expect("工作区已打开")
    }

    fn note(&self, title: &str, body: &str) {
        personal_knowledge_os_lib::application::note_service::NoteService::new(self.context())
            .create(CreateNoteInput {
                title: title.to_string(),
                category_id: None,
                initial_content: Some(body.to_string()),
                tags: None,
                parent_id: None,
            })
            .expect("新建笔记");
    }

    fn policy(&self) -> personal_knowledge_os_lib::application::ai::policy::AiPolicyService {
        personal_knowledge_os_lib::application::ai::policy::AiPolicyService::new(self.context())
    }

    /// 配置向量检索（开启 + 填嵌入模型名 + 授权一次）。
    fn configure_vector(&self) {
        credential_store::set_remote_model_key(self.data.path(), Some("sk-test")).expect("写凭据");
        self.policy()
            .update(UpdateAiPolicyInput {
                enabled: Some(true),
                remote_model_base_url: Some("https://api.example.com/v1".to_string()),
                remote_model_name: Some("chat-model".to_string()),
                remote_embedding_model: Some("fake-embed".to_string()),
                vector_enabled: Some(true),
                confirm_vector: Some(true),
                ..Default::default()
            })
            .expect("配置向量检索");
    }
}

fn fake() -> Arc<FakeEmbedding> {
    Arc::new(FakeEmbedding { dim: 64 })
}

#[test]
fn building_is_incremental() {
    let fixture = Fixture::new();
    fixture.configure_vector();
    fixture.note("索引设计", "# 索引设计\n\n倒排索引与分词策略。\n\n## 查询策略\n\n双字短语匹配。\n");
    fixture.note("调度实现", "# 调度\n\n工作窃取与任务唤醒。\n");

    let service = VectorIndexService::new(fixture.context());
    let provider = fake();

    let first = service.build(provider.as_ref(), true).expect("首次构建");
    assert!(first.embedded > 0);
    assert_eq!(first.skipped, 0, "首次没有可跳过的：{first:?}");

    // 内容没变 → 第二次全部跳过（**不发请求**）
    let second = service.build(provider.as_ref(), true).expect("二次构建");
    assert_eq!(second.embedded, 0, "内容没变不该重嵌：{second:?}");
    assert_eq!(second.skipped, first.total_chunks);

    // 改一篇笔记 → 只有它的分块要重嵌
    fixture.note("索引设计二", "# 索引设计\n\n倒排索引与分词策略（改过）。\n\n## 查询策略\n\n双字短语匹配。\n");
    let third = service.build(provider.as_ref(), true).expect("三次构建");
    assert!(third.embedded > 0 && third.embedded < third.total_chunks, "只该重嵌变化的那部分：{third:?}");
}

#[test]
fn build_requires_explicit_confirmation() {
    let fixture = Fixture::new();
    fixture.configure_vector();
    fixture.note("索引设计", "# 标题\n\n内容。\n");

    let error = VectorIndexService::new(fixture.context())
        .build(fake().as_ref(), false)
        .expect_err("没确认就必须拒绝");
    assert!(error.message.contains("确认"), "{}", error.message);
}

#[test]
fn unbuilt_index_means_the_vector_channel_is_skipped_but_bm25_still_works() {
    let fixture = Fixture::new();
    fixture.configure_vector();
    fixture.note("索引设计", "# 索引设计\n\n倒排索引与分词策略，查询优化要注意召回率。\n");

    let context = fixture.context();
    let bm25 = Bm25Retriever::new(Arc::clone(&context));
    let vector = VectorRetriever::new(Arc::clone(&context), fake());
    let providers: Vec<&dyn Retriever> = vec![&bm25, &vector];

    let outcome = retrieve(&providers, "查询优化", 5);
    let vector_channel = outcome
        .channels
        .iter()
        .find(|channel| channel.id == "vector")
        .expect("通道状态里必须有 vector");
    assert!(!vector_channel.available, "没构建索引时向量档不可用");
    assert!(
        !outcome.hits.is_empty(),
        "**检索必须照常可用**（BM25 兜底）：{:?}",
        outcome.channels
    );
    assert_eq!(outcome.hits[0].providers, vec!["bm25".to_string()]);
}

#[test]
fn vector_channel_joins_the_fusion_without_losing_bm25_hits() {
    let fixture = Fixture::new();
    fixture.configure_vector();
    // 小语料：用来做"同一批查询"的召回对比
    fixture.note("索引设计", "# 索引设计\n\n倒排索引与分词策略，查询优化要注意召回率。\n");
    fixture.note("调度实现", "# 调度\n\n工作窃取与任务唤醒，批量窃取降低竞争。\n");
    fixture.note("缓存策略", "# 缓存\n\nLRU 淘汰与命中率统计。\n");
    fixture.note("分词笔记", "# 分词\n\n中文双字短语与召回率。\n");

    let context = fixture.context();
    let service = VectorIndexService::new(Arc::clone(&context));
    let provider = fake();
    let report = service.build(provider.as_ref(), true).expect("构建");
    assert!(report.embedded > 0);
    let status = service.status(Some(&provider.id())).expect("状态");
    assert!(status.available, "构建之后向量档应当可用：{status:?}");

    let bm25 = Bm25Retriever::new(Arc::clone(&context));
    let vector = VectorRetriever::new(Arc::clone(&context), provider);

    let queries = ["查询优化", "工作窃取", "命中率", "双字短语"];
    let mut vector_contributed = false;
    println!("查询\tBM25 命中\t融合命中\t向量参与");
    for query in queries {
        let bm25_only = retrieve(&[&bm25], query, 5);
        let fused = retrieve(&[&bm25, &vector], query, 5);

        let bm25_notes: Vec<&str> = bm25_only
            .hits
            .iter()
            .map(|hit| hit.chunk.note_id.as_str())
            .collect();
        let fused_notes: Vec<&str> = fused
            .hits
            .iter()
            .map(|hit| hit.chunk.note_id.as_str())
            .collect();
        let used_vector = fused
            .hits
            .iter()
            .any(|hit| hit.providers.iter().any(|provider| provider == "vector"));
        vector_contributed |= used_vector;
        println!(
            "{query}\t{}\t{}\t{}",
            bm25_notes.len(),
            fused_notes.len(),
            used_vector
        );
        println!(
            "   通道：{:?}",
            fused
                .channels
                .iter()
                .map(|channel| (channel.id.as_str(), channel.available, channel.note.as_str()))
                .collect::<Vec<_>>()
        );

        // 核心性质：**融合不会把 BM25 召回的丢掉**（RRF 只叠加，不剔除）
        for note_id in &bm25_notes {
            assert!(
                fused_notes.contains(note_id),
                "融合后不应丢掉 BM25 的命中：query={query} note={note_id}（融合：{fused_notes:?}）"
            );
        }
    }
    assert!(vector_contributed, "构建之后，向量档应当真的参与了召回");
}
