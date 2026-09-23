# Personal Knowledge OS V1.0

本地优先的个人知识管理桌面应用。笔记正文以 Markdown 文件保存在用户自己的目录中，
SQLite 负责元数据与全文检索，界面基于 Tauri 2 + React 19 + TypeScript。

本仓库是 [Personal-Knowledge-OS-Engineering-Design-V1.0.md](./Personal-Knowledge-OS-Engineering-Design-V1.0.md)
工程设计的实现：第一阶段只做好**记录、分类、搜索**，同时保证数据自主、可备份、可扩展。

## 已实现能力

| 领域 | 能力 |
| --- | --- |
| 工作区 | 创建、打开（含就地初始化已有目录）、切换、关闭、最近列表、统计 |
| 笔记 | 新建、编辑、保存、自动保存、删除到回收站、恢复、永久删除、重命名（含文件名）、移动分类、复制、收藏 |
| 分类 | 分类树、子分类、重命名（同步目录与文件路径）、删除策略（Inbox / 父分类 / 取消分类） |
| 标签 | 创建、重命名、删除、笔记多标签绑定，大小写不敏感唯一 |
| 搜索 | 标题 + 正文 + 标签全文检索、中文双字短语、英文前缀、相关度与时间排序、分类/标签/收藏/时间过滤、结果摘要与高亮、索引状态与手动重建 |
| 外部编辑 | 文件监听（原生事件 + 轮询兜底）、自动采纳外部新增/修改、标记缺失文件、忽略自身写入回声 |
| 数据安全 | 原子写入、内容哈希冲突检测、回收站、ZIP 备份与校验、Markdown 导出、路径穿越防护 |
| 界面 | 三栏工作台、全局搜索面板（⌘/Ctrl + K）、编辑/分屏/预览、深浅主题、空状态、状态栏 |

## 技术栈

Tauri 2 · Rust · React 19 · TypeScript(strict) · Vite 8 · Tailwind CSS 4 · CodeMirror 6 ·
SQLite(bundled) + FTS5 · pulldown-cmark · notify · zip · serde · tracing

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
│   ├── logs/app.log         # 运行日志
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
- [开发路线](./docs/ROADMAP.md)
