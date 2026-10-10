//! 研究助手领域模型（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §9）。
//!
//! 这一层只描述数据结构：**不联网、不接模型**，因此任何字段都必须能被规则实现填满
//! （降级路径见 §9.6：没有 Jev 时用规则表，没有 LLM 时只做「收集 + 分级 + 列清单」）。
//!
//! 流程固定五步：立项 Plan → 收集 Collect → 验证 Verify → 汇总 Synthesize → 落库 Write-back；
//! 每步产物都持久化在 `.knowledge/research/<sessionId>.json`，可中断、可审计、可续跑。

use serde::{Deserialize, Serialize};

use crate::domain::ai::ResearchBudget;

/// 会话状态（只能单向推进；`Aborted` 后不再接受后续步骤）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResearchStatus {
    /// 已立项（有研究计划）
    Planned,
    /// 已收集材料
    Collected,
    /// 已完成交叉验证
    Verified,
    /// 已生成报告
    Composed,
    /// 已产出写入提案（尚未落库）
    Proposed,
    /// 已写入知识库（`written_note_id` 指向落库的笔记）
    Written,
    /// 已中止（可保留已有材料作为草稿）
    Aborted,
}

impl ResearchStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ResearchStatus::Planned => "planned",
            ResearchStatus::Collected => "collected",
            ResearchStatus::Verified => "verified",
            ResearchStatus::Composed => "composed",
            ResearchStatus::Proposed => "proposed",
            ResearchStatus::Written => "written",
            ResearchStatus::Aborted => "aborted",
        }
    }
}

/// 写入方式（`research_write` 的 `mode`）。
pub const WRITE_MODE_CREATE: &str = "create";
/// 追加到已有笔记（带写前历史快照，可回滚）
pub const WRITE_MODE_APPEND: &str = "append";
/// 放弃本次写入（不落库）
pub const WRITE_MODE_ABANDON: &str = "abandon";

/// 来源权威性级别 A–E（§9.3；`A` 最权威，`E` 不可用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SourceLevel {
    /// 官方文档 / 标准（RFC、ISO、W3C）/ 官方发布 / 同行评议论文
    A,
    /// 官方博客、厂商工程博客、知名项目文档
    B,
    /// 高质量技术媒体、会议演讲、教科书
    C,
    /// 个人博客、教程站、问答社区
    D,
    /// 论坛贴、无署名内容、AI 生成内容、出处无法判定
    E,
}

impl SourceLevel {
    /// 写入 frontmatter 的 `source_reliability`（1–5）。
    pub fn reliability(self) -> i64 {
        match self {
            SourceLevel::A => 5,
            SourceLevel::B => 4,
            SourceLevel::C => 3,
            SourceLevel::D => 2,
            SourceLevel::E => 1,
        }
    }

    /// 由 1–5 的可靠性反推级别（本地笔记的 `source_reliability` 用它对齐）。
    pub fn from_reliability(value: i64) -> Self {
        match value.clamp(1, 5) {
            5 => SourceLevel::A,
            4 => SourceLevel::B,
            3 => SourceLevel::C,
            2 => SourceLevel::D,
            _ => SourceLevel::E,
        }
    }

    /// 是否属于「弱来源」（D/E 需要两个独立来源一致才进结论段）。
    pub fn is_weak(self) -> bool {
        matches!(self, SourceLevel::D | SourceLevel::E)
    }

    pub fn label(self) -> &'static str {
        match self {
            SourceLevel::A => "A 权威",
            SourceLevel::B => "B 可信",
            SourceLevel::C => "C 一般",
            SourceLevel::D => "D 弱",
            SourceLevel::E => "E 不可用",
        }
    }
}

/// 来源通道。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceOrigin {
    /// 本知识库内的笔记（作为「已知基线」或证据）
    Local,
    /// 外部网页（D3 起才有；D1/D2 只会出现 `Local`）
    Web,
}

/// 研究请求（`research_start` 的输入）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ResearchRequest {
    /// 课题（必填；过于宽泛会被拒绝并要求补充范围）
    pub topic: String,
    /// 范围说明：领域 / 关注点 / 语言等
    pub scope: Option<String>,
    /// 时间窗（天）；缺省用 730（约两年）
    pub time_window_days: Option<u32>,
    /// 用户是否表达了「结果写入知识库」的意图（**必须显式**，AI 不自行决定）
    pub write_back: bool,
    /// 写回目标分类；为空表示 Inbox（未分类）
    pub target_category_id: Option<String>,
}

