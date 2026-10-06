# AI 能力框架规划（AI Decision Layer）

> 状态：**规划稿**（2026-10-07 确认「现在就为未来 AI 引入预留框架结构」）。
> 本文只定义**框架与边界**，不实现模型调用；落地排期见 [ROADMAP.md](./ROADMAP.md) 的 V3.0。
> 原则与设计文档一致：**本地优先、可关闭、可离线、不阻塞编辑器**。

## 1. 目标与不做的事

**目标**：让「AI 能力」以**插件化的决策层**接入现有本地知识库，而不是把模型调用散落在 UI 里。

**不做**：

- 不做云端账号体系、不做团队协作与分享；
- 不默认上传任何笔记内容（默认只允许本地模型；远程模型必须显式开启并逐次确认）；
- 不把 AI 结果写回正文（默认只读 + 建议，写回必须用户确认并留痕）。

## 2. 分层结构

```text
表现层（React）
  AI 面板 / 行内建议 / 命令面板「AI 动作」
        │  invoke('ai_*')
        ▼
命令层  commands/ai_commands.rs          仅转发 + 事件（进度 / 流式片段）
        ▼
★ AI 决策层  application/ai/decision.rs
        │  意图识别 → 能力路由 → 计划 → 执行 → 校验 → 汇总
        ├── ai/capabilities/*    具体能力（摘要、问答、标签建议、关联推荐、重写）
        ├── ai/tools/*           只读/受限工具（检索、读笔记、列反链、统计）
        └── ai/policy.rs         隐私门、配额、超时、可关闭开关
        ▼
★ 模型接入层  ai/provider.rs（trait LlmProvider）
        ├── LocalLlmProvider       Ollama / llama.cpp / MLX（默认）
        └── RemoteLlmProvider      OpenAI 兼容 HTTP（需显式开启）
        ▼
★ 检索与向量层  ai/retrieval/*
        ├── KeywordRetriever       复用现有 FTS5（零额外依赖，永远可用）
        └── VectorRetriever        Embedding + 本地向量索引（可选增强）
        ▼
基础设施  FTS5 索引 / 向量文件 / 模型进程 / 缓存
```

## 3. 决策层（核心）

决策层是**唯一**决定「用不用 AI、用哪个能力、要不要外发数据」的地方。

| 阶段 | 输入 | 输出 | 约束 |
| --- | --- | --- | --- |
| 意图识别 | 用户动作 + 选中文本 + 当前笔记上下文 | `Intent`（枚举 + 置信度） | 优先规则/启发式，命中不了再交给模型 |
| 能力路由 | `Intent` + 能力注册表 | `CapabilityId` | 能力必须声明：需要的上下文、是否可外发、估算成本 |
| 计划 | `CapabilityId` + 上下文 | `Vec<ToolCall>` | 工具只能读，写操作必须转为「建议」返回 |
| 执行 | `ToolCall` 列表 | `Vec<ToolResult>` | 全部走工具层，禁止能力直接读数据库/文件 |
| 校验 | 工具结果 | 引用来源（noteId + 片段） | **回答问题必须带引用**，无引用不外发 |
| 汇总 | 结果 + 引用 | `AiAnswer { text, citations[], actions[] }` | `actions` 是**建议**，需要用户点击才落库 |

### 3.1 关键接口（Rust）

```rust
// ai/provider.rs —— 模型接入层：本地与远程实现同一个 trait
pub trait LlmProvider: Send + Sync {
    fn id(&self) -> &'static str;                       // "local.ollama" / "remote.openai"
    fn is_local(&self) -> bool;                          // 决定是否允许携带正文
    fn capabilities(&self) -> ProviderCapabilities;      // 是否支持流式 / 工具调用 / 长度上限
    fn complete(&self, request: LlmRequest) -> Result<LlmResponse>;
    fn stream(&self, request: LlmRequest, sink: &dyn Fn(LlmChunk)) -> Result<LlmResponse>;
}

// ai/retrieval/mod.rs —— 检索层：关键词与向量都实现同一个 trait
pub trait Retriever: Send + Sync {
    fn id(&self) -> &'static str;                        // "fts" / "vector"
    fn search(&self, query: RetrievalQuery) -> Result<Vec<RetrievedChunk>>;
    fn index_status(&self) -> IndexStatus;                // 复用现有 FTS 状态模型
}

// ai/tools/mod.rs —— 工具层：能力只能通过工具访问工作区
pub trait AiTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn kind(&self) -> ToolKind;                          // Read / Suggest（无 Write）
    fn call(&self, ctx: &ToolContext, args: serde_json::Value) -> Result<ToolOutput>;
}
```

