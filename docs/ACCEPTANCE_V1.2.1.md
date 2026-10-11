# V1.2.1 验收清单（macOS 为主，Windows / Linux 对照）

> 生成时间：2026-10-11。**产物状态：按你的指令「打包暂停」，因此本文件不含安装包校验**；
> 已经完成的是「本机可执行」的自动化验收与性能实测（下表全部有命令与数字），
> 产物完整性与真机人工项列成待执行清单，恢复打包后按 F / E 两节逐条勾选即可。
>
> 标注说明：**✅ 已自动验证**（本机可执行，含证据）；**⬜ 需打包后 / 需真机**（括号内为原因）。
>
> 版本号口径：代码内容是 V1.2.1，但四处的版本号仍是 **1.2.0**（`package.json` /
> `src-tauri/Cargo.toml` / `Cargo.lock` / `tauri.conf.json`）；升版与产物一起在恢复打包时执行
> （见 [PLAN_V1.2.1.md](./PLAN_V1.2.1.md) 阶段 A1）。

## A. 回归矩阵（✅ 已自动验证）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| Rust 全量测试 | ✅ 39 个测试目标全绿（库单测 116 项 + 1 项 ignore 的基准 + 集成测试） | `cd src-tauri && cargo test --offline` |
| 前端类型检查 | ✅ 无错误 | `npx tsc --noEmit` |
| 前端 ESLint | ✅ 无告警 | `npx eslint .` |
| 前端单元 / 组件测试 | ✅ 52 个文件、273 用例全通过 | `CI=true pnpm test` |
| 前端生产构建 | ✅ 构建成功 | `CI=true pnpm build` |
| 端到端主流程 | ✅ 1 条主流程通过（34 个编号步骤 + 补充断言，浏览器预览后端） | `CI=true pnpm test:e2e`（截图见 `docs/screenshots/`）；重负载下时序敏感，配置了 1 次重试与 10s 断言超时（原因写在 `playwright.config.ts` 注释里） |

关键断言（对应 V1.2.1 新增能力的「可测证据」）：