/// 一个子问题（研究计划的最小执行单元）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubQuestion {
    pub id: String,
    pub question: String,
    /// 检索关键词（本地 FTS5 与（D3 起）联网搜索共用）
    pub keywords: Vec<String>,
    /// 命中的本地来源数量（收集后回填）
    pub local_hits: usize,
}

/// 本地已有笔记（「已知基线」，避免重复劳动，也会出现在报告第 4 段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KbBaseline {
    pub note_id: String,
    pub title: String,
    pub snippet: String,
}

/// 研究计划（① 立项的产物）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchPlan {
    pub topic: String,
    pub scope: Option<String>,
    /// 3–5 个子问题（受预算上限约束）
    pub sub_questions: Vec<SubQuestion>,
    /// 本地知识库里的已知基线
    pub local_baseline: Vec<KbBaseline>,
    /// 时间窗（天）
    pub time_window_days: u32,
    /// 本次预算快照（取自 AI 策略）
    pub budget: ResearchBudget,
    /// 计划说明（含降级路径与局限，如实告知用户）
    pub notes: Vec<String>,
}

/// 一条来源（本地笔记或外部网页）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchSource {
    /// 会话内引用编号从 1 起（报告里写作 `[n]`）
    pub reference: usize,
    pub title: String,
    /// 外部来源地址（本地来源为空）
    pub url: Option<String>,
    pub origin: SourceOrigin,
    /// 本地笔记 id（本地来源必填）
    pub note_id: Option<String>,
    /// 命中的子问题
    pub sub_question_id: String,
    pub level: SourceLevel,
    /// 与 `level` 一致的 1–5 可靠性
    pub reliability: i64,
    /// 分级依据（可读，便于用户核对，不是黑盒）
    pub grade_reason: String,
    /// 来源本身的发布时间（可缺省 → 时效未知）
    pub published_at: Option<String>,
    /// 本地：笔记更新时间；外部：抓取时间
    pub fetched_at: String,
    /// 适用版本（例如「Rust 1.82」）；未知为 `None`
    pub applicable_version: Option<String>,
    /// 摘要片段（外部来源只存摘要，不整篇复制）
    pub snippet: String,
    /// 是否可能已过时（发布时间早于时间窗且无更新版本）
    pub stale: bool,
    /// 决策层给出的「来源可信度」0–1（§9.6：Jev 或规则档；写入时与级别一起看）
    #[serde(default)]
    pub credibility: f32,
}

/// 一条结论/事实（以子问题为单位汇总）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchClaim {
    pub sub_question_id: String,
    pub text: String,
    /// 引用的来源编号（对应 `ResearchSource::reference`）
    pub citations: Vec<usize>,
    /// 支持来源中的最高级别（最保守的呈现用最低级别，见 `report.source_reliability`）
    pub best_level: SourceLevel,
    /// 是否交叉验证（≥2 个**独立**来源；单一来源一律为 false）
    pub cross_verified: bool,
    /// 时效：所有支持来源都在时间窗内
    pub fresh: bool,
    /// 与本知识库已有笔记的关联（`[[标题]]`）
    pub related_notes: Vec<String>,
    /// 决策层给出的「结论置信度」0–1（交叉验证与来源可信度的合成）
    #[serde(default)]
    pub confidence: f32,
}

/// 警告类别（报告里必须是**醒目块**，不是脚注）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WarningKind {
    /// 存在未验证来源（E 级只作线索）
    UnverifiedSources,
    /// 只有单一来源，未交叉验证
    SingleSource,
    /// 来源之间冲突（D4 才具备语义比对能力；D1 只会因规则命中而出现）
    Conflict,
    /// 时效未知或可能已过时
    Stale,
    /// 外部检索尚未接入（D3）
    ExternalNotAvailable,
    /// 本地没有找到任何相关材料
    NoLocalEvidence,
    /// 报告含有未接模型的模板生成内容
    TemplateGenerated,
    /// 报告/推测段由本地模型生成（未经人工核对）
    ModelGenerated,
}

