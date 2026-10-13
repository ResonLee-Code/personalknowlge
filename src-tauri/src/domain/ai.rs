//! AI 策略与审计的 DTO（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §6、§9.7）。
//!
//! 默认口径（2026-10-10 用户确认）：
//! - AI 总开关**默认关闭**；关闭后所有 `ai_*` 命令返回 `AI_DISABLED`；
//! - 联网档位**默认仅本地**（`localOnly`）；切到 `allowNetwork` 后**首次使用仍需单次确认**；
//! - 研究预算有上限（子问题 / 抓取页 / 时长），触顶即停。

use serde::{Deserialize, Serialize};

/// 联网档位（对标语雀式的「仅本地 / 允许联网检索」两档）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum NetworkMode {
    /// 只允许本地模型与本地检索（默认）
    #[default]
    LocalOnly,
    /// 允许联网检索（仍需单次确认后才真正发起请求）
    AllowNetwork,
}

impl NetworkMode {
    pub fn as_str(self) -> &'static str {
        match self {
            NetworkMode::LocalOnly => "localOnly",
            NetworkMode::AllowNetwork => "allowNetwork",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "localOnly" | "local_only" | "local" => Some(NetworkMode::LocalOnly),
            "allowNetwork" | "allow_network" | "network" => Some(NetworkMode::AllowNetwork),
            _ => None,
        }
    }
}

/// 一次决策**试过的一档**（Track D2 / N6）：可解释 Trace 的一行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionAttempt {
    /// provider 标识（`rule` / `local.ollama` / `remote.openai-compatible` …）
    pub provider: String,
    pub ok: bool,
    /// 人读原因（成功：给了几条结果；失败：为什么不可用）。**不含正文**。
    pub detail: String,
}

/// 决策 Trace（Track D2 / N6）：**每一次打分/决策都能解释清楚**它是怎么来的。
///
/// 纪律：只记元数据（类型 / 档位 / 条数 / 耗时），**不记笔记正文**——
/// 与审计同一条隐私口径。`kind` 用字符串，避免 domain 依赖 application 的枚举。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionTrace {
    /// 决策类型（`RELEVANCE` / `UTILITY` / `CONFLICT` …，与 `DecisionKind` 序列化一致）
    pub kind: String,
    /// `hit` = 直接吃缓存；`miss` = 真的走了一遍候选链
    pub cache: String,
    /// 最终生效的档位；全军覆没时为空串
    pub provider: String,
    /// 候选条数
    pub candidates: usize,
    /// 逐档尝试结果（按尝试顺序）
    pub attempts: Vec<DecisionAttempt>,
    pub took_ms: u64,
}

/// 一个检索通道的状态（D3/N7）：这次检索**实际用了哪些通道**、谁不可用、为什么。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalChannelStatus {
    /// `bm25` / `vector` / `graph`
    pub id: String,
    pub available: bool,
    /// 人读说明（召回了几条 / 为什么跳过）
    pub note: String,
}

/// 融合后的一条检索命中（D3/N7）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalHit {
    pub note_id: String,
    pub chunk_index: i64,
    /// 标题路径（`A > B`）
    pub heading: String,
    pub snippet: String,
    pub char_start: i64,
    pub char_end: i64,
    /// RRF 融合分（越大越靠前）
    pub score: f32,
    /// 贡献这条命中的通道
    pub providers: Vec<String>,
}

/// 一次检索的完整报告：通道状态 + 融合结果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalReport {
    pub channels: Vec<RetrievalChannelStatus>,
    pub hits: Vec<RetrievalHit>,
}

/// 向量索引状态（D3/N7 第二步）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorIndexStatus {
    /// 向量通道现在能不能用（配置就绪 + 已构建过向量）
    pub available: bool,
    pub built_chunks: i64,
    pub total_chunks: i64,
    /// 上次构建用的嵌入模型标识
    pub model: Option<String>,
    pub built_at: Option<String>,
    /// 人读说明（为什么可用 / 不可用）
    pub note: String,
}

/// 构建向量索引**之前**的预览（给确认框用）：要发多少、发给谁。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorIndexPreview {
    pub will_send: bool,
    pub host: Option<String>,
    pub notes: i64,
    pub chunks_total: i64,
    /// 本次真正要外发的片段数（增量：内容没变的不重发）
    pub chunks_to_embed: i64,
    pub reason: String,
}

/// 一次构建的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorIndexReport {
    pub model: String,
    pub total_chunks: i64,
    pub embedded: i64,
    pub skipped: i64,
    pub took_ms: u64,
}

/// 研究/抓取的预算上限（防止一次研究失控烧掉时间与额度）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ResearchBudget {
    /// 子问题上限（1–10）
    pub max_subquestions: u32,
    /// 单次抓取页数上限（1–50）
    pub max_pages: u32,
    /// 单次时长上限（秒，30–1800）
    pub max_duration_secs: u32,
}

impl Default for ResearchBudget {
    fn default() -> Self {
        Self { max_subquestions: 5, max_pages: 12, max_duration_secs: 480 }
    }
}

