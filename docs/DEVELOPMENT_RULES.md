# 开发规范

对应设计文档第 6 / 22 / 23 章。所有改动都应遵守本文件。

## 1. 分层约束（强制）

1. UI 不直接访问数据库、不直接写 Markdown 文件；只能通过 `src/services/*` 调用命令。
2. 仓储（`repositories/`）之外不允许出现 SQL 字符串；仓储方法接收 `&Connection`。
3. 应用层负责编排与事务；**禁止在 `Db::with_conn` 回调内再次调用 `with_conn`**（会死锁）。
4. 命令层（`commands/`）只做参数转发与 `spawn_blocking`，不写业务逻辑。
5. 领域模型不依赖 Tauri / React / SQLite。
6. 新增文件系统操作必须经过 `WorkspacePaths::resolve_relative` 做路径安全检查。

## 2. 代码规范

- TypeScript：`strict`、`noUnusedLocals`、`noUncheckedIndexedAccess`；禁止 `any`（必要时用 `unknown` + 收窄）。
- Rust：`cargo fmt` 风格；错误使用 `Result<T, AppError>`；不使用 `unwrap()`/`expect()` 处理可预期失败
  （`expect` 仅用于不可能失败的初始化并写明原因）。
- 命名：Rust `snake_case`，TypeScript `camelCase`，React 组件 `PascalCase`。
- 注释：解释「为什么」，不要复述代码；对外行为变化必须同步更新 `docs/`。
- 提交：单一主题、可编译、说明测试结果（`feat:`、`fix:`、`test:`、`docs:`）。

## 3. 测试要求

| 层 | 位置 | 要求 |
| --- | --- | --- |
| Rust 单元 | `src/**/mod.rs::tests` | 文本处理、路径安全、frontmatter、ID、时间、ULID、事件过滤 |
| Rust 集成 | `src-tauri/tests/workspace_flow.rs` | 工作区、笔记、分类、标签、搜索、外部编辑、备份恢复 |
| 前端单元 | `src/**/*.test.ts(x)` | 工具函数、样式分支、组件渲染、内存后端命令 |
| 前端 E2E | `e2e/*.spec.ts` | 创建工作区、新建笔记、自动保存、搜索、设置、主题（Playwright） |

新增功能必须补充对应测试；修复缺陷必须新增可复现该缺陷的测试。

## 4. 数据安全底线

- 保存失败不得覆盖原文件（原子写入 + 临时文件）；
- 删除先进回收站；
- 不静默覆盖用户内容（哈希冲突检测）；
- 备份必须可独立校验（`manifest.json` + SHA-256）；
- 日志不记录笔记正文、不记录敏感信息。

## 5. 运行环境说明（重要）

本仓库的 `.cargo/config.toml` 把 crates.io 指向可达镜像：

```toml
[source.crates-io]
replace-with = "ustc"

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
```

原因：当前网络环境中 `index.crates.io` 的 TLS 证书链不可用（证书过期 / 主机名不匹配），
`cargo build` 无法拉取依赖。若你的环境可以正常访问 crates.io，删除 `.cargo/` 目录即可。

其他环境要点：

- Rust 依赖已锁定在 `src-tauri/Cargo.lock`，可用 `cargo build --offline` 离线构建；
- 受限环境（沙箱、部分网络盘）无法接收 FSEvents 等原生事件时，
  文件监听会自动降级为轮询（`PollWatcher`，3 秒），并在状态栏与 toast 中提示；
- 前端依赖使用 pnpm；`pnpm-workspace.yaml` 中声明了允许执行构建脚本的包（esbuild）。
- `.npmrc` 关闭了 pnpm 的「运行脚本前校验依赖」行为（`verify-deps-before-run=false`），
  否则离线环境下 `pnpm build` / `pnpm tauri dev` 会因为一次不必要的重装而失败。

## 6. AI 协作规范（设计文档 23）

AI 编程工具必须：

