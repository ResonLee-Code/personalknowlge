# 下一代技术分析与调研（基于 V2.0 × Jev 架构文档）

> 状态：分析稿（2026-10-07）。输入文档：仓库根目录
> [Personal-Knowledge-OS-Jev-Architecture-V2.0.md](../Personal-Knowledge-OS-Jev-Architecture-V2.0.md)（52 节）。
> 本文回答三件事：**V2.0 文档要求 vs 当前代码的真实差距**、**关键技术选型的调研结论**、
> **下一代按什么顺序做什么**。
> 实现层的接口定义见 [AI_ARCHITECTURE_PLAN.md](./AI_ARCHITECTURE_PLAN.md)；
> 版本排期见 [ROADMAP.md](./ROADMAP.md)。

## 1. 结论摘要

1. 我们**已经完成了 V2.0 文档里 V1.0 + V1.1 的绝大部分**（Markdown 资产、FTS5/BM25 检索、
   双向链接、附件、导入导出、备份、知识图谱），且图谱比文档排期（V2.0）**提前交付**；
2. 真正的差距不在"AI"，而在**知识对象的建模**：文档要求的 `Fact` / `Decision` / `Source` /
   `Evidence` / 类型化 `Relation` / `Lifecycle` 目前都不存在，我们只有 `notes` + `note_links`；
3. 文档判断"先做基础设施、AI 只做可插拔判断层"**与我们的既有架构一致**，不需要返工；
4. 但在三点上要**修正文档的建议**：向量检索的落地方式（不建议引 C 扩展）、
   Jev 的定位（我们实测确认它是云端 early access，与"本地优先"冲突，必须与规则 Provider 双轨）、
   以及 Confidence 的可计算来源（4 层 Confidence 里只有一层来自模型，其余要自己造）；
5. 建议的下一代顺序：**先补知识对象/关系/生命周期（不需要 AI）→ 再补 Decision 骨架（仍然不需要模型）
   → 再补分块与向量检索 → 最后接 Jev 并做 A/B 评估**。这与文档 Phase 1→7 完全对齐。

## 2. 现状盘点：文档要求 vs 代码实现

### 2.1 已经具备（可以直接复用）

| 文档要求 | 当前实现 | 位置 |
| --- | --- | --- |
| Note / Folder / Tag / Search / Markdown / Attachment | ✅ 全量 | `note_service` / `category_service` / `tag_service` / `search_service` / `attachment_service` |
| Import / Export / Backup | ✅ 含目录递归导入、四格式导出、ZIP 备份校验 | `import_service` / `export_service` / `backup_service` |
| Backlink | ✅ 反链面板 + 引用计数 + 未解析链接一键建（M1） | `link_service` / `BacklinkPanel` |
| SQLite + FTS5 + BM25 | ✅ 中文双字短语 + 标题加权 + 高亮摘要 | `infrastructure/fts.rs` |
| Knowledge Graph | ✅ 节点/边/过滤/聚焦/社区着色/导出（M3，提前交付） | `graph_service` / `features/graph` |
| "Decision 不应写死进 Knowledge" 的思想 | ✅ 已用同构做法：`note_links` 定义为**可重建派生数据**，`links_rebuild` 一键修复 | `database/003`、`link_service::rebuild` |
| 工程目录分层（UI/应用/领域/仓储/基础设施） | ✅ 与文档 §31 基本一致 | `docs/ARCHITECTURE.md` |

### 2.2 缺失（按对下一代的重要性排序）

| 缺失项 | 文档章节 | 影响 | 补齐成本 |
| --- | --- | --- | --- |
| `Source`（来源）与来源可靠性 | §23.4 | 没有它，Knowledge Confidence 无从计算，冲突/过时判断缺少权重 | 低（frontmatter + 一列 + 元信息栏展示） |
| 类型化 `Relation`（SUPERSEDES / CONTRADICTS / DERIVED_FROM…） | §13 | 现在链接只有 wikilink/markdown/url，"谁取代了谁""谁和谁矛盾"表达不出来 | 中（新表 + 语法 + 图谱边样式） |
| `Lifecycle` 状态机（ACTIVE / UPDATED / SUPERSEDED / ARCHIVED） | §14 | 无法标记"已过时"，Freshness 判断没有落点 | 中（status 扩展 + `superseded_by` + `valid_until`） |
| `Decision` 对象（decision/reason/alternatives/evidence/superseded_at） | §12.3 | 项目决策是个人知识库里**复用价值最高**的一类内容，目前只能当普通笔记 | 低（约定 frontmatter + 视图） |
| `Fact` / `Entity` / `Event` | §12 | 结构化事实与实体的抽取、检索 | 高（先不做） |
| `Evidence` 与分块（chunk） | §19/§24/§27 | 没有"证据"这一层，引用只能到笔记级，Context 预算无法精细化 | 中（chunks 表 + 标题感知分块 + 引用定位） |
| `DecisionEngine` / Provider 架构 | §20/§21 | 业务直接调模型 = 被单一供应商锁死 | 中（trait + 规则 Provider 先行） |
| `DecisionCache` | §45 | 重复调用、成本与实验能力 | 低（一表 + 键设计） |
| `Decision Trace` | §29 | 可信度与可解释性（用户看到"为什么"） | 中（追加型记录 + UI） |
| 4 层 Confidence | §23 | 排序与阈值判断的输入 | 中（见 §4.4 的可计算来源） |
| Agentic Retrieval（证据充分性 + 查询扩展） | §25/§37 | 多轮检索，V2.2 目标 | 高（依赖上面全部） |

