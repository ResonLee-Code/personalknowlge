# V1.2.1 验收清单（macOS 为主，Windows / Linux 对照）

> 生成时间：2026-10-11。**产物状态：按你的指令「打包暂停」，因此本文件不含安装包校验**；
> 已经完成的是「本机可执行」的自动化验收与性能实测（下表全部有命令与数字），
> 产物完整性列成待执行清单（F 节，等打包指令）；
> **真机项已跑第一轮**（E 节，2026-10-11：3 项通过 / 1 项部分通过 / 1 项不适用 / 5 项待办，
> 每项都有证据或未验原因）。
>
> 标注说明：**✅ 已自动验证**（本机可执行，含证据）；**⬜ 需打包后 / 需真机**（括号内为原因）。
>
> 版本号口径：**四处已升到 1.2.1**（2026-10-11；`package.json` / `src-tauri/Cargo.toml` /
> `Cargo.lock` / `tauri.conf.json`）。三平台**安装包**仍停在 V1.2.0，等单独指令再打包
> （2026-10-11 用户确认：打包不再作为常规任务项，先做本平台编译与真机验收）。

## A. 回归矩阵（✅ 已自动验证）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| Rust 全量测试 | ✅ 39 个测试目标全绿（库单测 116 项 + 1 项 ignore 的基准 + 集成测试） | `cd src-tauri && cargo test --offline` |
| 前端类型检查 | ✅ 无错误 | `npx tsc --noEmit` |
| 前端 ESLint | ✅ 无告警 | `npx eslint .` |
| 前端单元 / 组件测试 | ✅ 55 个文件、288 用例全通过 | `CI=true pnpm test` |
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
| 在线模型接入（OpenAI 兼容） | 授权后打分与报告由在线模型给出、报告写明实际 provider 且请求头带 Key；**未授权时零请求**且留一条 `outbound=false` 的拒绝审计；在线失败 → 降到本地模型 → 再降到模板，流程不中断；**换地址后授权作废并停止向旧地址外发**；直接改库塞裸 IP 也发不出去；Key 只写在应用数据目录、不进工作区 / 不进响应 / 不进审计，只回显 `remoteModelKeySet` | `tests/remote_model_flow.rs`（7 项）+ `ai::policy` / `ai::remote_model` 单测（4 项，反向验证过 3 处） |
| 文档与代码一致性 | README / ARCHITECTURE / 安装指南 / 发布流程的版本、能力表、命令与体积都按当前代码校正；文档内引用的相对路径全部存在（**可执行守卫**：跑 `pnpm test` 即检查，链接改坏立刻失败） | `src/lib/docsLinks.test.ts`（两类：相对链接目标 + 行内代码里的仓库内路径，均反向验证过） || 文档与代码一致性 | README / ARCHITECTURE / 安装指南 / 发布流程的版本、能力表、命令与体积都按当前代码校正；文档内引用的相对路径全部存在（**可执行守卫**：跑 `pnpm test` 即检查，链接改坏立刻失败） | `src/lib/docsLinks.test.ts`（两类：相对链接目标 + 行内代码里的仓库内路径，均反向验证过） |
| 版本身份与 schema 版本一致 | 四处版本号一致（漏改一处立刻失败；CHANGELOG 的版本小节刻意不在此列）；另外**文档里的「数据库 schema N」与代码 `SCHEMA_VERSION` 一致**（迁移升版时最容易漏改安装指南） | `src/lib/versionConsistency.test.ts`（2 项，均反向验证过） |
| 命令契约 | 前端 104 条命令与 `lib.rs` 注册表、mock 分支三方对齐；**参数名也对齐**（77 处带参调用 0 不一致）；守卫两次反向验证（插入未注册命令 / 改错参数名 → 立即失败） | `tests/command_contract.rs`（3 项） |
| IPC 契约 | ① 响应结构：界面读取的 **29 个 DTO**，TS 字段必须存在于 Rust 结构（238 字段 0 不一致）；② **事件名** 3/3 一致；③ **错误码** 22/22 集合相等。三条都反向验证过（故意制造漂移 → 立即失败） | `tests/dto_contract.rs`（13 项） |
| 可访问性（程序化标签） | 全部 `Input` / `select` / `textarea` 都有 `id` / `aria-label` / 被 `<label>` 包裹（本轮补齐 25 处）；无标题图标按钮与缺 `alt` 图片均为 0；守卫反向验证过 | `src/lib/a11yLabels.test.ts` |
| 快捷键 | 语义：无处理器的快捷键**不被吞**（⌘S/⌘E 由编辑器页面实现）；行为：⌘S 在自动保存延迟 3000ms 下仍能立即保存（E2E 断言窗口 1500ms）、⌘E 能把焦点送回编辑器；Delete 在输入框内不触发 | `src/hooks/useKeyboardShortcuts.test.ts`（4 项）+ E2E 第 5b 步；「关于」页列表与实现的双向对齐：`src/lib/shortcutsContract.test.ts`（2 项） |
| 文件监听降级 | 原生事件不可用时自动降级为轮询（3 秒）——用测试开关强制「探测通过但启动失败」，断言返回 `WatchMode::Polling` 而不是直接失败；线程创建失败也返回错误而非 panic | `src-tauri/src/infrastructure/watcher.rs` 单测（3 项） |
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
>
> 后续多轮改动（收藏派生列 / 回收站重建 / 编码解码 / 各类守卫）之后再复测一次（2026-10-11）：
> 写入 0.49 s、扫描采纳 **3.73 s**、FTS 重建 4.09 s、检索**平均 17.6 ms / p95 42.3 ms**、
> 各项召回 100%——**仍无退化**（回收站采纳只对 `.trash/` 做一次目录遍历，空回收站可忽略）。

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
| 设置项白名单 + 预览端排序对齐 | 主题 / 新建位置 / 编辑器模式只接受已知值（未知忽略并保留上一个有效值，`"SPLIT"` 这类大小写不同也算未知）；浏览器预览端的列表排序与桌面端**逐字对齐**（置顶最前、每种排序带 `id` 兜底、标题用码点序而非拼音序） | `workspace_flow.rs`（设置用例扩展）+ `mockBackend.test.ts`（+3，含「预览端 settings_update 只认已知字段 / 夹取数值 / 枚举白名单」，均反向验证过） || 设置项白名单 + 预览端排序对齐 | 主题 / 新建位置 / 编辑器模式只接受已知值（未知忽略并保留上一个有效值，`"SPLIT"` 这类大小写不同也算未知）；浏览器预览端的列表排序与桌面端**逐字对齐**（置顶最前、每种排序带 `id` 兜底、标题用码点序而非拼音序） | `workspace_flow.rs`（设置用例扩展）+ `mockBackend.test.ts`（+3，含「预览端 settings_update 只认已知字段 / 夹取数值 / 枚举白名单」，均反向验证过） |
| 预览端拖拽排序与后端一致 | `notes_reorder` 同层归一化（传入在前、其余按当前显示顺序追加、写 1..N）、不存在的 id 个别跳过 / **全部不存在报 `NOTE_NOT_FOUND`**、空数组报 `INVALID_INPUT` | `mockBackend.test.ts`（29 项，含本项，反向验证过） || 预览端拖拽排序与后端一致 | `notes_reorder` 同层归一化（传入在前、其余按当前显示顺序追加、写 1..N）、不存在的 id 个别跳过 / **全部不存在报 `NOTE_NOT_FOUND`**、空数组报 `INVALID_INPUT` | `mockBackend.test.ts`（29 项，含本项，反向验证过） |
| 预览端分类改名 / 移动 / 删除与后端一致 | 重命名：空名 / 斜杠 / 同级重名被拒 + 改目录名 + **子树笔记路径按前缀搬迁**；移动：防自指与成环、目标必须存在、重算序号与目录名前缀并搬迁路径；删除：三种策略归属与计数各不相同（`moveToParent` 归父级、其余归未分类） | `mockBackend.test.ts`（32 项，含 3 项本项，反向验证过） || 预览端分类改名 / 移动 / 删除与后端一致 | 重命名：空名 / 斜杠 / 同级重名被拒 + 改目录名 + **子树笔记路径按前缀搬迁**；移动：防自指与成环、目标必须存在、重算序号与目录名前缀并搬迁路径；删除：三种策略归属与计数各不相同（`moveToParent` 归父级、其余归未分类） | `mockBackend.test.ts`（32 项，含 3 项本项，反向验证过） |
| 预览端命令边界取值与后端一致 | `search_quick.limit` 夹取 1–30、`note_recent.limit` 夹取 1–100（`Number.isFinite` 判断，0 不被当成缺省）；空关键词回退成「按时间浏览」 | `mockBackend.test.ts`（34 项，含本项，反向验证过） || 预览端命令边界取值与后端一致 | `search_quick.limit` 夹取 1–30、`note_recent.limit` 夹取 1–100（`Number.isFinite` 判断，0 不被当成缺省）；空关键词回退成「按时间浏览」 | `mockBackend.test.ts`（34 项，含本项，反向验证过） |
| 预览端不伪造成功（备份） | 「立即备份」「校验备份」在预览模式返回 `BACKUP_FAILED`（请到桌面应用），**不再给出假路径 / 假体积 / 恒 true 的校验结果**；只读的列表与目录查询仍可用 | `mockBackend.test.ts`（35 项，含本项，反向验证过） |
| AI 闸门 · 主机解析与「只连本机」 | 主机解析抽成**一份共用实现**（闸门 / 白黑名单 / 来源分级 / 本地模型地址共用，杜绝两处口径漂移）：小写 → 去 `www.`、端口、userinfo，IPv6 字面量去方括号；本地模型地址只放行 `localhost` / `127.0.0.1` / `[::1]`，**拒绝** `localhost.evil.com`、`evil.com#@localhost`、十进制 IP、`0.0.0.0` 等写法；IPv6 在「域名规则」与「主机解析」两侧口径一致 | `ai::host` 单测（2 项）+ `ai::policy` 单测（2 项，双向反向验证） |
| 联网检索 | 可配置检索源 + 分级 + 预算 + 注入丢弃 | `tests/web_search_flow.rs` |
| 待复核提醒 | `note_due_for_review` + 首页卡片 | `tests/review_flow.rs` + `ReviewDueCard.test.tsx` |

