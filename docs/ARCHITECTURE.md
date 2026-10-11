# 系统架构

对应设计文档第 8 章「系统总体架构」。原则保持不变：**模块化单体 + 本地优先 + 分层**。

## 1. 分层与目录映射

| 层 | 职责 | 代码位置 |
| --- | --- | --- |
| 表现层 | 界面与交互，不直接访问 SQLite 与文件系统 | `src/` |
| 命令层 | Tauri 命令，参数校验与线程调度 | `src-tauri/src/commands/` |
| 应用层 | 编排业务流程（工作区、笔记、分类、标签、搜索、导入、导出、备份、双向链接、附件、模板、图谱、剪藏、关系、历史；**AI 策略闸门 / 只读工具 / 研究助手 / 本地模型**） | `src-tauri/src/application/` |
| 领域层 | 数据结构与不变量，不依赖框架 | `src-tauri/src/domain/` |
| 仓储层 | 隔离 SQL 细节（trait + SQLite 实现） | `src-tauri/src/repositories/` |
| 基础设施层 | SQLite、Markdown 存储、FTS5、文件监听、备份、frontmatter、docx 生成 | `src-tauri/src/infrastructure/` |
| 通用能力 | 错误、ID、哈希、时间、文本、路径、日志、事件 | `src-tauri/src/core/` |

```text
React UI (src/)
   │  invoke(command)
   ▼
commands/            ← 只做转发；阻塞任务交给 async_runtime::spawn_blocking
   ▼
application/         ← NoteService / CategoryService / TagService / SearchService / BackupService /
                       LinkService / AttachmentService / TemplateService / GraphService / IndexService /
                       WorkspaceService / ClipService / RelationService / HistoryService / ImportService / ExportService
  application/ai/    ← AiPolicyService（四道闸门 + 审计）/ AiTools（只读工具）/ ResearchService（五步调研）
                       RuleDecisionProvider（规则档打分）/ OllamaModel（本地模型）/ source_grade /
                       web_search（联网检索）/ template_provider（无模型时的报告模板）
   ▼
domain/  +  repositories/  +  infrastructure/
   ▼
Markdown 文件 + SQLite（WAL）+ FTS5 索引 + 回收站
   + `.knowledge/`：history（版本快照）/ research（研究会话）/ templates / logs/ai-audit.jsonl
```

## 2. 关键运行时对象

- `state::AppInner`
  - `data_dir`：跨平台应用数据目录（全局注册表 `workspaces.json`、日志）
  - `registry`：最近打开的工作区（`WorkspaceRegistry`）
  - `active`：当前工作区上下文 `Arc<WorkspaceContext>`
  - `events`：事件出口 `Arc<dyn EventSink>`
- `application::context::WorkspaceContext`
  - `workspace`、`paths`（`WorkspacePaths`）
  - `db: Arc<Db>`（单连接 + `Mutex`，WAL）
  - `storage: Arc<MarkdownStorage>`（原子写入 + 路径安全 + 回收站）
  - `settings: RwLock<AppSettings>`
  - `watcher: Mutex<Option<WatcherHandle>>`
  - `index_error` / `index_rebuilding`：索引运行状态

`Db::with_conn` 借用连接并在回调中执行操作；仓储方法接收 `&Connection`，
由应用层统一获取连接与开启事务。**禁止在 `with_conn` 回调内再次调用 `with_conn`**（会死锁）。

## 3. 主要数据流

### 3.1 保存笔记（设计文档 13.1）

```text
用户输入（编辑器）
  → 防抖（useAutosave，默认 1000ms）
  → invoke note_save(baseContentHash)
  → NoteService::save
      1. 读取磁盘文件并计算 sha256，与 baseContentHash 比对（不一致 → NOTE_CONFLICT）
      2. 规范化 frontmatter（保留未知字段）→ 序列化
      3. 原子写入：临时文件 + fsync + rename 替换
      4. 更新 notes 元数据（标题、分类、哈希、字数、更新时间）
      5. 更新标签关系
      6. 更新 FTS5 索引（失败只标记索引过期，正文不丢）
  → 返回 SaveNoteResult（含 indexWarning）
```