impl ResearchBudget {
    /// 与应用层的硬上限对齐（用户可调小，但不能调大）。
    pub fn clamped(&self) -> Self {
        Self {
            max_subquestions: self.max_subquestions.clamp(1, 10),
            max_pages: self.max_pages.clamp(1, 50),
            max_duration_secs: self.max_duration_secs.clamp(30, 1800),
        }
    }
}

/// AI 策略（持久化在工作区设置里）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiPolicy {
    /// AI 能力总开关（默认关闭）
    pub enabled: bool,
    /// 联网档位（默认仅本地）
    pub network: NetworkMode,
    /// 联网档位已由用户确认过一次（用于「首次使用再确认」）
    pub network_confirmed_at: Option<String>,
    /// 是否允许抓取网页正文（默认关闭；仅「搜索摘要」时可不开）
    pub allow_fetch_body: bool,
    /// 允许的域名白名单（为空 = 不预置白名单，按内置分级表判定）
    pub domain_allowlist: Vec<String>,
    /// 强制丢弃的域名黑名单
    pub domain_denylist: Vec<String>,
    /// 联网检索源地址（URL 模板，必须含 `{query}`；可选含 `{limit}`）
    ///
    /// 约定：返回 JSON（SearxNG `format=json` 或 `{ "results": [...] }` / 裸数组）。
    /// **默认为空**——应用不预置任何第三方检索服务，由用户自行配置。
    pub search_endpoint: Option<String>,
    /// 研究预算
    pub budget: ResearchBudget,
    /// 本地模型地址（Ollama HTTP，例如 `http://127.0.0.1:11434`）；为空 = 不启用本地模型
    pub local_model_endpoint: Option<String>,
    /// 本地模型名（例如 `qwen2.5:7b`）；为空 = 不启用
    pub local_model_name: Option<String>,
    /// 用本地模型**写报告**（生成侧；关闭时用模板实现）
    pub local_model_compose: bool,
    /// 用本地模型**打分**（决策侧近似；未校准，默认关闭，仅用于与规则档 A/B）
    pub local_model_score: bool,
    /// 在线模型：OpenAI 兼容端点的基地址（例如 `https://api.example.com/v1`）；为空 = 不启用
    pub remote_model_base_url: Option<String>,
    /// 在线模型名（例如 `deepseek-chat`）；为空 = 不启用
    pub remote_model_name: Option<String>,
    /// 用在线模型**写报告**（生成侧）
    pub remote_model_compose: bool,
    /// 用在线模型**打分**（决策侧近似）
    pub remote_model_score: bool,
    /// 在线模型是否已获授权（首次确认时间）。地址或模型名变更后会被清空，需要重新确认。
    pub remote_model_authorized_at: Option<String>,
    /// 是否已配置 API Key（**只回显「有没有」，不回显 Key 本身**；
    /// Key 存在应用数据目录的 `ai-credentials.json`，不进工作区与备份）
    #[serde(default)]
    pub remote_model_key_set: bool,
    /// **逐次外发确认**（Track C2，可选闸门）：开启后，把内容交给在线模型的**每一次**调用
    /// 都必须由调用方带上「本次已确认」；否则在线这一档**根本不参与**（零请求）。
    /// 默认关闭——默认口径仍是「首次授权 + 换地址作废」。
    #[serde(default)]
    pub confirm_each_outbound: bool,
    /// 向量检索（D3/N7 第二步，**默认关闭**）：开启后检索多一路「向量召回」。
    /// 嵌入**与在线模型共用地址与 Key**，但模型名单独一项（嵌入模型通常与聊天模型不同）。
    #[serde(default)]
    pub vector_enabled: bool,
    /// 嵌入模型名（例如 `text-embedding-3-small`）；为空 = 向量档不可用
    #[serde(default)]
    pub remote_embedding_model: Option<String>,
    /// 向量检索授权时间：**查询词**会发给嵌入端点，需要确认一次；
    /// 换地址 / 换嵌入模型后作废（与在线模型同一套口径）。
    #[serde(default)]
    pub vector_authorized_at: Option<String>,
    /// 决策模型档（D4）：`None`/`rule` = 离线规则档（默认）；
    /// `system_one` = TypeSafe Jev / Perplexity Decisions（结构相同的托管决策模型）。
    #[serde(default)]
    pub decision_provider: Option<String>,
    /// 决策模型端点基地址；为空 = 用 provider 预设
    #[serde(default)]
    pub decision_base_url: Option<String>,
    /// 决策模型名；为空 = 用 provider 预设（`pplx-decider-v1.1-27b` / `jev-latest`）
    #[serde(default)]
    pub decision_model: Option<String>,
    /// 决策档授权时间：打分会把**材料片段**发给该端点，需确认一次；换地址/换模型后作废
    #[serde(default)]
    pub decision_authorized_at: Option<String>,
    /// 是否已配置决策模型 Key（**只回显布尔**）
    #[serde(default)]
    pub decision_key_set: bool,
    /// 出站代理（默认跟随系统代理）；给"只有浏览器能上外网"的环境兜底
    #[serde(default)]
    pub outbound_proxy: Option<String>,
    /// 审计日志条数上限（超出后截断最旧的记录）
    pub audit_max_entries: u32,
}

