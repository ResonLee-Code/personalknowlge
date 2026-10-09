# Personal Knowledge OS

本地优先的个人知识管理桌面应用。笔记正文以 Markdown 文件保存在用户自己的目录中，
SQLite 负责元数据与全文检索，界面基于 Tauri 2 + React 19 + TypeScript。

**当前版本：V1.0.1**（2026-10-07）—— 在 V1.0.0 基础上补齐导入 / 导出 / 分栏布局并修复若干缺陷，
变更明细见 [docs/CHANGELOG.md](./docs/CHANGELOG.md)；下一阶段安排见 [docs/PLAN_V1.1.md](./docs/PLAN_V1.1.md)。

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
| 平台 | macOS（`.app` + `.dmg`）、Windows（NSIS 安装包 + 便携版，交叉编译未签名）、Linux（`deb` / `rpm` / `AppImage`，用 `scripts/linux/build-linux.sh` 或 Linux CI 构建） |
| 图谱 | `/graph` 知识图谱：节点 = 笔记、边 = 双向链接；分类着色、度数定大小、分类/标签/孤立节点过滤、1–2 跳聚焦、缩放/平移/拖拽固定、双击打开 |
| 外部编辑 | 文件监听（原生事件 + 轮询兜底）、自动采纳外部新增/修改、标记缺失文件、忽略自身写入回声 |
| 数据安全 | 原子写入、内容哈希冲突检测、回收站、ZIP 备份与校验、Markdown 导出、路径穿越防护 |
| 界面 | 三栏工作台、全局搜索面板（⌘/Ctrl + K）、快速切换（⌘/Ctrl + P）、编辑/分屏/预览 + 大纲、深浅主题、空状态、状态栏 |
| 编辑 | 双向链接 `[[标题]]`（`[[` 自动补全、预览 chip 跳转）+ 反向链接面板、未解析链接一键建笔记、图片粘贴落盘到 `attachments/`、四种格式导出 |
| 视图模式 | 编辑 / 分屏 / 预览切换后立即记住；分屏比例可拖拽调整（0.3–0.7） |

## 技术栈

Tauri 2 · Rust · React 19 · TypeScript(strict) · Vite 8 · Tailwind CSS 4 · CodeMirror 6 ·
SQLite(bundled) + FTS5 · pulldown-cmark · notify · zip · serde · tracing

图谱视图为自研 Canvas 力导向布局（不引入可视化依赖）；AI 能力的框架规划见
[`docs/AI_ARCHITECTURE_PLAN.md`](./docs/AI_ARCHITECTURE_PLAN.md)（V1.1 不含模型调用）。

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

| 平台 | 状态 |
| --- | --- |
| macOS（V1.0.1） | ✅ 已构建：`src-tauri/target/release/bundle/{macos,dmg}/` |
| Windows（V1.0.0） | ⏸ 待确认后同步：`dist-windows/` 现有安装包与便携版仍为 2026-09-24 构建，不含 V1.0.1 改动 |

### Windows（x64）

已构建好的安装包位于 `dist-windows/`：

| 文件 | 说明 |
| --- | --- |
| `Personal Knowledge OS_1.0.0_x64-setup.exe` | NSIS 安装包（默认按当前用户安装，无需管理员权限） |
| `Personal Knowledge OS_1.0.0_x64-portable.zip` | 便携版：解压后直接运行，含 `WebView2Loader.dll` 与使用说明 |

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
- [功能规划（对标语雀/飞书 + 知识图谱）](./docs/FEATURE_PLANNING.md)
- [变更记录](./docs/CHANGELOG.md)
- [下一阶段计划（V1.1）](./docs/PLAN_V1.1.md)
