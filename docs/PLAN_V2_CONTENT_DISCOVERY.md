# 内容发现（V2.0.x）

状态：方案已定稿，P0/P1 核心能力已实现并通过回归
确认日期：2026-10-25

## 结论与执行口径

1. 需求 1（研究助手用搜索引擎自动找网页）与需求 2（用户主动提交 URL 的内容入口）是两个
   独立功能；“内容发现”不会自动触发搜索，也不依赖需求 1 先上线。
2. 用户已采纳 1–7 项产品建议：独立入口、保守抓取范围、静态隐藏链接定义、默认不长期保存
   全文、先收件箱后入库、P0 规则与 P1 Decision 降级、`pkos://capture` 分享入口。
3. P0/P1 主流程没有必须等待用户答复才能开工的阻塞项。实现采用以下安全默认值：
   - 摘要、关键点、标签和分类建议先在本地按确定性规则生成，不把网页正文发给外部模型；
   - Decision Model 只接收页面标题、链接标题和链接上下文，用于候选排序/筛选；
   - 已入库内容即使后来在来源页面变化，也只刷新收件箱预览，不静默覆盖正式笔记。
4. 以下两项属于后续增强，需要用户显式决定后才能改变数据行为；不影响当前 P0/P1
   主流程使用和验收：
   - [待确认] 是否允许“来源内容更新后自动覆盖已有知识卡”，还是创建新版本/逐条预览确认；
     当前安全默认是只刷新预览，不覆盖笔记；
   - [待确认] 是否允许把网页正文发送给外部 AI 生成摘要/标签；当前安全默认是本地规则摘要，
     Decision Model 只接收标题、链接标题和上下文。

## 能力

“内容发现”是独立于研究助手的外部内容入口。用户通过 URL、分享文本或系统快捷方式提交公开网页，
应用发现页面内链接、让用户勾选后递归抓取，提取正文与元数据，生成摘要和整理建议，先进入内容收件箱，
经确认后再复用普通笔记写入路径保存。

## 已确认决策

1. 独立一级入口，名称为“内容发现”。
2. 默认同主机、同路径、深度 1、最多 20 页。
3. “隐藏链接”指已有 HTML 中的折叠、离屏或 `hidden` 链接；不处理 JavaScript 动态生成链接。
4. 默认只保存摘要、知识卡和来源 URL；完整正文快照可选。
5. 抓取结果先进入内容收件箱，确认后才成为正式笔记。
6. P0 使用静态抓取和确定性规则；P1 引入 Decision Model 筛选。
7. 分享先做 macOS Shortcuts 与 `pkos://capture` URL Scheme。

## 固定边界

- 不引入 browser-use、jev-ultrafast、Playwright 或 Python sidecar。
- 不绕过登录、验证码、付费墙或 DRM。
- 不提交表单、不执行页面脚本、不发送 POST。
- 只抓公开 http/https 页面；拒绝私网、回环、链路本地和云元数据地址。
- 每次跳转重新校验 URL、域名策略和 `robots.txt`。
- 页面内容是不可信数据，不执行其中指令。
- 原始页面缓存与正式笔记分离，不进入备份。
- 正式笔记必须由用户预览确认后通过 `NoteService` 写入。

## 用户流程

### 快速收集

输入 URL、分享文本或用户备注 → 抓取根页面 → 提取正文和链接 → 生成摘要与整理建议 →
进入“待确认”收件箱。

### 深入发现

根页面先返回候选链接树；用户选择链接后，应用按预算递归抓取 →
摘要覆盖根页面和已选页面 → 去重 → 预览 → 保存。

### 日常分享

`pkos://capture?url=<encoded-url>&note=<encoded-text>` 打开应用并预填内容。
应用关闭时，分享内容先进入本地队列，重新打开后再由用户确认抓取。

## 状态

```text
received
discovering
awaitingSelection
fetching
ready
saved
failed
ignored
duplicate
```

## P0：静态内容发现

