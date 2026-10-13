//! **接口预留**（第二代方案储备，本版不接任何模型）。
//!
//! 目的：把「决策层 / 生成层 / 检索层」的边界与签名先固定下来，
//! 让第一代的功能开发（笔记、检索、关系、生命周期）不被未来的模型接入方式牵动。
//! 这里只定义 trait 与 DTO，不包含任何网络调用、依赖或后台任务。
//!
//! 设计依据见 `docs/AI_ARCHITECTURE_PLAN.md` 与 `docs/NEXT_GEN_ANALYSIS.md`。

use serde::{Deserialize, Serialize};

/// 决策类型（对应 V2.0 文档 §11 的输出状态与 §20 的能力分类）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionKind {
    /// 相关性：候选知识是否与当前问题相关
    Relevance,
    /// 有用性：是否值得进入证据集合
    Utility,
    /// 时效：是否已经过时
    Freshness,
    /// 冲突：两条知识是否互相矛盾
    Conflict,
    /// 被取代：新知识是否取代旧知识
    Superseded,
    /// 证据充分性：当前证据是否足以作答
    EvidenceSufficiency,
}

impl DecisionKind {
    /// 与 serde 的 `SCREAMING_SNAKE_CASE` 一致的字符串（Trace、日志、缓存列都用它）。
    pub fn as_str(self) -> &'static str {
        match self {
            DecisionKind::Relevance => "RELEVANCE",
            DecisionKind::Utility => "UTILITY",
            DecisionKind::Freshness => "FRESHNESS",
            DecisionKind::Conflict => "CONFLICT",
            DecisionKind::Superseded => "SUPERSEDED",
            DecisionKind::EvidenceSufficiency => "EVIDENCE_SUFFICIENCY",
        }
    }
}

/// 决策输出状态（与 V2.0 文档 §11 一致的枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionChoice {
    /// 当前答案应使用
    Use,
    /// 知识应保留（但与当前问题无关）
    Keep,
    /// 当前问题下无价值
    Drop,
    /// 无法确定
    Unknown,
    /// 与其他知识冲突
    Conflict,
    /// 已过时
    Stale,
    /// 已被新知识替代
    Superseded,
    /// 证据不足，需要继续检索
    NeedsMoreSearch,
}

/// 一次决策的输入：状态 + 候选集合。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionState {
    /// 用户问题或当前上下文摘要（不含整篇正文）
    pub query: String,
    /// 候选的轻量表示（标题、片段、时间、来源可靠性…）
    pub candidates: Vec<DecisionCandidate>,
}

/// 候选知识（**只携带必要字段**，用于控制外发体积与隐私）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionCandidate {
    pub note_id: String,
    pub title: String,
    /// 片段（默认可截断到 300–600 字）
    pub snippet: String,
    pub updated_at: String,
    pub source_reliability: Option<i64>,
    /// 可选的时效标记（来自 `valid_until`）
    pub valid_until: Option<String>,
}

/// 单条决策结果：区分 score（打分）与 confidence（模型对判断本身的把握）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionResult {
    pub note_id: String,
    pub choice: DecisionChoice,
    /// 0–1，越大越该用
    pub score: f32,
    /// 0–1，模型/规则的置信度（校准口径由 Provider 声明）
    pub confidence: f32,
    /// 人类可读的理由（Decision Trace 展示）
    pub reason: String,
    pub kind: DecisionKind,
}

/// 决策模型：Jev（云端，类型化 + 校准置信度）、本地小模型、规则实现同一个 trait。
pub trait DecisionModel: Send + Sync {
    /// 例如 "rule" / "jev" / "local.qwen"
    fn id(&self) -> &'static str;
    /// 是否本地（决定是否允许携带片段正文）
    fn is_local(&self) -> bool;
    /// 一次决策调用（Provider 内部负责 schema 声明、缓存与降级）
    fn decide(
        &self,
        kind: DecisionKind,
        state: &DecisionState,
    ) -> crate::core::error::Result<Vec<DecisionResult>>;

    /// 判断一组材料是否**互相矛盾**（Track C5 的语义档）。
    ///
    /// 默认返回 `Ok(None)` = "这一档不做语义冲突判断"，调用方据此回退规则档（极性词）。
    /// 本地 / 在线模型覆写它，让冲突判定从"关键词对峙"升级为"模型判 CONFLICT"。
    fn judge_conflict(
        &self,
        _query: &str,
        _candidates: &[DecisionCandidate],
    ) -> crate::core::error::Result<Option<ConflictVerdict>> {
        Ok(None)
    }
}

/// 一组材料是否互相矛盾的判定结果（Track C5）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictVerdict {
    pub conflict: bool,
    pub reason: String,
}

/// 检索片段（分块后的证据单元）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievedChunk {
    pub note_id: String,
    pub chunk_index: i64,
    /// 标题路径，例如 ["索引设计", "查询策略"]
    pub heading_path: Vec<String>,
    pub snippet: String,
    /// 正文中的字符区间，用于「引用跳回原文」
    pub char_start: i64,
    pub char_end: i64,
    /// 混合检索得分（BM25 / 向量 / 图）
    pub score: f32,
    /// 来源通道，便于调试与 Decision Trace 展示
    pub origin: RetrievalOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetrievalOrigin {
    /// 关键词（现有 FTS5）
    Bm25,
    /// 向量召回（可选增强）
    Vector,
    /// 知识图谱邻居
    Graph,
}

/// 检索层：第一阶段只实现 BM25（复用现有 FTS5），向量与图后续接入。
pub trait Retriever: Send + Sync {
    fn id(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> crate::core::error::Result<Vec<RetrievedChunk>>;
}

/// 生成模型（LLM）：本地 Ollama 与远程 OpenAI 兼容实现同一个 trait。
pub trait GenerativeModel: Send + Sync {
    fn id(&self) -> &'static str;
    fn is_local(&self) -> bool;
    /// 是否支持流式输出（界面按需选择）
    fn supports_streaming(&self) -> bool;
    fn complete(
        &self,
        request: &GenerationRequest,
    ) -> crate::core::error::Result<GenerationResponse>;

    /// 增量生成（Track C1）：成功时返回**完整**结果，并在过程中按块回调 `on_delta`。
    ///
    /// 默认实现退化为"一次性生成 + 一次回调"——不支持流式的 provider（例如模板实现）
    /// 不必改一行代码，就能走同一条调用路径。支持流式的 provider 覆写它。
    ///
    /// 约定：`on_delta` 收到的是**增量片段**（不是累计文本），调用方自己累加。
    fn complete_streaming(
        &self,
        request: &GenerationRequest,
        on_delta: &mut dyn FnMut(&str),
    ) -> crate::core::error::Result<GenerationResponse> {
        let response = self.complete(request)?;
        on_delta(&response.text);
        Ok(response)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationRequest {
    pub prompt: String,
    /// 证据片段（携带 note_id + 字符区间，便于生成引用）
    pub evidence: Vec<RetrievedChunk>,
    pub max_tokens: Option<u32>,
    /// 是否允许把片段外发给远程模型
    pub allow_remote: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationResponse {
    pub text: String,
    pub model: String,
    pub duration_ms: u64,
    pub usage: Option<GenerationUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerationUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}