### 3.2 决策层的输入输出（DTO 草案）

```rust
pub struct AiRequest {
    pub intent: Option<String>,      // 缺省时由决策层推断
    pub note_id: Option<String>,     // 当前笔记
    pub selection: Option<String>,   // 选中文本
    pub question: Option<String>,    // 显式提问
    pub allow_remote: bool,          // 本次是否允许外发（默认 false）
}

pub struct AiAnswer {
    pub text: String,
    pub citations: Vec<Citation>,    // { note_id, title, snippet }
    pub actions: Vec<SuggestedAction>, // 例如「添加标签」「建立链接」，需用户确认
    pub provider: String,
    pub usage: Option<Usage>,        // 本地模型为 None
    pub duration_ms: u64,
}
```

## 4. 检索层：关键词优先，向量可选

1. **第一阶段（默认）**：直接复用 FTS5 索引（已经支持中文双字短语与标题加权），
   决策层用 FTS + 标题匹配挑选候选片段 → 送进模型的上下文。
   好处：零新增依赖、离线可用、索引与现有 `search_*` 命令共用一套状态。
2. **第二阶段（可选增强）**：引入 embedding + 本地向量索引（文件存储，随工作区走），
   与 FTS 结果做加权融合（RRF）；索引可一键重建 / 删除，不参与备份的关键路径。

> 说明：需求里提到的 **JEV** 我按「向量检索/嵌入组件」理解并落在这一层的 `VectorRetriever` 位置；
> 若 JEV 是指别的机制（例如某种推理引擎或运行时），告诉我它的准确定义，
> 我会调整本节命名与接口而不是重写架构 —— 因为决策层只依赖 `Retriever` trait。

## 5. 隐私与开关（默认最保守）

| 开关 | 默认 | 说明 |
| --- | --- | --- |
| AI 功能总开关 | **关闭** | 关闭后所有 `ai_*` 命令返回 `AI_DISABLED`，UI 不显示入口 |
| 允许本地模型 | 开启（总开关打开后） | 本地模型不产生外发 |
| 允许远程模型 | **关闭** | 每次调用需显式勾选「本次允许外发」，并在回答里标注 |
| 发送原文 | **关闭** | 只发送检索命中的片段（默认截断到 N 字符），可配置为「仅标题」 |
| 结果缓存 | 关闭 | 开启后缓存在 `.knowledge/ai-cache/`，可一键清空 |

事件与审计：每次决策记录 `intent / capability / provider / 是否外发 / 耗时` 到日志
（不含正文），便于用户自查。

## 6. 分阶段落地

| 阶段 | 内容 | 依赖 |
| --- | --- | --- |
| A0（已完成） | 现有 FTS5 检索、双向链接、附件与模板 —— AI 的「原料」 | — |
| A1 | `ai/policy.rs` + `ai/tools/*` + `ai_*` 命令骨架（不接模型，返回「未配置」） | 无 |
| A2 | `LocalLlmProvider`（Ollama HTTP）+ 摘要 / 问答 / 标签建议三个能力 | A1 |
| A3 | 行内建议与「AI 动作」入口、引用展示、建议落库确认流程 | A2 |
| A4 | `VectorRetriever`（可选）+ 关联推荐、图谱辅助聚类 | A2、M3 图谱 |
| A5 | 远程 provider（OpenAI 兼容）+ 每次外发确认 + 成本统计 | A3 |

**验收口径**：A2 起，任意能力必须满足 —— 无网络可用（本地模型）、回答带引用、
关闭总开关后界面无残留入口、任何写回都需要用户确认。

## 7. 与现有模块的边界

- 复用：`SearchService`（FTS）、`LinkService`（关联）、`NoteService`（只读路径）；
- 不侵入：AI 不进入保存/索引写入路径，除非用户确认建议，且确认后仍走 `note_save`；
- 可测试：决策层与工具层不依赖具体模型，测试用 `MockProvider` + 固定 `Retriever`；
- 可卸载：新增能力全部落在 `application/ai/`，删除该目录不影响主流程编译。