1. 先阅读 `docs/ARCHITECTURE.md`、`docs/DATABASE_SCHEMA.md`、`docs/DEVELOPMENT_RULES.md`；
2. 先输出实施计划与预计修改文件；
3. 只修改必要文件，不擅自改变架构；
4. 增加必要测试，运行 `cargo test` 与 `pnpm test` / `pnpm build`；
5. 输出变更摘要与未解决问题。

禁止：绕过仓储直接写 SQL、为通过编译删除测试、未经确认引入新框架、
修改数据库结构却不提供迁移、自动引入云服务或远程模型。

### 前端动作必须有诚实的反馈（2026-10-11 血泪教训）

- store 的写操作把错误收敛到 `error` 状态（不抛出）时，**必须返回 `boolean`**；
- 调用处禁止写 `void action(...).then(() => toast.success(...))`——失败时它照样报成功；
  统一用 `reportAction(ok, success, storeError, fallbackError)`（`src/lib/actionFeedback.ts`）；
- 批量操作要**如实计数**（`已恢复 1/2 篇，1 篇失败：原因`），不得把部分失败报成全部成功；
- 确实要「静默降级」的次要数据（例如首页统计、审计列表）必须在注释里写明理由。

### 表单控件必须有程序化标签（2026-10-11）

- **placeholder 不是标签**：对屏幕阅读器与语音控制都不可见；
- 每个 `Input` / `select` / `textarea` 必须有：`id` + `<Label htmlFor>`、`aria-label`，
  或被 `<label>` / `<Label>` 元素包裹；
- 自定义的 `Row label="…"` 只是视觉排版（渲染成 `<p>`），**不构成标签**——
  用它的地方要额外给控件加 `aria-label`；
- 守卫：`src/lib/a11yLabels.test.ts` 会扫描全部 TSX，缺标签即失败。

### 异步请求：最后发起的才算数（2026-10-11）

- 任何「按参数拉数据再写 store」的异步动作（详情、列表、检索、统计）都要用
  `createRequestGuard()`（`src/lib/latestRequest.ts`）判断结果是否过期；
- 过期就丢弃结果，**不要**写 store；`loading` 只由最新一次请求收起；
- 否则快速切换/连续输入时会出现「选中的是 B，内容是 A」这类错位——
  单测里很容易复现（用一个可控的 Promise 让先发的请求后 resolve）。

### 预览端（mock）必须与后端**同规则**（2026-10-11）

- 校验、错误码、默认值、写入纪律都要对齐：预览端放宽校验会让人误以为「功能就是这样」；
- 已知的**故意**差异只有两类：① 需要真实文件系统/网络的能力（导入导出、剪藏真实抓取、
  附件读写、本地模型调用）在预览端明确报「浏览器预览模式不支持…」；
  ② 研究会话的本地检索用词命中近似，而后端走 FTS5（已在注释里写明）；
- 每次给后端加校验时，顺手看一眼 mock 对应分支（`command_contract` 守卫只保证命令存在，
  不保证行为一致）。
- **归一化规则**（标签去空/折叠空白/大小写不敏感、分类路径、标题规范化）必须在两端一致，
  否则预览里会出现桌面端不会有的重复数据；
- **返回形状**也要一致（例如 `note_set_tags` 返回 `NoteDetail`，`note_set_tag_names` 返回 `Tag[]`）；
- 不要在预览端**硬编码**版本号、schema 版本、常量表——从 `package.json` 或共享常量读取，
  避免「关于」页这类界面在预览里显示错误信息。

### 防抖保存：切换 / 离开前必须 flush（2026-10-11）

- 凡是有防抖窗口的写操作（目前是编辑器自动保存），在**切换目标**或**卸载**之前必须显式
  `flush()` 立即落盘，否则窗口内的改动会随组件状态被覆盖而静默丢失；
- 编辑器通过 `registerEditorFlush()` 把「立即落盘」注册给 store，`openDetail` 在切换笔记前调用；
- 例外：用户**主动**要求丢弃本地内容的路径（冲突对话框的「重新加载磁盘内容」）
  传 `{ discardLocalEdits: true }` 显式跳过——不要把本地内容写回去。