## E. 真机验收（2026-10-11，macOS arm64 / 本机构建 1.2.1 —— 五轮汇总）

> 在**本机构建的 1.2.1**（`pnpm tauri build --bundles app`）上分五轮跑；前两轮用
> `/private/tmp/pkos-acceptance`，第三轮起换成干净的 `/private/tmp/pkos-accept3`（都是临时库，可随手删）。
> 方法：AppleScript（System Events）读 AX 树拿元素坐标 + CoreGraphics 投递真实鼠标事件，逐步截图存档。
>
> **结论：9 项完全通过、1 项不适用（第 10 条，本机构建没有 quarantine）。** 下表是**唯一结论**；
> 每一轮的经过、踩到的坑与当时的判断在表后按轮次记录。
> 已通过的条目都有可复核证据（文件字节数、服务端日志、包内容、界面截图），没验的一律写「未验」。

| # | 项目 | 结果 | 证据 / 未验原因 |
| --- | --- | --- | --- |
| 1 | 编辑器输入、`[[` 补全、原生下拉与日期控件（`valid_until`） | **✅** | ① 编辑器输入、`⌘E` 聚焦、`⌘S` 立即写盘全部生效（文件 mtime 与内容随之更新、预览同步、`已保存` 指示出现）；② **`[[` 补全**由用户实机确认：在调研报告正文输入 `[[` 弹出 4 条候选（调研报告 / 验收笔记A / 验收笔记B / 验收笔记C，每条带「双向链接」说明）；③ **原生日期控件**（「来源与时效」→ 有效期至）已点开：字段暴露为 `AXDateTimeArea`，点击后弹出系统日历浮层（Apr 2027 + 星期表头 + 上下月/年切换），日期按本地格式分段显示 |
| 2 | 导入对话框与导出（md / html / docx / pdf） | **✅** | 导出三格式实测（第三轮）：Markdown → 带 frontmatter 的 `.md`；HTML → 自包含单文件（内联样式、正文完整）；Word → 合法 OOXML（`[Content_Types].xml`/`word/document.xml`/`styles.xml`）；三者都走 macOS 原生保存面板。**PDF 打印视图**：先发现 ACL 缺陷（`open_path not allowed by ACL`）→ 修复 → **用户实机确认正常**。导入：把导出的 `.docx` 再导入 → 新建笔记且正文/标签正确抽取。**未单独试**：`.csv` 与「选择文件夹」目录导入（对话框已列出支持）、`.txt`/`.markdown` |
| 3 | 备份创建 / 校验 / 恢复 | **✅** | ① **创建**（第一轮）：`KnowledgeBackup_20261011-183921.zip`（294,805 B），`zipfile` 核对包内 12 个条目（数据库 / 配置 / 全部笔记 / 附件 / history / templates / manifest），**中文名带 UTF-8 标志位**（跨平台解压不乱码）；② **校验**（第六轮）：界面回显逐项结果——`✓ 清单文件存在：manifest.json 解析成功`、`✓ 文件完整性：已校验 20 个文件`；③ **恢复**（第六轮）：选一个空目录 → 得到**完整可用的知识库**（`.knowledge/{config.json,database.sqlite,history}` + `00_Inbox/` 5 篇笔记，库里 5 条记录）；④ **打开恢复出的库**：侧边栏「全部笔记 5」、列表正常（不出现「这里还没有笔记」） |
| 4 | 拖拽分栏、大纲三态的手感 | **✅** | 第三轮：固定 → 贴边 mini（悬停临时展开）→ 钉住 → 隐藏 → 再显示，五种状态走全；拖分割线把 `editorSplitRatio` 从 0.3 改到 **0.444 并落库**；**中途切「编辑 → 分屏」后再拖依旧线性跟手**（此前反馈的「切换一下才好用」本轮未复现）。主观手感仍建议你亲手拖两下 |
| 5 | 版本历史：命名、还原、对比 | **✅** | 第三轮：保存后自动生成 `history/<id>/<时间戳>.md`；对比面板显示 `+1 −0` 并高亮新增行；「保存为版本」写入 `labels.json`（`{"2026…":"v1 验收版本"}`）；「还原到选中版本」后正文确实回滚（新增行消失），toast 写明「只回滚正文」 |
| 6 | 网页剪藏（真实网络抓取公开网页） | **✅** | 真实抓取 `https://example.com` → 生成 `00_Inbox/20261011-183616-Example-Domain.md`：标题抽到 `Example Domain`、`source_type: web` + `source_ref: https://example.com/`、正文转 Markdown、带「来源 + 抓取时间」引用块 |
| 7 | 研究助手：本地调研全链路 | **✅** | 第三轮：AI 未开时被闸门挡住并给出准确提示；开启后 立项 → 收集 → 验证 → 汇总 → 生成写入提案 → **确认写入**，落库 `20261011-205419-调研报告：工作窃取调度策略.md`、会话状态 `written`；写前 diff 与「本地无材料 → 只给计划、不给结论」的警告都如实显示 |
| 8 | 研究助手：联网检索 | **✅** | 第三轮（用本机桩检索服务，`127.0.0.1:8899`，冒充 SearxNG）：配置检索源 → 档位「允许联网检索」→ 首次确认框 → 一次收集拿到 **4 条本地 + 2 条外部**来源（按 §9.3 分级），并**丢弃 1 个疑似注入页**；审计 `web_search outbound=true outcome=ok`、`research_collect … with_web`。中间一次失败是桩服务被回收（连接被拒），应用如实报错并降级——不是缺陷 |
| 9 | 首页「待复核」卡片点击跳转与时效角标 | **✅** | 手写一篇 `valid_until: 2026-09-01` 的笔记 → 首页出现「待复核 1 篇（其中已过期 1 篇）」与「已过期 40 天」角标 → 点标题跳转到该笔记，详情行显示 `⚠ 可能已过期`、`✓ 已索引`、来源类型与可靠性星级 |
| 10 | 首次启动放行 | **不适用（本地构建）** | 本机 `.app` **无 `com.apple.quarantine` 属性**、`Signature=adhoc` → 双击不会触发 Gatekeeper；该条只在「从网络下载的包」上有意义，等出包后再验 |

