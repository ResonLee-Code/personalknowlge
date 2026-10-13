//! AI 策略 / 工具命令（V1.2.1 阶段 D1）。
//!
//! 这一层**只做转发**：闸门在 `application::ai::policy`，工具在 `application::ai::tools`。

use tauri::State;

use crate::application::ai::policy::AiPolicyService;
use crate::application::ai::diagnose::{self, AiDiagnosis, DiagnoseInputs, HttpLocalModelProbe};
use crate::application::ai::presets::{LocalModelPreset, LOCAL_MODEL_PRESETS};
use crate::application::ai::retrieval::{self, Bm25Retriever};
use crate::application::ai::embeddings::{EmbeddingProvider, RemoteEmbeddingProvider};
use crate::application::ai::retrieval::VectorRetriever;
use crate::application::ai::vector_index::VectorIndexService;
use crate::domain::ai::{VectorIndexPreview, VectorIndexReport, VectorIndexStatus};
use crate::application::ai::tools::{AiTools, KbHit};
use crate::application::clip_service::ClippedPage;
use crate::core::error::Result;
use crate::domain::ai::{AiAuditEntry, AiPolicy, UpdateAiPolicyInput};
use crate::infrastructure::credential_store;
use crate::state::AppState;

use super::run;

/// 按策略构造向量档的嵌入 Provider。
///
/// **任何一项不就绪就返回 `None`**——"不构造 = 不外发"是靠结构保证的，
/// 不靠调用时记得判断（与在线模型同一套纪律）。
fn embedding_provider(
    context: &std::sync::Arc<crate::application::context::WorkspaceContext>,
) -> Option<std::sync::Arc<dyn EmbeddingProvider>> {
    let policy = context.settings().ai;
    if !policy.enabled || !policy.vector_enabled || policy.vector_authorized_at.is_none() {
        return None;
    }
    let embedding_model = policy.remote_embedding_model.clone()?;
    let service = AiPolicyService::new(std::sync::Arc::clone(context));
    let (base, _chat_model, key) = service.remote_model()?;
    Some(std::sync::Arc::new(
        RemoteEmbeddingProvider::new(&base, &embedding_model, &key)
            .with_proxy(service.outbound_proxy()),
    ))
}

/// 读取当前 AI 策略（默认最保守）。
#[tauri::command]
pub async fn ai_policy_get(state: State<'_, AppState>) -> Result<AiPolicy> {
    let context = state.inner.context()?;
    run(move || Ok(AiPolicyService::new(context).policy())).await
}

/// 更新 AI 策略（总开关 / 联网档位 / 抓取正文 / 白黑名单 / 本地与在线模型 / 预算）。
///
/// **API Key 只写不读**：`remote_model_api_key` 在这里落到应用数据目录的凭据文件，
/// 不进工作区设置库（否则备份包、导出的 Markdown 会把 Key 一起带走）；
/// 响应里只用 `remote_model_key_set` 回显「配没配」。
#[tauri::command]
pub async fn ai_policy_set(
    state: State<'_, AppState>,
    input: UpdateAiPolicyInput,
) -> Result<AiPolicy> {
    let context = state.inner.context()?;
    let data_dir = state.inner.data_dir.clone();
    run(move || {
        if let Some(key) = input.remote_model_api_key.clone() {
            credential_store::set_remote_model_key(&data_dir, Some(&key))?;
        }
        AiPolicyService::new(context).update(input)
    })
    .await
}

/// 最近的行为审计（新的在前；不含正文）。
#[tauri::command]
pub async fn ai_audit_tail(state: State<'_, AppState>, limit: usize) -> Result<Vec<AiAuditEntry>> {
    let context = state.inner.context()?;
    run(move || AiPolicyService::new(context).audit_tail(limit)).await
}

/// `ai_model_presets`：本地模型的「低配机型预设」。
///
/// **值只来自后端常量**（`application::ai::presets`），前端只展示与填充——
/// 避免"前端一份、后端一份"两处漂移。此命令不需要工作区上下文。
#[tauri::command]
pub async fn ai_model_presets() -> Result<Vec<LocalModelPreset>> {
    Ok(LOCAL_MODEL_PRESETS.to_vec())
}

/// `ai_diagnose`：模型自检——现在「写报告」会用哪一档，为什么（Track C4）。
///
/// **只读**、**只在需要时探测本机**（`GET /api/tags`），**绝不联系在线端点**：
/// 诊断不能变成一次外发。因此这个命令**不要求 AI 已开启**——恰恰是用来查"为什么没生效"的。
#[tauri::command]
pub async fn ai_diagnose(state: State<'_, AppState>) -> Result<AiDiagnosis> {
    let context = state.inner.context()?;
    run(move || {
        let service = AiPolicyService::new(context);
        let policy = service.policy();
        let (remote_base_name, remote_key_present) = service.remote_diagnosis();
        let inputs = DiagnoseInputs {
            local: service.local_model(),
            remote_base_name,
            remote_key_present,
            policy: &policy,
        };
        Ok(diagnose::evaluate(inputs, &HttpLocalModelProbe))
    })
    .await
}