## 3. 对 V2.0 文档的技术判断

**认同（不需要改）**

- "AI 不应成为底层依赖、应是可插拔的决策与推理能力"——与现有分层、`Provider` 思路一致；
- "V1.0 不引入 Vector DB / Jev / Agent，只在数据模型和架构层预留接口"——我们正是这么做的；
- "Knowledge ≠ Decision，Decision 是 Query+Context+Knowledge+Policy+Model 的动态结果"——
  我们已经用"可重建派生数据"这一模式验证过（`note_links`），把它扩展到 Decision 层是自然延伸；
- §18"不要在 1 万笔记 / 10 万 chunk 规模提前上向量数据库"——**与实测一致**：
  我们 3000 篇的 `graph_get` 全量 96 ms、`links_rebuild` 513 ms，SQLite 完全够用。

**需要修正/补充（本文的增量价值）**

1. **向量检索不要绑 C 扩展**。文档只说"之后再加 Vector"。若照常见做法接 `sqlite-vss` /
   `sqlite-vec`，我们必须为 macOS(x64/arm64) + Windows(x64) + Linux 各带一份动态库，
   与"单文件安装包 + 免运维"冲突。建议：**先上纯 Rust 的暴力检索**（见 §4.1），量级不够再换 HNSW。
2. **Confidence 分层要落到可计算来源**。文档给了 4 层，但没说每层怎么来。我们调研后的可行映射见 §4.4；
   关键是：**只有 Decision Confidence 来自模型**，其余三层必须由我们自己的检索、元数据与规则产生。
3. **Jev 的定位要按事实收紧**。文档把 Jev 当"Decision Intelligence Provider"，方向正确，
   但依据我们核实（TypeSafe AI 官网/博客）：Jev 是**云端 early access**、只输出类型化概率决策、
   70–500 ms、输入 $0.042/MTok、输出不计费、**不生成字符串**。
   因此"Jev 可替换"不能只是口号：必须有**离线可用的 RuleProvider 兜底**，
   否则一次网络/配额问题就让"检索→判断"链路不可用，违背本地优先。
4. **V1.0"不该做"清单里应补一条**：不要在没有 `Source` 与 `Relation` 的情况下做"冲突/过时"判断——
   没有来源可靠性作为权重，冲突检测会大量误报。
5. **文档缺少评估方法**（只有 §39-43 的 PoC 大纲）。个人知识库没有公开 ground truth，
   建议自建 100 条标注样本 + 5 个任务的小型评测集（见 §4.6），否则"Jev 有没有用"无法回答。

## 4. 关键技术调研

### 4.1 向量检索：三种落地方案对比（含推荐）

前提：我们的 SQLite 是 `rusqlite bundled`（引擎编进二进制），要加载扩展就得为每个平台单独分发动态库。

| 方案 | 优点 | 缺点 | 结论 |
| --- | --- | --- | --- |
| `sqlite-vec`（C 扩展） | 检索快、SQL 内完成 | 每平台一份 `.dylib/.dll/.so`，签名与加载路径复杂；与免运维冲突 | 二期备选 |
| 纯 Rust 暴力检索（BLOB + f32 + rayon） | 零新依赖、零打包成本；10 万 chunk 量级可接受 | 量级再大需要近似算法 | **推荐先做** |
| 纯 Rust HNSW（`hnsw_rs` / `instant-distance`） | 千万级仍可 | 新依赖、索引需持久化与重建策略 | 二期备选 |

暴力检索的量级估算：10 万 chunk × 768 维 × 4 B = 约 300 MB 原始向量；
单次全量余弦相似度约 7,700 万次乘加，单线程约 80–150 ms，`rayon` 8 线程约 15–30 ms。
对"按需检索 + DecisionCache + 每轮 Top 30"的交互式场景足够，且**先把正确性做对**。