**额外证据（同样属于真机行为，第一轮）**：

| 项 | 证据 |
| --- | --- |
| 启动与版本 | 日志 `启动 Personal Knowledge OS 1.2.1` + `数据目录：…/PersonalKnowledgeOS`；`Info.plist` 的 `CFBundleShortVersionString = 1.2.1` |
| 老库自动迁移 | 打开一个 schema 较早的库时日志依次出现 `迁移 version=7/8/9/10`，随后 `已启用原生文件监听`（不重启、不丢数据） |
| 外部文件实时采纳 | 手写一个 `.md` 进 `00_Inbox/` → 界面计数立刻 `3 → 4`、正文字数 `203 → 241`，无需重启 |
| 保存链路 | `⌘S` 后文件 mtime 与内容同步更新，头部指示 `已保存` |

**未完成项与过程记录（按轮次）**：

> **第二轮（2026-10-11 晚）**：用户真机发现两个缺陷，其中
> ①「切换知识库后列表不更新 + 报『笔记不存在或已被删除』」已定位并修复
> （根因与修复见 CHANGELOG「切换知识库后列表不更新」），**真机复测通过**；
> ②「从目录管理进入笔记时编辑预览区频闪」：第二轮**未能复现**，第三轮按用户给的更精确路径
> （目录管理 → 进 A → 退回 → 进 B）**复现成功**，并抓到服务端日志：同一篇笔记被写入
> `title="" content_len=0` —— **频闪与「正文被写空」是同一个根因**（切笔记中间帧的错配落盘），
> 已按三条防线修复（见 CHANGELOG「切笔记时的『迟到的保存』会把正文写空」），真机复验帧稳定、
> 文件字节数不再变化。**遗留**：编辑器状态被整体清空的来源尚未定位（守卫已能挡住写入），
> 第四轮已追到底（见下）。