/// 一条警告。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchWarning {
    pub kind: WarningKind,
    pub message: String,
    /// 相关来源编号（便于定位）
    #[serde(default)]
    pub references: Vec<usize>,
}

/// 报告第 4 段：与本地知识库的关系。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KbRelation {
    pub note_id: String,
    pub title: String,
    /// `补充` / `重复` / `相关`
    pub relation: String,
}

/// 调研报告（④ 汇总的产物，结构固定六段，便于测试与用户形成预期）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchReport {
    pub generated_at: String,
    /// 「资料截至 YYYY-MM-DD」
    pub data_cutoff: String,
    /// 生成者标识（D1/D2 为模板实现，接模型后换成 provider id）
    pub composer: String,
    /// 打分用的决策模型标识（规则档 `rule`；Jev 接入后变成 `jev` / `local.*`）
    #[serde(default)]
    pub decision_provider: String,
    /// 第 1 段：结论先行（**只放交叉验证过的**）
    pub conclusions: Vec<ResearchClaim>,
    /// 第 2 段：关键事实（**只放交叉验证过的**）
    pub key_facts: Vec<ResearchClaim>,
    /// 第 3 段：争议与不确定性
    pub conflicts: Vec<ResearchClaim>,
    /// 第 4 段：与本知识库的关系
    pub kb_relations: Vec<KbRelation>,
    /// 第 5 段：待验证清单（单一来源 / 时效未知）
    pub pending: Vec<ResearchClaim>,
    /// 第 6 段：参考来源
    pub references: Vec<ResearchSource>,
    /// 模型推测段（默认折叠；写入笔记时**默认不包含**，需显式开启）
    pub speculation: Vec<String>,
    /// 醒目警告块
    pub warnings: Vec<ResearchWarning>,
    /// 按最低级别取整的可靠性（写入 frontmatter）
    pub source_reliability: i64,
    /// 建议的复核日期（Y-M-D；到期后应用提示复核）
    pub valid_until: Option<String>,
    /// 渲染好的 Markdown（六段，顺序固定）
    pub markdown: String,
}

/// 同标题已有笔记（写回时提示，**默认不覆盖**）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateNote {
    pub note_id: String,
    pub title: String,
    pub updated_at: String,
}

/// 写入提案（⑤ 的产物；**必须用户确认才落库**，见 §9.5 `kb_write_proposal`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchProposal {
    pub title: String,
    /// 目标分类；`None` = Inbox（未分类）
    pub category_id: Option<String>,
    pub tags: Vec<String>,
    pub source_type: String,
    pub source_ref: String,
    pub source_reliability: i64,
    pub valid_until: Option<String>,
    /// 报告正文（Markdown）
    pub body: String,
    /// 写入前的 diff（新增文件的预览：每行 `+`）
    pub diff_preview: String,
    /// 同标题已有笔记（非空时必须由用户选择「新建 / 追加 / 放弃」）
    pub duplicates: Vec<DuplicateNote>,
    /// 是否建议同时创建「来源清单」子文档
    pub suggest_source_note: bool,
}

/// 写入知识库的输入（**必须由用户确认后**才调用；AI 不自行决定落库）。
///
/// 写入纪律（§9.3 已确认口径）：默认落 Inbox、同标题**不覆盖**（由用户选新建/追加/放弃）、
/// 写前展示 diff、可回滚（追加走 `note_save`，自动留历史快照）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ResearchWriteInput {
    pub session_id: String,
    /// `create` / `append` / `abandon`
    pub mode: String,
    /// 追加模式的目标笔记（缺省时取提案里唯一的同标题笔记）
    pub target_note_id: Option<String>,
    /// 是否把「模型推测」段一并写入（**默认 false**：推测段默认不落库）
    pub include_speculation: bool,
    /// 是否同时创建「来源清单」子文档
    pub create_source_note: bool,
    /// 覆盖标题（缺省用提案标题）
    pub title: Option<String>,
    /// 覆盖目标分类（缺省用提案的分类，通常是 Inbox）
    pub category_id: Option<String>,
}

impl Default for ResearchWriteInput {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            mode: WRITE_MODE_CREATE.to_string(),
            target_note_id: None,
            include_speculation: false,
            create_source_note: false,
            title: None,
            category_id: None,
        }
    }
}

