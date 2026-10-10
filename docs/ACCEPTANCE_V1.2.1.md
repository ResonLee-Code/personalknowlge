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
| Rust 全量测试 | ✅ 29 个测试目标全绿（库单测 109 项 + 1 项 ignore 的基准 + 集成测试） | `cd src-tauri && cargo test --offline` |
| 前端类型检查 | ✅ 无错误 | `npx tsc --noEmit` |
| 前端 ESLint | ✅ 无告警 | `npx eslint .` |
| 前端单元 / 组件测试 | ✅ 41 个文件、217 用例全通过 | `CI=true pnpm test` |
| 前端生产构建 | ✅ 构建成功 | `CI=true pnpm build` |
| 端到端主流程 | ✅ 1 条 34 步流程通过（浏览器预览后端） | `CI=true pnpm test:e2e`（截图见 `docs/screenshots/`）；重负载下时序敏感，配置了 1 次重试与 10s 断言超时（原因写在 `playwright.config.ts` 注释里） |

关键断言（对应 V1.2.1 新增能力的「可测证据」）：

| 能力 | 断言 | 测试文件 |
| --- | --- | --- |
| 扫描 / 监听并发安全 | 扫描与监听同刻采纳同一批 400 篇、两线程同扫、边扫边建 120 篇：均无 UNIQUE 冲突 | `tests/concurrency_scan_flow.rs` |
| 图谱大库 | 3 万篇全量查询 < 1.5s 红线；节点总数不再被截断；抽样保留 3708 条边 | `tests/graph_bench.rs`（断言）+ `tests/graph_flow.rs` |
| 网页剪藏 | 抓取 → 转 Markdown → 写 `source_type: web` 与来源地址 | `tests/clip_flow.rs` |
| 版本历史命名版本 | 保存 / 命名 / 改名 / 清除与还原 | `tests/history_flow.rs` |
| 单篇复制到分类 | 副本带「副本」后缀、原作保留、分类正确 | `tests/batch_flow.rs` |
| AI 闸门 | 默认口径下 `web_fetch` 指向不可达地址只返回 `AI_DISABLED` / `NETWORK_DISABLED`（**未触网**）；放行后真实抓取并记 `outbound=true` | `tests/ai_policy_flow.rs` |
| 研究助手闭环 | 六段报告 + 「资料截至」+ 单一来源不进结论段 + 全程 `outbound=false`；写入默认 Inbox / 同标题不覆盖 / 追加留可回滚快照 / 放弃零写入 | `tests/research_flow.rs`（11 项） |
| 联网检索 | 仅本地口径下检索源**一个请求都不发**；放行后分级与预算生效、疑似注入页丢弃；未配置检索源不外发 | `tests/web_search_flow.rs`（3 项） |
| 待复核提醒 | 0 / 14 / 365 天三档窗口、过期天数、无法解析的时效字段跳过、回收站不提醒 | `tests/review_flow.rs`（2 项） |
| 决策层打分 | 来源可信度 ∈ (0,1]、交叉验证结论置信度高于单一来源；注入桩 `DecisionModel` 可改变打分（A/B 通路） | `tests/research_flow.rs` |
| 本地模型接入 | 开启后可信度由模型分决定且报告写明 provider；连不上自动降级且流程不中断；开关关闭时零请求；远程地址被拒绝 | `tests/local_model_flow.rs`（3 项） |

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

## C. 升级与迁移（✅ 已自动验证）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 旧库（schema 5，无 rowid 映射表）打开后自动迁移并重建索引 | ✅ 已索引 == 总数 | `cargo test --offline --test upgrade_flow` |
| 子文档层级（迁移 007）在前向迁移后可用 | ✅ `parent_id` 列 + 索引存在，`parent:` 可写 id 或标题 | `tests/parent_flow.rs` |
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
2. **研究助手不接模型**：报告由模板实现生成（结论段是来源原句摘录），推测段明确标注「未接模型」；
   Jev 本体未接入（需要先定模型访问方式，倾向本地 Ollama HTTP）；
3. **联网检索不预置检索源**：需要你自己填一个返回 JSON 的检索地址（支持 SearxNG `format=json`）；
4. **冲突检测是规则档**：同一子问题内「支持 vs 反对/限制」表述才判冲突，语义级比对待模型接入；
5. **语义检索 / 向量索引未实现**：检索仍是 FTS5（中文双字方案），召回实测 100%（1 万篇基准口径内）；
6. **中文分词未引入第三方分词器**：1 万篇基准下双字方案已达 100% 召回，按既定结论暂不引入。