### 3.2 搜索（设计文档 18.1）

```text
关键词 → build_fts_match（清理语法；中文转双字短语；英文加前缀）
  → FTS5 MATCH + bm25(权重: note_id 0, title 6, content 1, keywords 2)
  → 过滤（分类 / 标签 / 收藏 / 时间）
  → 读取命中笔记正文（最多一页 50 条）→ 生成摘要与高亮片段
  → 排序：标题命中优先 → 相关度 → 更新时间
```

### 3.3 外部编辑（设计文档 13.3 / 19）

```text
文件系统事件（notify：FSEvents / ReadDirectoryChangesW / inotify，或 PollWatcher 兜底）
  → 防抖 600ms + 事件合并（文件粒度 + 目录粒度）
  → IndexService::apply_file_changes
      · 忽略内部目录、隐藏文件、临时文件
      · 忽略应用自身写入（RecentWrites：路径 + 哈希 + 15s 窗口）
      · 新文件 → adopt（复用 frontmatter id，目录映射到分类）
      · 已存在 → 哈希变化则更新元数据与索引
      · 删除 → 标记 missing 并移出索引
  → 推送 workspace://files-changed 事件 → 前端刷新列表并提示
```

### 3.4 备份（设计文档 20）

`VACUUM INTO` 生成数据库一致性快照 → 流式写入 ZIP（边写边算 SHA-256）→ 写入 `manifest.json` →
`backup_validate` 复算哈希与数量 → `backup_restore` 只允许恢复到空目录。

包内容（2026-10-11 起）：活跃笔记 Markdown、**版本历史**（`.knowledge/history`）、
**模板**（`.knowledge/templates`）、**回收站**（`.trash`）、附件、数据库快照与 `config.json`。
清单与校验按四类分别计数（笔记 / 附件 / 历史·模板 Markdown / 回收站 Markdown），
写入与校验共用同一套分类规则（`classify_entry`），避免「清单说 N 个、实际 M 个」自相矛盾。
不入包：`.knowledge/research`（可丢弃的中间产物）、`.knowledge/logs`（AI 审计元数据）、
`backups` 自身。

**恢复到新目录 = 同一个知识库换了位置**：`open()` 先按 `root_path` 查库里的记录；
查不到但库里已有记录时（整目录拷贝 / 从备份恢复，也是文档推荐的搬迁方式），
沿用那条记录的身份（id）并把 `root_path` 指到新位置——否则会新建 id，
快照里的行全成孤儿，**回收站、收藏这些只存在数据库里的状态会静默丢失**。
应用级「最近打开」按 id upsert，因此同一份备份恢复到多处时列表里只保留最近打开的那一个路径。

### 3.5 研究助手（V1.2.1 阶段 D）

```text
research_start  → 校验课题（过于宽泛要追问范围）→ 拆 3–5 个子问题（模板）→ 本地基线（kb_search）→ 会话落盘
research_collect→ 本地 FTS5 检索 → 按「意图标记」把材料归属到子问题 → 逐条分级（A–E）
                  └ 联网档位开启 + 本次确认 + 已配置检索源时才走外部检索（失败/未配置都只警告）
research_verify → 按子问题汇总：≥2 个独立来源才算交叉验证；正负表述对立判为「冲突」（不选边）
research_compose→ 六段报告（结论先行 / 关键事实 / 争议 / 与本库关系 / 待验证 / 参考来源）
                  └ 生成侧：本地模型（开启且可用）否则模板实现；失败自动降级并写警告
research_proposal→ 标题 / 分类 / diff / 同标题提示（不覆盖）
research_write  → 用户确认后 note_create 或 note_save（追加留历史快照）→ 补 N1 元数据
```

