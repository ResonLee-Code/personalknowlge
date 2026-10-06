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
修改数据库结构却不提供迁移、自动引入云服务或 AI 依赖。

## 7. 提交前检查清单

```bash
cd src-tauri && cargo fmt --check && cargo test --offline
cd .. && pnpm lint && pnpm test && pnpm build
pnpm build && pnpm test:e2e      # 端到端 UI 验证，截图输出到 docs/screenshots
```

## 8. 文档同步规则

> 文档与代码必须同步演进，避免「实现已变、文档还停在旧描述」。

每次提交按下表更新文档：

| 变更类型 | 必须更新 |
| --- | --- |
| 新增能力 / 行为调整 | `docs/CHANGELOG.md`（用户可见的变化）、受影响的设计文档 |
| 新增或修改命令、DTO、错误码 | `docs/API_CONTRACT.md` |
| 界面布局、交互、快捷键变化 | `docs/UI_SPECIFICATION.md`（必要时重跑 E2E 更新 `docs/screenshots/`） |
| 模块划分、数据流、与设计文档的差异 | `docs/ARCHITECTURE.md` |
| 表结构或迁移 | `docs/DATABASE_SCHEMA.md` + 新增迁移文件 |
| 阶段完成 / 排期调整 | `docs/ROADMAP.md`、`docs/PLAN_V1.1.md` |

版本号在 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`
与 `docs/CHANGELOG.md` 四处必须一致。

## 9. 跨平台产物更新规则

- 修改代码后**不要自动**重新生成另一平台的安装包；
- 更新 Windows/macOS 安装包前先确认目标版本号，再执行打包脚本；
- 打包后必须记录：产物路径、SHA-256、构建时间与验收结果，并同步到
  `docs/CHANGELOG.md` 的「产物」小节；
- 未在真机验证的产物必须显式标注「未在真机验证」，不得表述为已验证。
- 当前（2026-10-07 确认）以**本地运行与自测**为主：不做代码签名与公证、不引入自动更新；
  Windows 安装包在收到明确指令前不重新生成。