存储建议：向量放在 `.knowledge/index/vectors/`（与现有 `index/` 同级），
**派生数据、不进备份**，`rebuild` 从正文重算；chunk 元数据（note_id、标题路径、字符区间、hash）放 SQLite。

### 4.2 嵌入模型（本地优先）

| 选项 | 形态 | 备注 |
| --- | --- | --- |
| Ollama / llama.cpp HTTP（`/api/embeddings`） | 本地服务 | **与现有"本地 LLM 走 HTTP"的约定一致**，不增加打包体积；用户需自行安装 |
| `fastembed-rs`（ONNX Runtime） | 进程内 | 体验最好，但要带 ONNX Runtime 二进制（每平台 10–20 MB）与模型文件 |
| `candles`（纯 Rust） | 进程内 | 无外部二进制，但 CPU 推理慢、模型格式转换麻烦 |

中文场景候选模型（按体积/质量权衡）：`bge-small-zh-v1.5`（512 维，512 上下文，~95 MB）、
`bge-m3`（1024 维，8k 上下文，~2.2 GB，质量最好但重）、`multilingual-e5-small`（384 维，多语言）。
建议默认 `bge-small-zh-v1.5`，把模型名做成配置项；**首个版本只做 Provider，不做内置模型**。

### 4.3 分块（Chunking）——被文档低估的前置条件

`Evidence` / `Citation` / `Context Budget` 全都依赖 chunk。建议：

1. 以 Markdown 标题层级切块（我们已有 `parseOutline`，复用即可），
2. 单块目标 300–600 字，超长按段落二次切分，相邻块重叠 40–60 字；
3. 每个 chunk 记录：`note_id / chunk_index / heading_path / char_start / char_end / content_hash`；
4. **引用回到原文**：`char_start..char_end` 让 UI 能直接滚动定位（编辑器已有 jump 能力）；
5. chunk 是派生数据，正文改动 → 该笔记的 chunk 重建（与 `note_links` 同一时机）。

### 4.4 四层 Confidence 的可计算来源（本文补充）

| 层 | 来源 | 计算方式（建议） |
| --- | --- | --- |
| Retrieval Confidence | 我们自己的 FTS | BM25 分数分布 + 命中词覆盖率（标题命中加权）→ 归一到 0–1 |
| Decision Confidence | **Jev 输出**（或本地模型/Rule 的确定性映射） | 直接采用其校准概率；RuleProvider 给出固定档位（如 0.5/0.7/0.9） |
| Evidence Confidence | 我们自己 | 证据数量、覆盖的查询子主题数、来源可靠性均值、冲突扣分 → 规则公式 |
| Knowledge Confidence | 用户/元数据 | `source` 类型（官方文档/书籍/实测/个人推测…）+ 用户手动评分 + 时间衰减 |

最终排序分（Context Engine 用）：
`value = relevance × utility × freshness × sourceReliability × decisionConfidence / tokenCost`。

### 4.5 Jev 集成的工程契约（可验证的部分 + 需要补充的部分）

**已核实**（TypeSafe AI 官网与发布博客，2026-09-15）：类型化输出、校准置信度、并行采样、
不生成字符串、early access、输入 $0.042/MTok、输出免费、官方列举的用途就是
"分类/路由/打分/抽取/分支 + score/judge/verify/guardrail"。

**尚未核实**（因此必须在 PoC 第一步做契约探针，不能先写死代码）：
接口形态（HTTP/JSON schema 声明格式）、鉴权与配额、批量上限、schema 复杂度上限、
置信度是否按任务校准、数据保留与合规策略、SLA、是否提供私有化。

**据此的工程约束**：

1. `DecisionEngine` 只依赖 trait，`JevProvider` 是四个实现之一（其余：`RuleProvider`、`LocalModelProvider`、`FutureProvider`）；
2. 每次决策写入 `DecisionCache`：`query_hash / knowledge_id / decision_type / provider / model / schema_version / score / confidence / created_at / expires_at`；
3. **不外发正文**：默认只发 chunk 摘要 + 元数据（标题、时间、来源、可靠性），
   外发前把 `note_id` 换成当次随机会话别名，回答里再映射回来；
4. 每次外发在 Decision Trace 里记录"发了什么（字段清单）+ 大小"，用户可以审计；
5. Jev 不可用/超配额 → 自动降级 RuleProvider，并在 UI 标注"未使用决策模型"。

### 4.6 评估方法（文档缺的部分）

个人知识库没有现成 ground truth，建议自建小样本：

