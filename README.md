# Personal Knowledge OS

本地优先的个人知识管理桌面应用。笔记正文以 Markdown 文件保存在用户自己的目录中，
SQLite 负责元数据与全文检索，界面基于 Tauri 2 + React 19 + TypeScript。

**当前版本：V1.2.1**（2026-10-11 升版，`package.json` / `src-tauri/Cargo.toml` /
`Cargo.lock` / `src-tauri/tauri.conf.json` 四处一致）。三平台**安装包**仍停在 V1.2.0：
按约定等单独指令再重新打包（先做本平台编译与真机验收）。
变更明细见 [docs/CHANGELOG.md](./docs/CHANGELOG.md)；阶段计划见
[docs/PLAN_V1.2.1.md](./docs/PLAN_V1.2.1.md)；验收证据见
[docs/ACCEPTANCE_V1.2.1.md](./docs/ACCEPTANCE_V1.2.1.md)。

本仓库是 [Personal-Knowledge-OS-Engineering-Design-V1.0.md](./Personal-Knowledge-OS-Engineering-Design-V1.0.md)
工程设计的实现：第一阶段只做好**记录、分类、搜索**，同时保证数据自主、可备份、可扩展。

## 已实现能力

| 领域 | 能力 |
| --- | --- |
| 工作区 | 创建、打开（含就地初始化已有目录）、切换、关闭、最近列表、统计 |
| 笔记 | 新建（可选本地模板）、编辑、保存、自动保存、删除到回收站、恢复、永久删除、重命名（含文件名）、移动分类、复制、收藏 |
| 导入 | 外部 Markdown / 文本文件与目录导入（保留 frontmatter 的 id/标题/标签/时间，复制语义，源文件不变）；支持「保留目录结构」，并内置 **Obsidian 库 / Notion 导出** 识别与转换（嵌入改写、哈希后缀剥离、附件一并导入） |
| 附件 | 粘贴/拖入的图片落到 `attachments/YYYY-MM/`；`/attachments` 页面按月份清点、显示引用笔记并清理 |
| 导出 | 单篇笔记导出为 Markdown / HTML / Word(.docx)，PDF 通过打印视图另存；整库导出 Markdown 与 ZIP 备份 |
| 布局 | 侧边栏 / 笔记列表 / 编辑与预览三处分栏可拖拽（双击复位、方向键微调），宽度随工作区记忆 |
| 分类 | 分类树、子分类、重命名（同步目录与文件路径）、删除策略（Inbox / 父分类 / 取消分类） |
| 标签 | 创建、重命名、删除、笔记多标签绑定，大小写不敏感唯一 |
| 搜索 | 标题 + 正文 + 标签全文检索、中文双字短语、英文前缀、相关度与时间排序、分类/标签/收藏/时间过滤、结果摘要与高亮、索引状态与手动重建 |
| 平台 | macOS（`.app` + `.dmg`）、Windows（NSIS 安装包 + 便携版）、Linux（`deb` / `rpm` / `AppImage`）；推送 `v*` 标签由 [release.yml](./.github/workflows/release.yml) 一次产出三平台工件并创建 Release（流程见 [docs/RELEASE_PROCESS.md](./docs/RELEASE_PROCESS.md)） |
| 图谱 | `/graph` 知识图谱：节点 = 笔记、边 = 双向链接；分类着色、度数定大小、分类/标签/孤立节点过滤、1–2 跳聚焦、缩放/平移/拖拽固定、双击打开 |
| 外部编辑 | 文件监听（原生事件 + 轮询兜底）、自动采纳外部新增/修改、标记缺失文件、忽略自身写入回声 |
| 数据安全 | 原子写入、内容哈希冲突检测、回收站、ZIP 备份与校验、Markdown 导出、路径穿越防护 |
| 界面 | 三栏工作台、全局搜索面板（⌘/Ctrl + K）、快速切换（⌘/Ctrl + P）、编辑/分屏/预览 + 大纲、深浅主题、空状态、状态栏 |
| 编辑 | 双向链接 `[[标题]]`（`[[` 自动补全、预览 chip 跳转）+ 反向链接面板、未解析链接一键建笔记、图片粘贴落盘到 `attachments/`、四种格式导出 |
| 视图模式 | 编辑 / 分屏 / 预览切换后立即记住；分屏比例可拖拽调整（0.3–0.7） |
| 文档层级 | 子文档（拖拽成为子文档 / 同层排序，`parent` / `order` 写进 frontmatter）、置顶、大纲预览（固定 / 贴边 / 隐藏三态） |
| 目录管理 | `/manage` 跨状态宽表（活跃 / 回收站 / 缺失）、搜索、「不在目录中」筛选、批量移动 / 打标签 / 删除 / 复制 |
| 版本历史 | 保存前自动留档（60 秒节流、每篇 50 份）、任意两版对比、命名版本（「保存为版本」）、还原正文 |
| 导入增强 | Word（`.docx` 标题 / 列表 / 实体）与 CSV（RFC 4180 → Markdown 表格）；Obsidian 库 / Notion 导出识别 |
| 统计与复核 | 首页统计明细（字数 / 近 7·30 天更新 / 置顶数 / 近 14 天写作活跃度）与 **待复核卡片**（`valid_until` 到期或 14 天内到期） |
| 网页剪藏 | 粘贴 URL → 抓取公开网页转 Markdown，写 `source_type: web` 与来源地址 |
| AI（第二代预留） | 设置页「AI」区块：总开关 / 联网档位（默认「仅本地」+ 首次确认）/ 抓取正文 / 域名黑白名单 / 研究预算 / 本地模型；四道闸门 + 行为审计（`.knowledge/logs/ai-audit.jsonl`，不含正文） |
| 研究助手 | `/research` 五步调研：立项 → 收集（本地 FTS5，可选联网检索）→ 验证（≥2 个独立来源才叫交叉验证）→ 汇总（固定六段报告 + 置信度）→ 写入提案（默认 Inbox、同标题不覆盖、写前 diff、可回滚） |
| 数据规模 | 3 万篇：图谱全量查询 811.7 ms；1 万篇：首次导入约 4.5 s、检索均值 18 ms、召回 100%（基准见验收清单） |