/// `ai_retrieval_search`：**多通道检索 + 融合**（D3/N7 第一步）。
///
/// 目前只有 `bm25` 一个通道（关键词检索永远可用）；返回**通道状态**（谁可用 / 为什么跳过）
/// 与融合后的命中（带原文精确区间，以及"是哪些通道召回的"）。
#[tauri::command]
pub async fn ai_retrieval_search(
    state: State<'_, AppState>,
    keyword: String,
    limit: usize,
) -> Result<crate::domain::ai::RetrievalReport> {
    let context = state.inner.context()?;
    run(move || {
        let keyword = keyword.trim().to_string();
        if keyword.is_empty() {
            return Err(crate::core::error::AppError::invalid_input("检索关键词不能为空。"));
        }
        let retriever = Bm25Retriever::new(std::sync::Arc::clone(&context));
        // 向量档：配好并授权了才参与；否则**如实标"不可用"并跳过**，BM25 照常
        let vector = embedding_provider(&context)
            .map(|provider| VectorRetriever::new(std::sync::Arc::clone(&context), provider));
        let mut providers: Vec<&dyn crate::application::ai::traits::Retriever> = vec![&retriever];
        if let Some(vector) = &vector {
            providers.push(vector);
        }
        let outcome = retrieval::retrieve(&providers, &keyword, limit.clamp(1, 50));
        Ok(retrieval::to_dto(outcome))
    })
    .await
}

/// `ai_decision_probe`：决策模型契约探针（D4）。
///
/// 用**我们自己的中文题目**跑一批探针，回报：准确率 / 自洽性 / 两次一致性 / 矛盾判别 /
/// 耗时 / 输入 token / 估算成本。**只读**：不碰知识库，只发这一小批请求。
#[tauri::command]
pub async fn ai_decision_probe(
    state: State<'_, AppState>,
) -> Result<crate::application::ai::decision_probe::DecisionProbeReport> {
    let context = state.inner.context()?;
    run(move || crate::application::ai::decision_probe::DecisionProbeService::new(context).run()).await
}

/// `ai_vector_index_status`：向量索引状态（建了多少、用的哪个模型、为什么不可用）。
#[tauri::command]
pub async fn ai_vector_index_status(
    state: State<'_, AppState>,
) -> Result<VectorIndexStatus> {
    let context = state.inner.context()?;
    run(move || {
        let provider_id = embedding_provider(&context).map(|provider| provider.id());
        VectorIndexService::new(context).status(provider_id.as_deref())
    })
    .await
}

/// `ai_vector_index_preview`：构建**之前**的预览（要发多少、发给谁）——只读，不发请求。
#[tauri::command]
pub async fn ai_vector_index_preview(
    state: State<'_, AppState>,
) -> Result<VectorIndexPreview> {
    let context = state.inner.context()?;
    run(move || {
        let provider_id = embedding_provider(&context).map(|provider| provider.id());
        VectorIndexService::new(context).preview(provider_id.as_deref())
    })
    .await
}

/// `ai_vector_index_build`：增量构建向量索引（**必须显式确认**；耗时可能较长）。
#[tauri::command]
pub async fn ai_vector_index_build(
    state: State<'_, AppState>,
    confirm: bool,
) -> Result<VectorIndexReport> {
    let context = state.inner.context()?;
    run(move || {
        let provider = embedding_provider(&context).ok_or_else(|| {
            crate::core::error::AppError::new(
                crate::core::error::ErrorCode::AiDisabled,
                "向量检索未就绪：需要在「设置 → AI」里开启、填嵌入模型名，并确认一次（嵌入与在线模型共用地址与 Key）。",
            )
        })?;
        VectorIndexService::new(context).build(provider.as_ref(), confirm)
    })
    .await
}

/// `ai_vector_index_clear`：清掉本地向量（派生数据，随时可重建）。
#[tauri::command]
pub async fn ai_vector_index_clear(state: State<'_, AppState>) -> Result<i64> {
    let context = state.inner.context()?;
    run(move || Ok(VectorIndexService::new(context).clear()? as i64)).await
}

/// `kb_search`：本地检索（复用 FTS5，不联网）。
#[tauri::command]
pub async fn ai_kb_search(
    state: State<'_, AppState>,
    keyword: String,
    limit: usize,
) -> Result<Vec<KbHit>> {
    let context = state.inner.context()?;
    run(move || AiTools::new(context).kb_search(&keyword, limit)).await
}

/// `web_fetch`：抓取网页（必须已开启 AI + 联网档位 + 抓取正文，并带单次确认）。
#[tauri::command]
pub async fn ai_web_fetch(
    state: State<'_, AppState>,
    url: String,
    confirmed: bool,
) -> Result<ClippedPage> {
    let context = state.inner.context()?;
    run(move || AiTools::new(context).web_fetch(&url, confirmed)).await
}

/// `web_search`：联网搜索（D3 接入；当前如实返回未接入，不伪造结果）。
#[tauri::command]
pub async fn ai_web_search(
    state: State<'_, AppState>,
    keyword: String,
    limit: usize,
    confirmed: bool,
) -> Result<Vec<super::super::application::ai::web_search::WebSearchHit>> {
    let context = state.inner.context()?;
    run(move || AiTools::new(context).web_search(&keyword, limit, confirmed)).await
}