- 会话存 `.knowledge/research/<sessionId>.json`（可丢弃的中间产物，删掉不影响笔记）；
- 每一步都写 `research_*` 审计（`capability / outbound / outcome / durationMs / note`，**不含正文**）；
- 打分由决策层给出：`credibility`（模型分与级别折算各半）、`confidence`（交叉验证折扣），
  provider 标识写进报告（`rule` / `local.ollama:<模型>`）。

## 4. 可替换性设计

| 关注点 | 边界 | 当前实现 |
| --- | --- | --- |
| 全文搜索 | `infrastructure::fts::SearchIndex` trait | `SqliteSearchIndex`（FTS5） |
| 元数据存储 | `repositories::*Repository` trait | SQLite 实现 |
| 文件正文 | `infrastructure::markdown_storage::MarkdownStorage` | 本地文件 + 原子写入 |
| 文档导出 | `application::export_service::ExportService` + `infrastructure::docx` | Markdown / HTML / Word(docx) / PDF 打印视图 |
| 外部导入 | `application::import_service::ImportService` | Markdown / 文本文件与目录，复制语义 |
| 事件出口 | `core::events::EventSink` trait | `TauriEventSink` / `NoopEventSink`（测试） |
| 文件监听 | `infrastructure::watcher` | 原生事件 + `PollWatcher` 兜底 |

因为应用层只依赖 trait，集成测试可以在没有 Tauri 运行时的情况下跑完整业务链路
（见 `src-tauri/tests/workspace_flow.rs`）。

扫描的边界（2026-10-11 补齐）：正文目录按常规采纳；收尾时另外把 `.trash/` 下
**尚未登记**的 Markdown 采纳成「已删除」笔记（`adopt_trash_notes`）——
回收站文件同样是带 frontmatter 的 Markdown，删库重建后应当能回到界面里并被恢复；
按 id / `trash_path` 判重，重复扫描不会重复登记。

## 5. 与设计文档的差异（已确认的工程决策）

1. **ULID 使用自实现（26 位 Crockford Base32）**，同毫秒内单调递增，保证字符串排序即时间排序；
   设计文档 12.3 的 `note_01JXYZ123` 形式得以保留。
2. **`notes` 表新增两列**：`favorite`（侧边栏「收藏」）与 `trash_path`（回收站位置）。
   其余字段与设计文档 11.2 完全一致，并提供迁移文件。
   其中 **`favorite` 已改为派生列**（2026-10-11）：与 `pinned` 同口径，写在笔记 frontmatter 的
   `favorite: true`（只在为真时写入），扫描 / 外部编辑时投影到列——
   此前它只存在数据库里，「删库重建」这条文档推荐的恢复手段会静默丢收藏；
   `trash_path` 仍是**库内运行态**（回收站位置由本机磁盘决定，不进 frontmatter）。
   后续迁移另加了若干**派生列**（`parent_id` / `declared_parent` / `sort_order` / `pinned`）：
   它们的唯一事实来源是笔记 frontmatter（`parent` / `order` / `pinned`），
   扫描时投影、外部编辑后重算、删库可从 Markdown 全量重建；
   其中 `declared_parent` 保存 `parent:` 的**原文**，用于「父文档还没出现」时在后续扫描轮次重试解析
   （见 [DATABASE_SCHEMA.md](./DATABASE_SCHEMA.md) 的迁移 007–010）。
3. **打开已有目录时不改写用户文件**：外部文件在首次保存时才补全 frontmatter；
   数据库通过「路径 → 记录」复用保证 id 稳定（避免每次扫描产生新 id）。
4. **正文首尾空行规范化**：frontmatter 与正文之间的分隔空行不属于正文内容。
   `tags` 按设计文档 12.3 输出为缩进块序列（`tags:\n  - sqlite`），
   未知字段由 `serde_yaml` 原样保留（含嵌套结构）。