## 技术栈

Tauri 2 · Rust · React 19 · TypeScript(strict) · Vite 8 · Tailwind CSS 4 · CodeMirror 6 ·
SQLite(bundled) + FTS5 · pulldown-cmark · notify · zip · serde · tracing ·
ureq(rustls) + dom_query（网页剪藏与联网检索，复用 tauri-utils 的 html5ever）

图谱视图为自研 Canvas 力导向布局（不引入可视化依赖）。AI 能力的边界与接口见
[`docs/AI_ARCHITECTURE_PLAN.md`](./docs/AI_ARCHITECTURE_PLAN.md)：**默认不联网、不接模型**——
联网是独立闸门（默认「仅本地」+ 首次确认，检索源由你自己配置，应用不预置第三方服务）；
本地模型（Ollama HTTP）也要显式打开开关才会调用，失败自动降级到规则档 / 模板实现。

## 环境要求

- Node.js ≥ 20 与 pnpm 11
- Rust ≥ 1.77（stable）与 C 编译器（构建 bundled SQLite）
- macOS 或 Windows 的桌面环境

> 如果网络无法访问 `index.crates.io`，本项目的 `.cargo/config.toml` 已经把 crates.io
> 指向可达的镜像；详见 [docs/DEVELOPMENT_RULES.md](./docs/DEVELOPMENT_RULES.md)。

## 常用命令

```bash
pnpm install                 # 安装前端依赖
pnpm dev                     # 浏览器预览（内置内存后端，无需 Tauri）
pnpm tauri dev               # 启动桌面应用（开发模式）
pnpm build                   # 前端类型检查 + 生产构建
pnpm test                    # 前端单元测试（Vitest）
pnpm build && pnpm test:e2e  # 端到端 UI 验证（Playwright + 内置内存后端）
pnpm lint                    # ESLint

cd src-tauri
cargo test                   # Rust 单元测试 + 集成测试
cargo build                  # 构建桌面二进制
```

## 界面预览

桌面应用（Tauri 外壳，真实 SQLite 后端）：

![桌面应用](./docs/screenshots/00-desktop-app.png)

| 首页 | 编辑器（分屏） |
| --- | --- |
| ![首页](./docs/screenshots/02-dashboard.png) | ![编辑器](./docs/screenshots/03-editor.png) |

| 搜索结果与高亮 | 深色主题 |
| --- | --- |
| ![搜索](./docs/screenshots/05-search-page.png) | ![深色主题](./docs/screenshots/09-dark-theme.png) |

其余截图（欢迎页、全局搜索面板、分类管理、标签、设置、笔记列表）位于
[`docs/screenshots/`](./docs/screenshots)，由 `pnpm test:e2e` 自动生成。

## 下载与安装

| 平台 | 产物版本 | 状态 |
| --- | --- | --- |
| macOS（arm64） | V1.2.0 | ✅ 已构建：`src-tauri/target/release/bundle/{macos,dmg}/`（dmg sha256 见 CHANGELOG） |
| Windows（x64） | V1.2.0 | ✅ 已构建（交叉编译、未签名）：`dist-windows/`，**待真机验收** |
| Linux（x64） | — | ⏳ 未产出：Tauri 不支持从 macOS 交叉编译，需在 Linux 上构建（CI 已备好） |