| 能力 | 断言 | 测试文件 |
| --- | --- | --- |
| 扫描 / 监听并发安全 | 扫描与监听同刻采纳同一批 400 篇、两线程同扫、边扫边建 120 篇：均无 UNIQUE 冲突 | `tests/concurrency_scan_flow.rs` |
| 图谱大库 | 3 万篇全量查询 < 1.5s 红线；节点总数不再被截断；抽样保留 3708 条边 | `tests/graph_bench.rs`（断言）+ `tests/graph_flow.rs` |
| 网页剪藏 | 抓取 → 转 Markdown → 写 `source_type: web` 与来源地址 | `tests/clip_flow.rs` |
| 网页剪藏 · 非 UTF-8 页面 | Content-Type 声明 GBK / **只在 `<meta charset>` 声明** / 完全不声明（UTF-8 严格失败后按 GBK 兜底）三种情况都要正确解码；错误信息不再把「编码问题」误报成「超过 3 MB」；抓取→写笔记的 `source_type: web` + 来源地址由前端用例覆盖 | `tests/clip_flow.rs`（5 项，含反向验证）+ `NewNoteDialog.test.tsx` |
| 导入非 UTF-8 文件（GBK 文本 / CSV） | 共用 `core::encoding::decode_text`（BOM → 严格 UTF-8 → GBK 兜底）：GBK 的 `.txt` / `.csv` 解对不出现乱码，且**如实提醒**「不是 UTF-8，已按 GBK 解码」（导入报告 + 成功路径的 toast）；UTF-8 文件不产生噪声提醒 | `tests/import_flow.rs`（13 项，含 2 项本项）+ `core::encoding` 单测（4 项）+ `NewNoteDialog.test.tsx`（+1，均反向验证过） |
| 扫描 / 打开工作区的提醒可见 | 后端 `warnings`（跳过无法读取的文件、重建链接失败…）在三条路径上必须可见：工作区打开/新建/切换、外部文件变化事件、设置页「扫描」；无提醒时不发提示。另：**GBK 的 `.md` 直接拷进工作区会被明确跳过并告警**（不静默入库，转正请走「导入」） | `src/lib/warnings.test.ts`（3 项）+ `src/stores/workspaceWarnings.test.ts`（2 项，反向验证）+ `tests/external_interference_flow.rs`（7 项，含本项） |
| 保存状态诚实性（降级可见） | `note_save` 成功但索引更新失败时，状态徽标显示琥珀色「已保存（检索索引待重建）」+ 原因可悬停，**不再与正常保存长得一样**；空白原因不降级；保存失败仍按失败展示 | `SaveStatusBadge.test.tsx`（4 项，反向验证过） |
| 加载失败不误导（关系面板） | 新增关系里搜索目标笔记失败时显示「读取笔记失败：<原因>」+ 重试，**不再显示成「没有匹配的笔记」**；确实没结果时才显示空态 | `RelationsPanel.test.tsx`（4 项，含本项，反向验证过） |
| 加载失败不误导（待复核 / 审计 / 备份列表） | 首页「待复核」失败 → 卡片显示「读取失败 + 重试」而不是**整块消失**；设置 → AI「最近行为」失败 → 报错而不是「还没有 AI 行为记录」；设置 → 备份列表失败 → 报错而不是「暂无备份文件」 | `ReviewDueCard.test.tsx`（+2）/ `AiSettingsPanel.test.tsx`（+1）/ `SettingsPage.test.tsx`（2 项，均反向验证过） |
| 加载失败不误导（反链面板 / 关于页） | 编辑器反链读取失败 → 「读取反向链接失败 + 重试」且头部显示「读取失败」（不再显示「还没有笔记引用这篇」/「被 0 篇引用」）；关于页 `app_info` 读取失败 → 说明原因，`—` 不冒充「不知道」 | `BacklinkPanel.test.tsx`（+1）/ `AboutPage.test.tsx`（2 项，均反向验证过） |
| 版本历史命名版本 | 保存 / 命名 / 改名 / 清除与还原 | `tests/history_flow.rs` |
| 单篇复制到分类 | 副本带「副本」后缀、原作保留、分类正确 | `tests/batch_flow.rs` |
| AI 闸门 | 默认口径下 `web_fetch` 指向不可达地址只返回 `AI_DISABLED` / `NETWORK_DISABLED`（**未触网**）；放行后真实抓取并记 `outbound=true` | `tests/ai_policy_flow.rs` |
| 研究助手闭环 | 六段报告 + 「资料截至」+ 单一来源不进结论段 + 全程 `outbound=false`；写入默认 Inbox / 同标题不覆盖 / 追加留可回滚快照 / 放弃零写入；**提案前不碰工作区、写入后文件即事实来源且立刻可检索、每一步都被闸门挡住**；**引用可点开**（第 6 段本地来源 `[[标题]]`、外部来源裸 URL，且不再出现点不开的 `note://`） | `tests/research_flow.rs`（15 项） |
| 联网检索 | 仅本地口径下检索源**一个请求都不发**；放行后分级与预算生效、疑似注入页丢弃；未配置检索源不外发；**检索源本身受域名白名单约束**；**黑名单 / 白名单之外的检索结果逐条丢弃**并在报告警告块点名（附录可见）；外部来源在报告里保持可点开；**E 级（不可用）来源只作线索**（不算交叉验证、不进结论段引用，只有 E 级时如实写明「不可用线索」） | `tests/web_search_flow.rs`（8 项） |
| 待复核提醒 | 0 / 14 / 365 天三档窗口、过期天数、无法解析的时效字段跳过、回收站不提醒 | `tests/review_flow.rs`（2 项） |
| **删库可全量重建 / 备份可恢复 / 导入整库的派生数据** | 删掉 `database.sqlite` 后重开：笔记 id 稳定、分类 / 标签 / 置顶 / **收藏** / 手动顺序 / 父子层级 / 来源与时效 / 双向链接 / FTS 全部从 Markdown 复原；**`.trash/` 里的回收站条目也会被登记回来（可恢复、幂等）**；外部新增按标题声明父文档的子文档一次扫描即生效；一次导入 61 篇（走批量路径）时层级（id 与标题两种声明）、order、pinned、来源元数据、标签、反链、类型化关系、FTS 全部正确；父文档改名后按标题声明的子文档同步改名、重建后层级仍在；**备份 → 恢复到空目录 → 打开后，层级 / 置顶 / 顺序 / 来源与时效 / 标签 / 反链 / FTS 一并完整** | `tests/rebuild_flow.rs`（6 项） |
| 收藏 = frontmatter 派生列 | 「收藏 / 取消收藏」写回 `favorite: true`（取消即移除字段）；外部手改 `favorite: true` 扫描后生效；`note_save` / 来源更新 / AI 追加写入都保留该字段 | `tests/metadata_preservation_flow.rs`（4 项，含本项，反向验证过） |
| 备份包内容与「恢复即同一知识库」 | 包内含笔记 / 版本历史（命名版本）/ 模板 / **回收站** / 附件 / 数据库 / 配置；恢复到新目录后打开：**沿用原知识库 id**（不产生孤儿行）、命名版本、模板、回收站里的笔记、附件、**收藏**（只存在数据库里的状态）全部回来；写入与校验共用同一套条目分类（四类分别计数） | `tests/rebuild_flow.rs`（含本项，反向验证过） |
| 决策层打分 | 来源可信度 ∈ (0,1]、交叉验证结论置信度高于单一来源；注入桩 `DecisionModel` 可改变打分（A/B 通路） | `tests/research_flow.rs` |
| 本地模型接入 | 开启后可信度由模型分决定且报告写明 provider；连不上自动降级且流程不中断；开关关闭时零请求；远程地址被拒绝（**写入侧与读取侧各校验一次**：即使有人直接改了库里的地址，也不会把笔记内容发到远程） | `tests/local_model_flow.rs`（4 项） |
| 文档与代码一致性 | README / ARCHITECTURE / 安装指南 / 发布流程的版本、能力表、命令与体积都按当前代码校正；文档内引用的相对路径全部存在（脚本化检查） | 本文件 + `git log` 的 docs 提交 |
| 命令契约 | 前端 104 条命令与 `lib.rs` 注册表、mock 分支三方对齐；**参数名也对齐**（77 处带参调用 0 不一致）；守卫两次反向验证（插入未注册命令 / 改错参数名 → 立即失败） | `tests/command_contract.rs`（3 项） |
| IPC 契约 | ① 响应结构：界面读取的 **29 个 DTO**，TS 字段必须存在于 Rust 结构（238 字段 0 不一致）；② **事件名** 3/3 一致；③ **错误码** 22/22 集合相等。三条都反向验证过（故意制造漂移 → 立即失败） | `tests/dto_contract.rs`（13 项） |
| 可访问性（程序化标签） | 全部 `Input` / `select` / `textarea` 都有 `id` / `aria-label` / 被 `<label>` 包裹（本轮补齐 25 处）；无标题图标按钮与缺 `alt` 图片均为 0；守卫反向验证过 | `src/lib/a11yLabels.test.ts` |
| 快捷键 | 语义：无处理器的快捷键**不被吞**（⌘S/⌘E 由编辑器页面实现）；行为：⌘S 在自动保存延迟 3000ms 下仍能立即保存（E2E 断言窗口 1500ms）、⌘E 能把焦点送回编辑器；Delete 在输入框内不触发 | `src/hooks/useKeyboardShortcuts.test.ts`（4 项）+ E2E 第 5b 步 |
| 文件监听降级 | 原生事件不可用时自动降级为轮询（3 秒）——用测试开关强制「探测通过但启动失败」，断言返回 `WatchMode::Polling` 而不是直接失败；线程创建失败也返回错误而非 panic | `src/infrastructure/watcher.rs` 单测（3 项） |
| 异步竞态 | 快速切换笔记 / 连续输入检索词时，先发出的响应后到**不会**覆盖当前状态（详情、列表、检索三处）；反向验证过（撤掉守卫 → `expected 'note_a' to be 'note_b'`） | `src/stores/raceGuard.test.ts`（3 项）+ `src/lib/latestRequest.ts` |
| 防抖保存不丢改动 | 切换笔记前先 flush（顺序断言「先落盘再取新详情」）；同一篇刷新不重复触发；`discardLocalEdits` 时跳过（冲突对话框「以磁盘为准」）；E2E 实测：延迟 3000ms 下输入后立刻切走，切回来内容仍在 | `src/hooks/useAutosave.test.ts`（3 项）+ `raceGuard.test.ts`（+1）+ E2E 第 5c 步 |
| 缺失笔记可恢复 | 笔记文件被移出后标记缺失；文件放回再扫描 → 自动恢复为正常笔记、正文读回、**重新可检索**；整目录被删/被移走时不 panic、保存返回明确错误、目录放回后无需重启即可恢复 | `tests/external_interference_flow.rs`（含本项，共 6 项） |
| 恢复不覆盖他人文件 | 原路径被别的文件占用时，从回收站恢复会改用唯一路径：占位文件**原封不动**、笔记落在新路径且内容正确；`purge` 对已被外部删除的文件也能正常清干净（记录 / 索引 / 历史） | `tests/external_interference_flow.rs`（共 6 项，含本项） |
| 父文档后到也能归位 | 子文档先被采纳（`parent:` 指向当时不存在的父文档）→ 父文档在**后一轮**（整库扫描 / 文件监听两条入口都验证）出现时，层级在**同一轮内**补上，`declared_parent` 保留声明原文供重试；反向验证过（关掉重试 → 两条用例立即失败） | `tests/external_interference_flow.rs`（+2 项） |
| 预览端与后端同规则 | ① 未知分类：新建 / 单篇移动 / 批量移动都拒绝（批量逐条记失败）；② 标签规范化：去空 / 折叠空白 / 大小写不敏感去重，空名与撞名按后端错误码拒绝；③ 返回形状（`note_set_tags` → `NoteDetail`、`note_set_tag_names` → `Tag[]`）；④ `app_info` 版本读 `package.json`、schemaVersion 与后端一致；⑤ 重命名与后端同规则（标题规范化 / 按时间戳改名文件 / `renameFile=false` 只改标题 / **重连 `[[旧标题]]` 链接**）；⑥ 相对路径（`YYYYMMDD-HHmmss-标题.md`、按分类目录落位、移动分类改路径、同秒撞名追加序号）；⑦ 永久删除「列表不可见 + 详情 `NOTE_NOT_FOUND`」，删除 / 永久删除不存在的笔记报错；⑧ 模板另存为在预览端如实报「不支持」。守卫逐条反向验证过（撤掉校验 / 强制 `renameFile=false` / 关掉链接改写 / `categoryDirectory` 恒返回 `00_Inbox` → 用例立即失败） | `src/services/mockBackend.test.ts`（26 项，含以上各项） |
| 元数据保全 | `note_save` / `note_set_source` / AI 追加写入之后，frontmatter 的来源、时效、父子层级、手动顺序、置顶、类型化关系、自定义字段、标签全部保留；创建时间不被改写 | `tests/metadata_preservation_flow.rs`（3 项） |
| 分类移动（挪目录 + 改路径） | 带孙分类的子树移到顶层 / 移回：完整目录路径正确、老目录消失、子目录随行、直属与子分类里的笔记路径改写且文件真实存在、归属 id 不变、移动后仍可检索、重开扫描后一致；防环与目标不存在被拒绝且不改数据 | `tests/category_move_flow.rs`（2 项） |
| 分类目录展示 | 分类管理页副标题显示**完整**相对路径（子分类不再被误读为根目录）；未建目录时沿用父目录 | `src/lib/categoryDirectory.test.ts`（3 项） |
| 「从最近列表移除」的安全性 | `workspace_forget` 只改注册表：笔记文件、数据库、目录结构都不动，还能重新打开且笔记仍在 | `tests/workspace_registry_flow.rs`（1 项） |
| 本地只读工具边界 | `kb_search` / `note_read` 在 AI 总开关关闭时仍可用（纯本地检索），且**永不产生 `outbound=true`** 审计；空关键词报 `INVALID_INPUT`、`limit` 夹取 1–50；`quick_search` 夹取 1–30 且看不到回收站里的笔记 | `tests/local_tools_flow.rs`（3 项） |
| 前端动作反馈诚实性 | 写操作失败时**不再提示成功**：笔记条目/编辑器菜单的六类动作、删除确认（含子文档撤销按实际成功数报）、标签删除、三处「重建索引」；批量撤销如实计数（`已恢复 1/2 篇…`） | `src/lib/actionFeedback.test.ts`（3 项）+ `notesStore.filters.test.ts` 增 2 项 |
| 加载失败不误导 | 设置 → AI 读取失败显示错误 + 重试（不再永远「正在读取…」）；快速切换读取失败显示「读取笔记失败 + 原因 + 重试」（不再假装「没有匹配」） | `AiSettingsPanel.test.tsx` / `QuickSwitcherDialog.test.tsx` 各增 1 项 |
| 引用完整性（不产生悬挂引用） | 给不存在的分类：新建笔记 / 单篇移动 / 批量移动 / 导入 / **AI 写回**都明确报错，且**无副作用**（不建文件、不改路径、不改会话状态，失败后仍可改用 Inbox 重试） | `tests/reference_integrity_flow.rs`（5 项） |
| 写回后再扫描不重复 | AI 写入的报告与「来源清单」子文档，连续两轮扫描后笔记数/文件数不变、同标题只有一份 | `tests/research_flow.rs`（15 项，含本项） |