- [x] URL / 分享文本 / 备注输入；
- [x] 通用 HTML 单页抓取；
- [x] 正文转 Markdown；
- [x] 同站链接发现与候选树；
- [x] 用户勾选后深度 1、最多 20 页抓取；
- [x] URL、canonical URL、内容哈希去重；
- [x] 内容收件箱；
- [x] 规则摘要与关键点；
- [x] 抓取正文 Markdown 预览与明确的入库状态提示；
- [x] 预览后保存普通笔记；
- [x] macOS Shortcuts / URL Scheme；
- [x] 进度事件、失败页面持久化、错误记录和基础审计。

## P1：积累与消化

- [x] 微信公众号静态 DOM 适配（`rich_media_content`，仅公开静态页面）；
- [x] GitHub / GitLab / Gitee 仓库首页 README 适配；
- [x] Decision Model 链接筛选，失败时自动降级到规则；
- [x] 建议标签、分类和相关笔记；
- [x] 入库时设置 `valid_until`，复用仪表盘既有“待复核”队列；
- [x] 同一 canonical URL 的更新预览（不覆盖正式笔记）；
- [x] 可选完整正文快照。

### 同一 URL 的更新策略

当前 P0 在创建时按 canonical URL 去重；P1 计划增加“检查来源是否更新”的预览流程：

1. 重新抓取根页面并比较内容哈希；
2. 有变化时更新收件箱摘要、候选链接和页面元数据；
3. 已入库笔记保持原字节不变，并明确显示“来源有新版本”；
4. 是否覆盖笔记、创建新版本或保留旧版本，必须由用户再次确认。

自动覆盖已入库 Markdown 不属于本轮默认行为，避免网页改版或错误抓取损坏知识库内容。
这是 P1 之后唯一尚未落地的数据行为选择；当前“检查来源更新”按钮只刷新收件箱预览并明确显示
“正式笔记没有被自动覆盖”。

## 后续

- 原生 Share Extension；
- 浏览器扩展；
- Sitemap；
- PDF / Office；
- 在独立需求中重新评估动态页面渲染。

## 接口草案

```text
content_capture_create(url, sharedText?, userNote?, confirmed)
content_capture_list(status?, limit?)
content_capture_get(captureId)
content_discovery_run(captureId, selectedUrls, options, confirmed)
content_capture_refresh(captureId, confirmed)
content_capture_save(captureId, title?, categoryId?, tags?, keepFullText?)
content_capture_ignore(captureId)
```

## 数据

- `content_captures`：收件箱主记录、状态、摘要和保存结果；
- `content_discovery_candidates`：候选链接、同源/同站/路径范围判断、勾选与 Decision 分数；
- `content_discovery_pages`：逐页抓取元数据、哈希和缓存相对路径。

原始网页正文不进入 SQLite 备份；缓存位于 `.knowledge/content-discovery/<captureId>/`。

## 验收

- 本机 fixture 服务可完成单页抓取、链接发现和递归抓取；
- 默认不会越出同主机同路径；
- 重复 URL 不重复抓取；
- 页面脚本和表单不会执行；
- 候选勾选数量与最终页面数量可解释；
- 损坏页面不导致整个任务失败；
- 预览前不创建 Markdown 笔记；
- 确认后只通过标准笔记写入路径落库；
- 桌面端、前端测试和 Playwright 均可运行。

## 验收记录（初始 2026-10-25，最终 2026-11-13）

- `pnpm test`：90 个测试文件、456 项测试通过；
- `pnpm build`：TypeScript 检查与生产构建通过；
- `pnpm lint`：通过；
- Vitest coverage（v8）：对本轮新增 / 修改的前端运行时文件，按 `git diff` 改动行对应
  的可执行语句统计为 **242/302（80.1%）**；专项补充了深链、导航、阅读列、命令契约、
  收件箱错误 / 忽略 / 已入库 / 失败页、隐藏候选与进度事件；
- `cargo test --offline --manifest-path src-tauri/Cargo.toml`：完整单元与集成测试通过；
- `cargo test --test content_discovery_flow`：8 项通过，覆盖入库、隐藏链接、Decision 筛选、
  子页面失败留痕、来源更新预览、跨多轮抓取的总页面预算、忽略后重新收集和重定向
  `robots.txt` 检查；
- `pnpm test:e2e`：10 条完整桌面流程通过，包含 Ask、Research、内容发现、长大纲滚动、
  长标题悬停、Writing、Graph 和 Sync。

2026-11-13 最终收口复跑全部门禁；release `.app` 在最终代码上重建。