> 第二轮**未跑**的验收项（当时记录）：2（导入导出）、3 的「校验 / 恢复」两步、4、5、7、8。
> 原因是 AX 自动化在本轮后期退化到每个元素 30–90 秒，继续跑的成本不可控；
> 另外自动化在验收用的**临时库**里误覆盖过两篇笔记（已用「版本历史」快照还原）。
> 下一步建议：换新进程 + 干净临时库再跑 —— 第三轮就是这么做的。

> **第三轮（2026-10-11 深夜，按上面的建议执行）**：新建干净临时库 `/private/tmp/pkos-accept3`
> （3 篇种子笔记）+ 重启应用进程，补齐了下面几项（逐项都有落盘/审计证据）：
>
> | 项 | 结果 | 证据 |
> | --- | --- | --- |
> | 4 大纲三态 + 拖拽分栏 | **✅** | 固定 → 贴边 mini（悬停临时展开）→ 钉住 → 隐藏 → 再显示，五种状态都走了一遍；分割线拖拽把 `editorSplitRatio` 从 0.3 改到 0.444 并落库；**中途切「编辑 → 分屏」后再拖依旧线性跟手**（正是此前反馈的「切换一下才好用」那点，本轮未复现） |
> | 5 版本历史 | **✅** | 保存后自动生成 `history/<id>/<时间戳>.md`；对比面板显示 `+1 −0` 并高亮新增行；「保存为版本」写入 `labels.json`（`{"2026…":"v1 验收版本"}`）；「还原到选中版本」后正文确实回滚（新增行消失），toast 提示「只回滚正文」 |
> | 2 导出 | **✅（除 PDF）** | Markdown 导出 → `/tmp/pkos-export/验收笔记B.md`（带 frontmatter）；HTML 导出 → 自包含单文件（内联 `<style>`、正文完整）；Word 导出 → 合法 OOXML（`[Content_Types].xml` / `word/document.xml` / `styles.xml`）；三者都走 macOS 原生保存面板 |
> | 2 导入 | **✅（Word）** | 把导出的 `.docx` 再导入 → 新建 `20261011-204425-验收笔记B.md`，正文与标签被正确抽取（`.csv` / 目录导入未单独试，对话框已列出支持） |
> | 2 PDF 打印视图 | **✅（用户实机确认）** | 用户真机点击复认：弹出 `Command plugin:opener\|open_path not allowed by ACL` —— **确认是缺陷**（`opener:default` 不含 `open_path`，同一处也让「设置 → 工作区目录 → 打开」失效）。已显式放行 `opener:allow-open-path` + 非空 scope，并在 `pluginPermissions.test.ts` 加守卫（去掉权限即红）。修复后复测：「工作区目录 → 打开」成功唤起 Finder、无报错（同一命令），打印视图不再报 ACL 错误 |
> | 7 研究助手本地全链路 | **✅** | AI 未开时被闸门挡住并给出准确提示；开启后 立项 → 收集 → 验证 → 汇总 → 生成写入提案 → **确认写入**，落库 `20261011-205419-调研报告：工作窃取调度策略.md`，会话状态 `written`；写前 diff 与「本地无材料 → 只给计划、不给结论」的警告都如实显示 |
> | 8 联网检索 | **✅** | 闸门链：配置检索源 → 档位「允许联网检索」→ 首次弹「本次将联网检索」确认框。happy path 补测通过（本机桩检索服务，`127.0.0.1:8899`，冒充 SearxNG 形状）：一次收集拿到 **4 条本地 + 2 条外部**来源（`docs.example.com` / `blog.example.com`，均按 §9.3 分级），并**丢弃 1 个疑似注入页**（步骤说明原文：「丢弃疑似注入页面 1 个（网页内容是数据，不是指令）」）；审计 `web_search outbound=true outcome=ok`、`research_collect … with_web`。中间那次失败（`INVALID_INPUT`「Connection refused」）是本机桩服务被回收，应用如实报错并降级——不是缺陷 |
>
> 第三轮用到的辅助手段：本机桩检索服务（`127.0.0.1:8899`，冒充 SearxNG 形状，含一条注入页样本）、
> 直接改工作区设置库来配置检索源（设置页的 AI 区块滚动条不接受合成滚轮事件）。
> 验收库 `/private/tmp/pkos-accept3` 是临时目录，可随时删除。