## B. 性能（✅ 已自动验证，本机 2026-10-11 实测）

### B1. 知识图谱 3 万篇（`BENCH_NOTES=30000 cargo test --offline --test graph_bench -- --ignored --nocapture`）

| 项目 | 结果 |
| --- | --- |
| 写入 3 万篇 Markdown | 1.55 s |
| 扫描采纳（含解析链接） | 14.36 s（新增 30000 篇） |
| 链接重建 | 5.19 s（链接 60000 条，已解析 60000 / 未解析 0） |
| FTS 索引重建 | 10.01 s（已索引 30001 / 共 30001） |
| **`graph_get` 全量** | **811.7 ms**（红线 1.5 s；返回 2000 节点 / 3708 边，库内 30001 节点 / 3708 边） |
| `graph_get` 分类过滤 | 324.0 ms |
| `graph_get` 2 跳聚焦 | 592.7 ms |
| `graph_get` 关闭孤立节点 | 813.8 ms |
| 体积 | `database.sqlite` 73.0 MB；`.knowledge` 合计 98.8 MB |

### B2. 中文检索 1 万篇（`BENCH_NOTES=10000 cargo test --offline --test search_bench -- --ignored --nocapture`）

| 项目 | 结果 |
| --- | --- |
| 写入 1 万篇 | 0.47 s |
| 扫描采纳（含建索引） | 4.51 s |
| FTS 重建 | 4.67 s（已索引 10001 / 共 10001） |
| 检索延迟（含摘要高亮） | 平均 18.0 ms / p95 43.5 ms（120 次） |
| 独特术语 recall@1 / @5 | 100% / 100% |
| 混合写法 `术语XXXX` recall@5 | 100% |
| 纯英文 `termXXXX` recall@5 | 100% |
| 中文四字短语命中率 | 100%（「查询优化」，40 次） |
| 体积 | `database.sqlite` 32.5 MB（每篇约 3.3 KB）；`.knowledge` 45.4 MB |