/// 写入结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchWriteResult {
    pub session_id: String,
    /// `created` / `appended` / `abandoned`
    pub outcome: String,
    /// 落库的笔记 id（放弃时为空）
    pub note_id: Option<String>,
    pub note_title: Option<String>,
    /// 来源清单子文档 id（未创建时为空）
    pub source_note_id: Option<String>,
    pub source_reliability: i64,
    pub valid_until: Option<String>,
    /// 写入前展示的 diff（新增行以 `+` 开头，追加时带 3 行上下文）
    pub diff_preview: String,
}

/// 一步执行记录（进度与审计的最小单位）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchStepLog {
    pub step: String,
    pub at: String,
    pub detail: String,
}

/// 一次研究会话（持久化在 `.knowledge/research/<id>.json`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchSession {
    pub id: String,
    pub request: ResearchRequest,
    pub status: ResearchStatus,
    pub created_at: String,
    pub updated_at: String,
    pub plan: Option<ResearchPlan>,
    #[serde(default)]
    pub sources: Vec<ResearchSource>,
    pub report: Option<ResearchReport>,
    pub proposal: Option<ResearchProposal>,
    #[serde(default)]
    pub steps: Vec<ResearchStepLog>,
    /// 已用预估时长（毫秒；D1/D2 为本地操作的真实耗时）
    pub elapsed_ms: u64,
    /// 中止原因（`aborted` 时有值）
    pub abort_reason: Option<String>,
    /// 已落库的笔记 id（写回后回填）
    pub written_note_id: Option<String>,
    /// 本次打分用的决策模型标识（规则档 `rule` / `local.ollama:<模型>`）
    #[serde(default)]
    pub decision_provider: Option<String>,
}

impl ResearchSession {
    pub fn summary_line(&self) -> String {
        format!(
            "{} · {} 个子问题 · {} 条来源",
            self.request.topic,
            self.plan.as_ref().map(|plan| plan.sub_questions.len()).unwrap_or(0),
            self.sources.len()
        )
    }
}

/// 会话列表项（列表页用，避免把整份报告塞进列表响应）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchSessionSummary {
    pub id: String,
    pub topic: String,
    pub status: ResearchStatus,
    pub created_at: String,
    pub updated_at: String,
    pub source_count: usize,
    pub written_note_id: Option<String>,
}

impl From<&ResearchSession> for ResearchSessionSummary {
    fn from(session: &ResearchSession) -> Self {
        Self {
            id: session.id.clone(),
            topic: session.request.topic.clone(),
            status: session.status,
            created_at: session.created_at.clone(),
            updated_at: session.updated_at.clone(),
            source_count: session.sources.len(),
            written_note_id: session.written_note_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_level_round_trips_through_reliability() {
        for level in [
            SourceLevel::A,
            SourceLevel::B,
            SourceLevel::C,
            SourceLevel::D,
            SourceLevel::E,
        ] {
            assert_eq!(SourceLevel::from_reliability(level.reliability()), level);
        }
        assert!(SourceLevel::D.is_weak() && SourceLevel::E.is_weak());
        assert!(!SourceLevel::C.is_weak());
        assert_eq!(SourceLevel::from_reliability(999), SourceLevel::A, "越界夹取到 5");
    }

    #[test]
    fn session_summary_counts_plan_and_sources() {
        let session = ResearchSession {
            id: "r1".into(),
            request: ResearchRequest { topic: "Rust 异步".into(), ..Default::default() },
            status: ResearchStatus::Planned,
            created_at: "2026-10-11T10:00:00+08:00".into(),
            updated_at: "2026-10-11T10:00:00+08:00".into(),
            plan: Some(ResearchPlan {
                topic: "Rust 异步".into(),
                scope: None,
                sub_questions: vec![],
                local_baseline: vec![],
                time_window_days: 730,
                budget: ResearchBudget::default(),
                notes: vec![],
            }),
            sources: vec![],
            report: None,
            proposal: None,
            steps: vec![],
            elapsed_ms: 0,
            abort_reason: None,
            written_note_id: None,
            decision_provider: None,
        };
        assert!(session.summary_line().contains("Rust 异步"));
        let summary = ResearchSessionSummary::from(&session);
        assert_eq!(summary.status, ResearchStatus::Planned);
        assert_eq!(summary.source_count, 0);
    }
}