5. **轮询兜底**：受限环境（沙箱、部分网络盘）收不到原生事件时自动改用 `PollWatcher`（3 秒间隔），
   并在状态栏与提示中说明，保证「外部编辑」场景在任意环境可用。
6. **路由使用 HashRouter**：桌面外壳以静态资源方式加载单页应用，哈希路由避免刷新 404。
7. **Inbox 不是分类**：`00_Inbox` 目录代表「未分类」，扫描已有目录时不会为它创建分类记录。
8. **导入 / 导出作为独立应用服务**：`application::ImportService` 负责把工作区外的
   Markdown / 文本（或整个目录）复制进知识库并保留 frontmatter 元数据；
   `application::ExportService` 负责单篇导出，其中 Word 由
   `infrastructure::docx` 生成 OOXML（不依赖外部工具），PDF 走「打印视图 + 系统打印面板」，
   避免引入重量级排版引擎与字体嵌入。
9. **设置数值宽容解析**：`UpdateSettingsInput` 的像素/延迟字段同时接受整数与小数并四舍五入，
   以兼容 Retina 屏下拖拽产生的浮点坐标（`clientWidth`/`clientX` 可能是小数）。
10. **双向链接是「可重建派生数据」**（V1.1 M1）：`note_links` 表不参与正文存储，
    完全可由 Markdown 正文重建（`links_rebuild`）。写入时机与 FTS 索引一致（保存 / 导入 /
    外部修改 / 创建 / 恢复 / 重命名 / 删除），因此链接与正文不会出现长期漂移；
    重命名会同步改写来源笔记正文里的 `[[旧标题…]]`，保留别名与锚点。
    目标解析顺序：标题（大小写不敏感）→ 工作区相对路径 → 文件名 → 笔记 id。

### 5.1 双向链接的前端结构（V1.1 M1-3）

```text
linkService (src/services/linkService.ts)
  → commands: links_get_backlinks / links_get_outgoing / links_get_unresolved /
              links_get_counts / links_rebuild
  → 组件：BacklinkPanel（编辑器底部）、UnresolvedLinksCard（首页）
浏览器预览：src/services/mock.ts 用与 Rust 端一致的规则在内存中解析 [[…]]，
            反链面板、引用计数与未解析链接在 `pnpm dev` 下同样可用。
```

链接读取失败时组件降级为空列表并在控制台留痕，不阻塞编辑（链接只是派生数据）。

编辑器侧的 `[[` 补全与预览 chip（V1.1 M2-2）：

```text
MarkdownEditor
  → autocompletion(override: [wikilinkCompletionSource(getTitles)])   ← 标题候选来自 noteService.list
MarkdownPreview
  → preprocessWikilinks：[[标题|别名]] → [别名](#wikilink/<目标>)
  → a 渲染器识别 #wikilink/ 前缀 → chip（已解析跳转 / 未解析一键创建）
```

解析语义集中在 `src/lib/wikilink.ts`：Rust 的 `core/text.rs`、浏览器预览后端与前端
共用同一套规则（`[[标题]]`、别名、锚点、转义、忽略代码区域），避免多处实现漂移。

### 5.2 附件读写（V1.1 M2-1）

```text
编辑器粘贴/拖入图片
  → 前端读取为 data URL
  → invoke attachment_save { noteId, fileName, mimeType, dataBase64 }
  → AttachmentService：Base64 解码 → attachments/YYYY-MM/ 原子写入
  → 返回 linkPath（相对当前笔记目录）→ 插入正文 ![](linkPath)
Markdown 预览
  → img src 是工作区相对路径时 invoke attachment_read → data URL 内联显示
```

约束：单文件 20 MB、单张预览内联 8 MB；`attachment_read` 只接受工作区内的相对路径，
越界路径（`../` 越出根目录）与外部 URL 一律拒绝。附件是普通文件，不进入 `notes` 表。
Base64 编解码由 `core::base64` 自带实现（避免为单点需求引入新依赖）。