> 迁移 010（`declared_parent` + 每轮待解析声明的重试）落地后复测（2026-10-11）：
> 写入 0.49 s、扫描采纳 3.76 s、FTS 重建 4.14 s、检索**平均 18.6 ms / p95 44.4 ms**、
> 各项召回仍为 100%——与上表同一口径，**无退化**（重试只在「本轮有新增 / 更新」时执行一次
> 带索引的小查询）。图谱侧同样复测：3 万篇写入 1.56 s、扫描采纳 14.63 s、
> `links_rebuild` 5.35 s（60000 条全解析）、FTS 重建 10.01 s、**`graph_get` 全量 843.7 ms**
> （红线 1.5 s 内）。

## C. 升级与迁移（✅ 已自动验证）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 旧库（schema 5，无 rowid 映射表）打开后自动迁移并重建索引 | ✅ 已索引 == 总数 | `cargo test --offline --test upgrade_flow` |
| 子文档层级（迁移 007）在前向迁移后可用 | ✅ `parent_id` 列 + 索引存在，`parent:` 可写 id 或标题 | `tests/parent_flow.rs` |
| 父文档声明原文（迁移 010） | ✅ `declared_parent` 列 + 索引存在，重建后可由 frontmatter 复原；旧库打开时自动迁移到 schema 10 | `tests/external_interference_flow.rs`（2 项） |
| 新增 frontmatter 字段（`parent` / `order` / `pinned`）可缺省，旧库打开自动补齐 | ✅ | `tests/parent_flow.rs` / `tests/order_flow.rs` / `tests/pin_flow.rs` |
| 删库可从 Markdown 全量重建 | ✅ | `tests/workspace_flow.rs` |
| 研究会话是旁路数据（`.knowledge/research/`），删除不影响笔记 | ✅ | `tests/research_flow.rs` |