> **第四轮（2026-10-11 深夜）**——把「编辑器状态被整体清空」这条线追完了：
> 根因是 `NoteService::detail()` 把**读文件失败静默降级成空正文**（`Err(_) => None`），
> 前端把空正文同步进编辑器，下一次保存就写回文件（真机上 5 篇笔记被写空的起点）。
> 已修：只有扫描器标记 `missing` 的笔记才按空正文返回，其它读取失败一律如实报错；
> 新增回归测试 1 项（反向验证过）。同轮用户实机确认两项：`[[` 补全（第 1 条）
> 与 PDF 打印视图（第 2 条，ACL 修复后正常）。

> **第五轮（2026-10-11 收尾）**：第 1 条的**原生日期控件**已点开验证（见上表第 1 行）。
> 此时 10 条里 8 条完全通过、1 条部分通过、1 条不适用。

> **第六轮（2026-10-11，用户指出「我也没看见『校验 / 恢复』」后补测）**：**用户是对的，我此前的判断错了**——
> 那两个按钮不是「在可视区下方」，而是**只在备份列表有条目时逐条渲染**（每张备份卡片下方一行）。
> 当时的验收库 `/private/tmp/pkos-accept3` 里一个备份都没有（唯一那个备份在第一轮的另一个临时库里），
> 界面显示的是「暂无备份文件。」，所以谁都看不到这两个按钮。现场补测：
> **立即备份 → 校验（逐项 ✓，见上表）→ 恢复到空目录 → 打开恢复出的库（5 篇笔记）**，四项全通；
> 这条从「部分通过」改为 **✅**。