1. 采集**你自己的 500–1000 条真实笔记** + 30–50 个真实问题；
2. 对每个问题，人工标注哪几条是"该用的证据"（这是 100–200 条 query-knowledge 对）；
3. 跑四个通道对比：`Rule` / `FTS-only` / `LLM(小模型)` / `Jev`，各自输出 USE/DROP 集合；
4. 指标：Precision@k、Recall@k、F1、端到端延迟、每千次调用成本、**校准误差（置信度 vs 实际正确率）**；
5. 结论口径不看"哪个更聪明"，而看文档 §43 的 8 条：是否减少无效 Context / 减少 token /
   减少错误引用 / 发现过时 / 识别冲突 / 提升答案质量 / 降低延迟 / 置信度是否稳定。

## 5. 下一代路线图（N1–N8，与文档 Phase 对齐）

| 编号 | 内容 | 是否需要 AI | 依赖 | 价值 |
| --- | --- | --- | --- | --- |
| N1 | `Source` + 来源可靠性 + `valid_until` 元数据（frontmatter/列/元信息栏/筛选） | 否 | — | Knowledge Confidence 的地基 |
| N2 | 类型化关系表 `note_relations`（SUPERSEDES/CONTRADICTS/DERIVED_FROM…）+ 图谱按关系着色 | 否 | N1 | 过时/冲突判断的输入 |
| N3 | 生命周期状态（ACTIVE/SUPERSEDED/ARCHIVED）+ `superseded_by` + "已过时"角标 | 否 | N2 | Freshness 的落点 |
| N4 | `Decision` 对象约定 + 视图（决策列表、理由、证据、替代方案） | 否 | N1 | 高复用价值的结构化知识 |
| N5 | chunk 索引 + 引用定位（标题感知分块、字符区间） | 否 | — | Evidence/Citation/Context 预算的前置 |
| N6 | `DecisionEngine` 骨架 + `RuleProvider` + `DecisionCache` + `Decision Trace`（**不接任何模型**） | 否（骨架） | N1–N5 | 把"形状"做真，接口冻结 |
| N7 | 检索 Provider 化：BM25 + 可选向量 Provider（Ollama 嵌入 + 纯 Rust 暴力检索）+ RRF 融合 | 是（可选） | N5 | 召回率提升，可离线 |
| N8 | Jev PoC：契约探针 → 单任务（Relevance）→ A/B 评估 → 三任务（Utility/Freshness） | 是 | N6/N7 | 用数据决定是否投产 |
| N9 | 证据充分性 + 查询扩展（Agentic Retrieval 雏形） | 是 | N8 | 文档 V2.2 目标 |
| N10 | **研究助手**：对话式深入调研 → 来源分级/交叉验证 → 报告 → 确认后写入知识库（规格见 AI_ARCHITECTURE_PLAN §9） | 是 | N1（已有）、N5、N6、N9 + **联网闸门**（新增） | 把「检索」升级为「研究」，并让成果直接沉淀成本地知识 |

**推荐下一轮先做 N1 + N2**：它们不需要任何模型，能把"来源与关系"这两块知识资产补上，
同时为后面所有判断类能力提供输入；做完后再做 N5/N6（骨架），最后才是模型与 Jev。

## 6. 风险与未决问题

| 风险 | 说明 | 缓解 |
| --- | --- | --- |
| Jev 依赖 | 云端 early access、无公开 SLA、定价与数据策略未核实 | Provider 化 + RuleProvider 兜底 + DecisionCache；先 PoC 再投产 |
| 云端 vs 本地优先 | 与产品原则冲突 | 默认关闭、逐次授权、只发片段、可审计 |
| 过度设计 | 一次上 Fact/Entity/Event/Agent 会拖垮节奏 | 严格按 N1→N9 顺序，每步可独立交付并测试 |
| 评估缺 ground truth | 无法证明"AI 有用" | §4.6 的标注样本；先小规模、可复现 |
| 中文嵌入质量/体积 | bge-m3 太大、小模型召回有限 | 默认小模型 + 可切模型；与 BM25 混合而非替代 |
| 向量索引体积 | 10 万 chunk ≈ 300 MB | 派生数据、可重建、不进备份、可一键删除 |

## 7. 需要确认的决策点

1. **N1/N2 是否现在就做**（我建议：是，下一步就做，且不涉及 AI）；
2. Jev 的**接口与访问方式**你手上是否已有（early access 账号 / API 文档）？
   有的话我按真实 schema 写 Provider；没有的话我先做 §4.5 的契约探针骨架；
3. 本地 LLM/嵌入你倾向哪种形态：**Ollama HTTP（零打包成本）**还是**进程内 ONNX（体验优先但包更大）**？
4. 向量索引是否接受"默认不随备份、可一键重建"（我建议是）。