## D. V1.2.1 新功能验收（✅ 自动化部分已覆盖）

| 功能 | 交付内容 | 验收证据 |
| --- | --- | --- |
| 子文档层级 | 拖拽成为子文档 / 同层排序、父文档重命名与删除后子文档稳定 | `tests/parent_flow.rs` + E2E 第 22–26 步 |
| 置顶 | frontmatter `pinned`，所有排序下最前 | `tests/pin_flow.rs` |
| 手动排序 | `order` 写入 frontmatter，按层归一化 | `tests/order_flow.rs` |
| 版本历史增强 | 命名版本 / 保存为版本 / 改名 / 清除 | `tests/history_flow.rs` |
| 条目菜单 | 复制到分类 / 复制相对路径 / 复制标题 | `tests/batch_flow.rs` + E2E |
| 目录管理页 | 跨状态宽表 + 搜索 + 批量 | E2E 第 30 步 |
| Word / CSV 导入 | `.docx`（标题/列表/实体）与 `.csv`（RFC 4180 → 表格） | `tests/import_flow.rs` |
| 网页剪藏 | URL → Markdown + 来源 frontmatter | `tests/clip_flow.rs` |
| 扫描 / 监听并发安全 | 采纳路径幂等 + 新建笔记幂等落库 | `tests/concurrency_scan_flow.rs` |
| 图谱大库 | 抽样 + 查询提速（3 万篇 811.7 ms） | `tests/graph_bench.rs` |
| AI 闸门与审计 | 四道闸门 + `.knowledge/logs/ai-audit.jsonl`（不含正文） | `tests/ai_policy_flow.rs` |
| 研究助手 | 五步流程 + 六段报告 + 冲突并列 + 写入纪律 | `tests/research_flow.rs` |
| 研究助手 · 引用可点开（§9.9 验收 1） | 报告第 6 段本地来源 `[[标题]]` / 外部来源裸 URL；页面上「结论与引用」逐句可点（本地跳笔记、外链新窗口，悬停显示级别与获取时间） | `tests/research_flow.rs` + `tests/web_search_flow.rs` + `ResearchPage.test.tsx`（各含反向验证） |
| 研究助手 · 实时进度（§9.2） | 每一步与每个抓取页都推 `ai://research-progress`（会话 id / 步骤 / 说明 / 已抓页数 / 来源数）；**只含元数据，不含笔记正文与网页内容**；仅本地口径下 `pagesFetched` 恒为 0；页面在长步骤里显示「已抓 N 页 / 来源 N 条」 | `tests/research_progress_flow.rs`（1 项，记账型事件出口）+ `ResearchPage.test.tsx`（+1） |
| 研究助手 · 运行中可中止（§9.2 / §9.7） | 中止优先：收集在本地跑完 / 每个候选前 / 收尾保存前都以磁盘状态为准，已中止就停下并把这一步成果并回草稿（不覆盖 `aborted` 与中止原因）；验证 / 汇总 / 提案同样处理；界面忙碌时「中止」仍可点 | `tests/research_abort_flow.rs`（2 项，慢速检索源必现竞态）+ `ResearchPage.test.tsx`（+1，反向验证） |
| 导出 · HTML / PDF 真的自包含 | 正文里指向工作区内的图片在导出时内联为 `data:` URL（HTML 与打印视图两处都验），远程 / `data:` / 缺失 / 超限的图片原样保留；Word 导出仍以「[图片]」占位（`docx.rs` 写明的取舍） | `tests/export_flow.rs`（7 项，含本项，反向验证过） |
| 图谱导出 · PNG / SVG | SVG：自包含、转义、无图例不画面板、空图合法；PNG：图例面板几何抽成纯函数并有单测，**失败（无 Canvas / 生成图片抛错）会明确提示而不是静默**（反向验证过） | `src/lib/graphExport.test.ts`（9 项）+ `src/features/graph/GraphPage.test.tsx`（+1） |
| 排序与分页的确定性（用户清单里的「抖动治理」） | 30 篇同秒写入的笔记：列表与检索的**分页不重复不遗漏、顺序与一次性取回一致、同一页重复请求结果相同**，且顺序**显式等于** `(updated_at DESC, id DESC)` / `(得分 ASC, updated_at DESC, id DESC)`——去掉 `n.id` 次级键两条用例立刻失败（真守卫，不是自洽检查） | `tests/paging_flow.rs`（2 项，反向验证过） |
| AI 闸门 · 主机解析与「只连本机」 | 主机解析抽成**一份共用实现**（闸门 / 白黑名单 / 来源分级 / 本地模型地址共用，杜绝两处口径漂移）：小写 → 去 `www.`、端口、userinfo，IPv6 字面量去方括号；本地模型地址只放行 `localhost` / `127.0.0.1` / `[::1]`，**拒绝** `localhost.evil.com`、`evil.com#@localhost`、十进制 IP、`0.0.0.0` 等写法；IPv6 在「域名规则」与「主机解析」两侧口径一致 | `ai::host` 单测（2 项）+ `ai::policy` 单测（2 项，双向反向验证） |
| 联网检索 | 可配置检索源 + 分级 + 预算 + 注入丢弃 | `tests/web_search_flow.rs` |
| 待复核提醒 | `note_due_for_review` + 首页卡片 | `tests/review_flow.rs` + `ReviewDueCard.test.tsx` |