**自动化本身的局限（下次做真机验收前先看）**：

1. 自动化在「点击」上花了大量时间——`System Events` 的 `entire contents` 对这个 WKWebView
   时灵时不灵，`click at` 还会命中隐藏元素（本轮误点开过「新建子文档」对话框），
   最后稳定的组合是「递归遍历 AX 树取坐标 + CoreGraphics 投递鼠标事件」；
2. 过程中的两个观察，下一轮要跟进：
   - **第 1 次点「立即备份」没有产出文件**，同时头部出现红色 `保存失败` 指示，**没有给出原因**；
     第 2 次点就成功了。这可能是「点击落在了卡片文字上」导致没触发，也可能是备份失败只被
     一个笼统的「保存失败」盖住——值得复现一次确认（属 P2）；
   - 系统中文输入法会拦截合成按键，做 UI 自动化时必须先切到 ABC 输入源，否则会把乱码写进笔记。

## F. 产物完整性（⬜ 待恢复打包后执行）

打包前的两项「不产生产物」的检查已经在 2026-10-11 做过，结果如下：

| 预检项 | 结果 | 证据 |
| --- | --- | --- |
| release 档能否编译（新增 ureq / dom_query 之后） | ✅ | `cd src-tauri && cargo build --release --offline` → `Finished release profile in 1m 04s` |
| release 二进制体积与架构 | ✅ | 9,842,176 bytes（9.84 MB），`Mach-O 64-bit executable arm64`（V1.2.0 为 7.79 MB，差值是剪藏 + 联网检索的依赖） |
| 版本号四处一致（已升到 1.2.1） | ✅ | 发布工作流的 preflight 现在会校验 `package.json` / `Cargo.toml` / `Cargo.lock` / `tauri.conf.json` 四处；本地守卫在 `pnpm test` 里也拦一道 |
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
   推测段明确标注「未接模型」；设置里有**两档可选模型**，都能分别用于
   **决策打分**与**撰写报告**（四个独立开关，默认全关）：
   ①「本地模型（Ollama）」只允许连本机地址；②「在线模型（OpenAI 兼容）」需要填地址 / 模型名 /
   API Key 并**首次授权**，换地址或模型名后授权作废。优先级是**在线（已授权）→ 本地 → 模板**，
   任何一档不可用都自动降级并在报告里写明实际 provider；
   **Jev 本体仍未接入**（需要先定模型访问方式；接口位置是 `ResearchService::decision_candidates()` /
   `composer_candidates()`）；
3. **联网检索不预置检索源**：需要你自己填一个返回 JSON 的检索地址（支持 SearxNG `format=json`）；
4. **冲突检测是规则档**：同一子问题内「支持 vs 反对/限制」表述才判冲突，语义级比对待模型接入；
5. **语义检索 / 向量索引未实现**：检索仍是 FTS5（中文双字方案），召回实测 100%（1 万篇基准口径内）；
6. **中文分词未引入第三方分词器**：1 万篇基准下双字方案已达 100% 召回，按既定结论暂不引入；

> 第 7 条（父文档晚于子文档出现时层级要等下次编辑）**已修复（2026-10-11）**：
> 新增派生列 `declared_parent`（迁移 010）保存 `parent:` 声明原文，
> 扫描末尾对「声明了父文档但还没解析成功」的子文档重试一次，
> 父文档在后一轮出现时层级**当轮**即生效（见 `tests/external_interference_flow.rs` 2 项，
> 反向验证过）。数据库 schema 随之升到 **10**。