impl Default for AiPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            network: NetworkMode::LocalOnly,
            network_confirmed_at: None,
            allow_fetch_body: false,
            domain_allowlist: Vec::new(),
            domain_denylist: Vec::new(),
            search_endpoint: None,
            budget: ResearchBudget::default(),
            local_model_endpoint: None,
            local_model_name: None,
            local_model_compose: false,
            local_model_score: false,
            remote_model_base_url: None,
            remote_model_name: None,
            remote_model_compose: false,
            remote_model_score: false,
            remote_model_authorized_at: None,
            remote_model_key_set: false,
            confirm_each_outbound: false,
            vector_enabled: false,
            remote_embedding_model: None,
            vector_authorized_at: None,
            decision_provider: None,
            decision_base_url: None,
            decision_model: None,
            decision_authorized_at: None,
            decision_key_set: false,
            outbound_proxy: None,
            audit_max_entries: 500,
        }
    }
}

/// 更新 AI 策略的输入（全部可选，缺省表示不改）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateAiPolicyInput {
    pub enabled: Option<bool>,
    pub network: Option<String>,
    /// 用户确认联网档位时置 true（写入时间戳）
    pub confirm_network: Option<bool>,
    pub allow_fetch_body: Option<bool>,
    pub domain_allowlist: Option<Vec<String>>,
    pub domain_denylist: Option<Vec<String>>,
    /// 检索源地址模板；传空字符串表示清除
    pub search_endpoint: Option<String>,
    pub local_model_endpoint: Option<String>,
    pub local_model_name: Option<String>,
    pub local_model_compose: Option<bool>,
    pub local_model_score: Option<bool>,
    /// 在线模型（OpenAI 兼容端点）：基地址；传空字符串表示清除
    pub remote_model_base_url: Option<String>,
    /// 在线模型名；传空字符串表示清除
    pub remote_model_name: Option<String>,
    pub remote_model_compose: Option<bool>,
    pub remote_model_score: Option<bool>,
    /// 首次授权确认（开启某一侧开关后置 true 才会真正外发）
    pub confirm_remote_model: Option<bool>,
    /// API Key：**只写不读**（读路径只返回 `remote_model_key_set`）；传空串表示清除
    pub remote_model_api_key: Option<String>,
    /// 逐次外发确认开关（Track C2，默认关闭）
    pub confirm_each_outbound: Option<bool>,
    /// 向量检索开关（D3，默认关闭）
    pub vector_enabled: Option<bool>,
    /// 嵌入模型名；传空字符串表示清除（换模型会作废向量授权）
    pub remote_embedding_model: Option<String>,
    /// 置 true 表示用户确认了「查询词与片段会发给嵌入端点」（写入授权时间戳）
    pub confirm_vector: Option<bool>,
    /// 决策模型档：`rule`（默认）/ `system_one`
    pub decision_provider: Option<String>,
    /// 决策模型基地址；传空字符串表示清除（回到 provider 预设）
    pub decision_base_url: Option<String>,
    /// 决策模型名；传空字符串表示清除
    pub decision_model: Option<String>,
    /// 置 true 表示用户确认了「材料片段会发给决策端点」
    pub confirm_decision: Option<bool>,
    /// 决策模型 API Key：**只写不读**；传空字符串表示清除
    pub decision_api_key: Option<String>,
    /// 出站代理（`http://host:port` / `socks5h://host:port`）；传空字符串表示跟随系统
    pub outbound_proxy: Option<String>,
    pub budget: Option<ResearchBudget>,
}

impl AiPolicy {
    /// 在线模型是否配置完整（基地址与模型名都填了才算）。
    pub fn remote_model(&self) -> Option<(&str, &str)> {
        let base = self.remote_model_base_url.as_deref()?.trim();
        let name = self.remote_model_name.as_deref()?.trim();
        if base.is_empty() || name.is_empty() {
            return None;
        }
        Some((base, name))
    }

    /// 本地模型是否可用（地址与模型名都填了才算配置完成）。
    pub fn local_model(&self) -> Option<(&str, &str)> {
        let endpoint = self.local_model_endpoint.as_deref()?.trim();
        let name = self.local_model_name.as_deref()?.trim();
        if endpoint.is_empty() || name.is_empty() {
            return None;
        }
        Some((endpoint, name))
    }
}

/// 一条审计记录（**不含正文**，只记行为与结果）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiAuditEntry {
    /// 时间（ISO 8601）
    pub at: String,
    /// 能力 / 工具名，例如 `research_start`、`web_fetch`
    pub capability: String,
    /// 是否发起了外发（联网 / 远程模型）
    pub outbound: bool,
    /// 结果：`ok` / `denied` / `error`
    pub outcome: String,
    /// 耗时毫秒
    pub duration_ms: u64,
    /// 备注（错误码或简要说明，不含正文）
    pub note: Option<String>,
}