## E. 需真机人工项（⬜ 需要你在桌面应用里操作）

> 现有安装包是 V1.2.0 的，**落后于本版代码**；恢复打包后请按 F 节重新构建，再逐条勾选。

| # | 项目 | 为什么不能自动 | 结果 |
| --- | --- | --- | --- |
| 1 | 编辑器输入、`[[` 补全、原生下拉与日期控件（`valid_until`） | 依赖 Tauri WebView 与原生控件 | ⬜ |
| 2 | 导入对话框（Word / CSV / Markdown / 目录）与导出（md / html / docx / pdf 打印视图） | 依赖系统文件对话框与打印面板 | ⬜ |
| 3 | 备份创建 / 校验 / 恢复 | 依赖磁盘状态与重启流程 | ⬜ |
| 4 | 拖拽分栏、大纲三态（固定 / mini / 隐藏）的手感 | 需要人工体感 | ⬜ |
| 5 | 版本历史：命名、还原、对比 | 依赖桌面端渲染 | ⬜ |
| 6 | 网页剪藏（真实网络抓取公开网页） | 需要外网与真实页面 | ⬜ |
| 7 | 研究助手：本地调研全链路（计划 → 报告 → 写入 → 打开） | 需要桌面端 | ⬜ |
| 8 | 研究助手：联网检索（配置真实检索源 → 首次确认 → 外部来源进报告） | 需要真实检索源与网络 | ⬜ |
| 9 | 首页「待复核」卡片点击跳转与时效角标 | 需要桌面端 | ⬜ |
| 10 | 首次启动放行（未签名：右键打开 / 系统设置里允许） | 依赖 Gatekeeper / SmartScreen | ⬜ |

