//! AI 应用层：策略闸门、检索、生成/决策模型与上层能力。
//!
//! 目的：先固定「决策层 / 检索层 / 生成层」的 trait 与 DTO，
//! 让第一代功能（笔记、检索、关系、生命周期）继续按自己的节奏演进；
//! 真正接入 Jev / 本地模型时，只需实现这些 trait，不改动上层业务。
//!
//! - 边界与理由：`docs/AI_ARCHITECTURE_PLAN.md`
//! - 差距盘点与技术选型：`docs/NEXT_GEN_ANALYSIS.md`
//! - **决策层 `DecisionModel` 已有三个实现**：规则 `RuleDecisionProvider`（离线兜底、无网络、无新依赖）、
//!   本地 `OllamaModel`、在线 `RemoteModel`（在线档需显式授权后才构造）；
//! - **生成层 `GenerativeModel` 同构**：`TemplateGenerativeModel` / `OllamaModel` / `RemoteModel`；
//! - **检索层 `Retriever` 已有关键词与可选向量实现**，由 `retrieval::retrieve` 做 RRF 融合；
//! - **Ask（V2.0 M1）** 已把检索、生成和三档降级串成带引用校验的只读问答。

pub mod ask;
pub mod policy;
pub mod decision_cache;
pub mod calibration;
pub mod decision_engine;
pub mod decision_probe;
pub mod diagnose;
pub mod embeddings;
pub mod evidence;
pub mod host;
pub mod local_model;
pub mod model_gateway;
pub mod organize;
pub mod presets;
pub mod query_expansion;
pub mod remote_model;
pub mod research;
pub mod retrieval;
pub mod rule_provider;
pub mod system_one;
pub mod source_grade;
pub mod template_provider;
pub mod tools;
pub mod traits;
pub mod vector_index;
pub mod web_search;
pub mod writing;

pub use policy::AiPolicyService;
pub use ask::AskService;
pub use decision_cache::{CachedDecision, DecisionCache};
pub use decision_engine::{DecisionEngine, DecisionOutcome};
pub use diagnose::{AiDiagnosis, AiDiagnosisCheck, DiagnoseInputs, HttpLocalModelProbe, LocalModelProbe};
pub use local_model::OllamaModel;
pub use organize::OrganizeService;
pub use presets::{LocalModelPreset, LOCAL_MODEL_PRESETS};
pub use remote_model::RemoteModel;
pub use research::ResearchService;
pub use retrieval::{reciprocal_rank_fusion, retrieve, Bm25Retriever, ChannelStatus, FusedChunk, RetrievalOutcome};
pub use embeddings::{EmbeddingProvider, RemoteEmbeddingProvider};
pub use vector_index::{cosine_similarity, VectorIndexService, VectorStore};
pub use rule_provider::RuleDecisionProvider;
pub use system_one::SystemOneDecisionModel;
pub use template_provider::TemplateGenerativeModel;
pub use traits::{
    DecisionCandidate, DecisionChoice, DecisionKind, DecisionModel, DecisionResult, DecisionState,
    GenerationRequest, GenerationResponse, GenerativeModel, RetrievalOrigin, RetrievedChunk,
    Retriever,
};
