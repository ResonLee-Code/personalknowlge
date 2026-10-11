//! 第二代能力**接口预留**（方案储备，本版不接任何模型、不注册任何命令）。
//!
//! 目的：先固定「决策层 / 检索层 / 生成层」的 trait 与 DTO，
//! 让第一代功能（笔记、检索、关系、生命周期）继续按自己的节奏演进；
//! 真正接入 Jev / 本地模型时，只需实现这些 trait，不改动上层业务。
//!
//! - 边界与理由：`docs/AI_ARCHITECTURE_PLAN.md`
//! - 差距盘点与技术选型：`docs/NEXT_GEN_ANALYSIS.md`
//! - 唯一已实现的是离线兜底 `RuleDecisionProvider`（无网络、无新依赖）

pub mod policy;
pub mod host;
pub mod local_model;
pub mod research;
pub mod rule_provider;
pub mod source_grade;
pub mod template_provider;
pub mod tools;
pub mod traits;
pub mod web_search;

pub use policy::AiPolicyService;
pub use local_model::OllamaModel;
pub use research::ResearchService;
pub use rule_provider::RuleDecisionProvider;
pub use template_provider::TemplateGenerativeModel;
pub use traits::{
    DecisionCandidate, DecisionChoice, DecisionKind, DecisionModel, DecisionResult, DecisionState,
    GenerationRequest, GenerationResponse, GenerativeModel, RetrievalOrigin, RetrievedChunk,
    Retriever,
};