## F. 产物完整性（⬜ 待恢复打包后执行）

打包前的两项「不产生产物」的检查已经在 2026-10-11 做过，结果如下：

| 预检项 | 结果 | 证据 |
| --- | --- | --- |
| release 档能否编译（新增 ureq / dom_query 之后） | ✅ | `cd src-tauri && cargo build --release --offline` → `Finished release profile in 1m 04s` |
| release 二进制体积与架构 | ✅ | 9,792,448 bytes（9.79 MB），`Mach-O 64-bit executable arm64`（V1.2.0 为 7.79 MB，差值是剪藏 + 联网检索的依赖） |
| 版本号四处一致（当前都是 1.2.0） | ✅ | 发布工作流的 preflight 现在会校验 `package.json` / `Cargo.toml` / `Cargo.lock` / `tauri.conf.json` 四处 |
| Windows 安装脚本不会打出错版本号 | ✅ | `installer.nsi` 未传 `APP_VERSION` 时 `!error` 直接失败（原来默认 1.0.0） |
| Linux 打包依赖齐全 | ✅（静态核对） | 工作流与本机脚本都补了 `libfuse2`（AppImage 走 linuxdeploy 需要 FUSE 2，Ubuntu 22.04 默认没有） |
| 三平台工作流结构与产物 | ✅（静态核对） | preflight / macos / windows / linux / release 五个作业齐全；Windows 产出 `setup.exe` + 便携 `zip` + `SHA256SUMS-windows.txt`，Linux 产出 `deb/rpm/AppImage` + `SHA256SUMS-linux.txt` |