### 5.3 本地模板（V1.1 M2-2）

```text
.knowledge/templates/*.md
  → templates_list   列出模板（名称 / 首标题 / 大小，跳过 {{…}} 形式的标题）
  → template_render  替换 {{title}} {{date}} {{time}} {{datetime}} {{weekday}}
  → 前端把渲染结果填入「新建笔记 → 初始内容」
  → 仍走 note_create 写入（索引 / 链接 / 统计路径保持唯一）
```

模板只读取 `.knowledge/templates` 下的 Markdown，模板名禁止包含路径分隔符与 `..`，
单文件上限 1 MB；新建工作区时会写入一个示例模板，用于让该能力可被发现。

### 5.4 知识图谱（V1.1 M3）

```text
graph_get
  → GraphService：
      节点 = SqliteNoteRepository.list(分类/标签/关键词过滤)
      边   = SqliteLinkRepository.list_resolved（仅已解析、去掉自链接）
      → 聚焦（N 跳 BFS）→ 孤立节点开关 → 度数优先截断（2000/5000）
      → 按可见边集重算邻居数 → 附加标签（tag_names_for_notes）
  → 前端 /graph：self-made Canvas 力导向（网格近邻斥力 + 弹簧 + 向心力）
```

图谱**不落库**，与反链面板同源（都由 `note_links` 派生），因此 `links_rebuild` 即可整体修复。
前端不引入第三方可视化依赖，布局收敛后停止 `requestAnimationFrame`，避免大库常驻耗电。

### 5.5 AI 决策层与研究助手（V1.2.1 阶段 D）

```text
命令层 commands/{ai,research}_commands.rs        ← 只转发
   ▼
application/ai/policy.rs      AiPolicyService    ← ★ 唯一的闸门判定点 + 审计
   │   ① 总开关（默认关）② 联网档位（默认「仅本地」）+ 本次确认 ③ 抓取正文开关
   │   ④ 域名黑名单（强制丢弃）/ 白名单（非空才生效）
   ▼
application/ai/tools.rs       AiTools            ← 只读：kb_search / note_read / web_fetch / web_search
   ▼
application/ai/research.rs    ResearchService    ← 五步流程 + 会话落盘 + 报告渲染
   ├─ source_grade.rs         A–E 分级（纯函数：域名内置表 → 内容自证 → 兜底「弱」）
   ├─ web_search.rs           检索源模板 {query}/{limit} + 三种 JSON 形状 + 注入判定
   ├─ template_provider.rs    无模型时的报告模板（文本自带「未接模型」标注）
   └─ local_model.rs          OllamaModel：GenerativeModel（生成）+ DecisionModel（打分）
                              ← 只连本机；开关默认关；失败自动降级
   ▼
application/ai/traits.rs      DecisionModel / GenerativeModel / Retriever  ← 接口预留（Jev 的插入点）
```

边界与不变量（与 [AI_ARCHITECTURE_PLAN.md](./AI_ARCHITECTURE_PLAN.md) §9.7 一致）：

1. **工具只读**：任何写库都必须回到「建议 + 用户确认」再走常规业务命令，AI 不直接写；
2. **默认不外发**：没开总开关 / 没开联网档位 / 没确认时，外部工具返回 `AI_DISABLED` / `NETWORK_DISABLED`
   且**不发起任何请求**（测试用「指向不可达地址仍只返回闸门错误」来证明）；
3. **网页内容是数据不是指令**：抓取结果剥 HTML、限长，命中明显的提示词注入特征即**丢弃**该页；
4. **审计不含正文**：只记时间 / 能力 / 是否外发 / 结果 / 耗时 / 备注；
5. **可降级**：没有 Jev 用规则表（`RuleDecisionProvider`），没有模型用模板实现，
   因此「调研 → 报告 → 写回」在完全离线的机器上也能跑通。