关于 AI 依赖的边界（2026-10-11 按已确认口径更新）：允许本地形态的模型依赖
（Ollama HTTP，默认**关闭**、只允许本机地址、失败自动降级）；**不允许**在未获确认时
引入任何云端模型或第三方检索服务（检索源地址由用户在设置里自行配置，应用不预置）。

## 7. 提交前检查清单

```bash
cd src-tauri && cargo fmt --check && cargo test --offline
cd .. && pnpm lint && pnpm test && pnpm build
pnpm build && pnpm test:e2e      # 端到端 UI 验证，截图输出到 docs/screenshots
```

> `cargo test` 里包含一条**命令契约守卫**（`src-tauri/tests/command_contract.rs`）：
> 它把「前端 `services/*.ts` 调用的命令名」「`lib.rs` 里注册的命令」「mock 的 `case` 分支」
> 三份清单对齐。前端用字符串调命令，编译器管不到、E2E 又只跑 mock——
> 少了这条守卫，命令名写错就会变成**只有桌面端才暴露的静默失败**。
>
> 它还会校验**参数名**：前端 `call('cmd', { … })` 的顶层键必须能在命令签名里找到
> （`state` 除外，camelCase/snake_case 都认）。改参数名时若只改一边，测试会直接拦下来。

## 8. 文档同步规则

> 文档与代码必须同步演进，避免「实现已变、文档还停在旧描述」。

每次提交按下表更新文档：

| 变更类型 | 必须更新 |
| --- | --- |
| 新增能力 / 行为调整 | `docs/CHANGELOG.md`（用户可见的变化）、受影响的设计文档 |
| 新增或修改命令、DTO、错误码 | `docs/API_CONTRACT.md` |
| 界面布局、交互、快捷键变化 | `docs/UI_SPECIFICATION.md`（必要时重跑 E2E 更新 `docs/screenshots/`） |
| 模块划分、数据流、与设计文档的差异 | `docs/ARCHITECTURE.md` |
| 新增 AI 能力 / 闸门 / 工具 / 研究对象 | `docs/AI_ARCHITECTURE_PLAN.md`（接口与边界）+ `docs/ARCHITECTURE.md` 的 AI 层 |
| 表结构或迁移 | `docs/DATABASE_SCHEMA.md` + 新增迁移文件 |
| 阶段完成 / 排期调整 | `docs/ROADMAP.md`、`docs/PLAN_V1.1.md` |

版本号必须一致的地方：

- **可机检的四处**：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、
  `src-tauri/tauri.conf.json`（发布工作流的 preflight 会拦截不一致的标签）；
- 另外 `docs/CHANGELOG.md` 的版本小节也要同步，否则用户看到的「当前构建」会对不上。

### 文档链接守卫

`docs/**.md` 与 `README.md` 里的相对链接由 `src/lib/docsLinks.test.ts` 检查
（随 `pnpm test` 一起跑）：目标文件不在就失败，并一次列出全部问题。
只查**被 Git 跟踪**的 Markdown——`docs/yuque-manuals/books/` 下的语雀手册正文是第三方抓取内容
（默认不入库），其中的伪链接不属于本项目要维护的引用。
写文档时把「示例链接」用行内代码包起来（`` `[文字](路径.md)` ``），否则会被当成真实引用。

## 9. 跨平台产物更新规则

- 修改代码后**不要自动**重新生成另一平台的安装包；
- 更新 Windows/macOS 安装包前先确认目标版本号，再执行打包脚本；
- 打包后必须记录：产物路径、SHA-256、构建时间与验收结果，并同步到
  `docs/CHANGELOG.md` 的「产物」小节；
- 未在真机验证的产物必须显式标注「未在真机验证」，不得表述为已验证。
- 当前（2026-10-07 确认）以**本地运行与自测**为主：不做代码签名与公证、不引入自动更新；
  Windows 安装包在收到明确指令前不重新生成。