| 项目 | 命令 / 检查 |
| --- | --- |
| 版本号四处一致升到 `1.2.1` | `package.json` / `src-tauri/Cargo.toml` / `Cargo.lock` / `src-tauri/tauri.conf.json` |
| macOS `.dmg`（含 `.app` 与 Applications 链接、CFBundle 版本、架构、体积） | `pnpm tauri build` → `hdiutil attach` + `plutil` |
| Windows `.exe` + portable `.zip` | 推 `v1.2.1` 标签走 [release.yml](../.github/workflows/release.yml) |
| Linux `.deb` / `.rpm` / `.AppImage` | 同上（一次 `v*` 标签触发三平台产物 + Release） |
| 校验值 | `SHA256SUMS-macos` / `SHA256SUMS-windows` / `SHA256SUMS-linux` |
| 安装说明同步 | [WINDOWS_INSTALL_GUIDE.md](./WINDOWS_INSTALL_GUIDE.md) / [LINUX_INSTALL_GUIDE.md](./LINUX_INSTALL_GUIDE.md) 的版本与截图 |

## G. 已知限制（不改代码也成立）

1. **未签名 / 未公证**：首次启动需手动放行（你已确认「暂不需要代码签名，本地运行测试」）；
2. **研究助手默认不接模型**：默认口径下报告由模板生成（结论段是来源原句摘录），
   推测段明确标注「未接模型」；设置里开启「本地模型（Ollama）」后可分别用它做
   **决策打分**与**撰写报告**（两个独立开关，默认关闭、只允许连本机地址、
   连不上自动降级到规则档并在报告里写明实际 provider）；**Jev 本体仍未接入**
   （需要先定模型访问方式，倾向本地 Ollama HTTP；接口位置是 `ResearchService::decision_model()`）；
3. **联网检索不预置检索源**：需要你自己填一个返回 JSON 的检索地址（支持 SearxNG `format=json`）；
4. **冲突检测是规则档**：同一子问题内「支持 vs 反对/限制」表述才判冲突，语义级比对待模型接入；
5. **语义检索 / 向量索引未实现**：检索仍是 FTS5（中文双字方案），召回实测 100%（1 万篇基准口径内）；
6. **中文分词未引入第三方分词器**：1 万篇基准下双字方案已达 100% 召回，按既定结论暂不引入；

> 第 7 条（父文档晚于子文档出现时层级要等下次编辑）**已修复（2026-10-11）**：
> 新增派生列 `declared_parent`（迁移 010）保存 `parent:` 声明原文，
> 扫描末尾对「声明了父文档但还没解析成功」的子文档重试一次，
> 父文档在后一轮出现时层级**当轮**即生效（见 `tests/external_interference_flow.rs` 2 项，
> 反向验证过）。数据库 schema 随之升到 **10**。