> ⚠️ **安装包落后于代码**：V1.2.1 的内容（子文档层级、目录管理页、网页剪藏、
> AI 闸门与研究助手、待复核提醒等）**都还没进安装包**（包是 V1.2.0）。
> 打包改为**按指令触发**（2026-10-11 确认）：需要时单独下指令；执行时按
> [ACCEPTANCE_V1.2.1.md](./docs/ACCEPTANCE_V1.2.1.md) 的 F 节走即可。

### Windows（x64）

已构建好的安装包位于 `dist-windows/`：

| 文件 | 说明 |
| --- | --- |
| `Personal Knowledge OS_1.2.0_x64-setup.exe` | NSIS 安装包（默认按当前用户安装，无需管理员权限） |
| `Personal Knowledge OS_1.2.0_x64-portable.zip` | 便携版：解压后直接运行，含 `WebView2Loader.dll` 与使用说明 |

> `dist-windows/` 里同时保留了 V1.0.0 / V1.1.1 的历史产物，便于回退对比。

安装步骤、静默安装参数、快捷方式、卸载、常见问题与验证清单见
[docs/WINDOWS_INSTALL_GUIDE.md](./docs/WINDOWS_INSTALL_GUIDE.md)。

在 Windows 上重新打包：`.\scripts\windows\build-windows.ps1`；
在 macOS/Linux 上交叉打包：`ZIG=/path/to/zig scripts/cross/build-windows.sh`
再执行 `scripts/windows/build-installer.sh`（详见安装说明第七节）。

### macOS

```bash
pnpm install && pnpm build && pnpm tauri build
# 产物：src-tauri/target/release/bundle/{macos,dmg}/
```

## 目录结构

```text
.
├── Personal-Knowledge-OS-Engineering-Design-V1.0.md   # 工程设计基线
├── docs/                                              # 产品 / 架构 / 数据库 / API / UI / 规范 / 路线
├── src/                                               # 前端（app, components, features, stores, services, lib）
├── src-tauri/                                         # Rust 后端（commands, application, domain, infrastructure, repositories）
└── scripts/generate-icon.py                           # 图标生成（纯标准库 PNG 输出）
```

## 数据布局

工作区目录（可在「创建知识库」时选择）：

```text
MyKnowledge/
├── .knowledge/
│   ├── database.sqlite      # 元数据 + FTS5 索引 + 设置
│   ├── config.json          # 工作区配置（版本、路径、目录约定）
│   ├── backups/             # 备份包
│   ├── templates/           # 「另存为模板」的本地模板
│   ├── history/<笔记 id>/    # 版本历史快照（命名版本存 labels.json）
│   ├── research/            # 研究助手会话（可丢弃的中间产物）
│   ├── logs/app.log         # 运行日志
│   ├── logs/ai-audit.jsonl  # AI 行为审计（时间/能力/是否外发/结果，不含正文）
│   └── index/               # 预留索引目录
├── .trash/                  # 回收站（保留原相对路径）
├── 00_Inbox/ 01_技术/ … 99_Archive/
└── attachments/
```

删除文件只会移除磁盘文件与索引，数据库可以随时从 Markdown 重建。

## 文档

- [产品需求](./docs/PRODUCT_REQUIREMENTS.md)
- [系统架构](./docs/ARCHITECTURE.md)
- [数据库设计](./docs/DATABASE_SCHEMA.md)
- [API 契约](./docs/API_CONTRACT.md)
- [界面规范](./docs/UI_SPECIFICATION.md)
- [开发规范](./docs/DEVELOPMENT_RULES.md)
- [构建命令速查（只编译 / 出安装包 / 发版）](./docs/BUILD_COMMANDS.md)
- [开发路线](./docs/ROADMAP.md)
- [功能规划（对标语雀/飞书 + 知识图谱）](./docs/FEATURE_PLANNING.md)
- [语雀官方公开手册归档（对标查阅用，1222 篇）](./docs/yuque-manuals/README.md)
- [变更记录](./docs/CHANGELOG.md)
- [V1.2.1 验收清单（回归矩阵 / 性能实测 / 真机人工项）](./docs/ACCEPTANCE_V1.2.1.md)
- [AI 能力框架（闸门 / 工具层 / 研究助手规格）](./docs/AI_ARCHITECTURE_PLAN.md)
- [下一代技术储备与差距分析](./docs/NEXT_GEN_ANALYSIS.md)
- [下一阶段计划（V1.1，历史）](./docs/PLAN_V1.1.md)
- [下一阶段计划（V1.2.1：收口与稳定性）](./docs/PLAN_V1.2.1.md)
