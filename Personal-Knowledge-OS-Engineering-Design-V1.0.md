# Personal Knowledge OS V1.0
## 软件工程设计说明书

> 版本：V1.0  
> 项目代号：Personal Knowledge OS  
> 产品类型：跨平台桌面个人知识管理软件  
> 目标平台：Windows / macOS  
> 开发模式：AI 编程工具辅助开发  
> 第一阶段：记录、分类、搜索  
> 架构策略：本地优先、模块化单体、商业化预留

> **文档状态**：本文档是 V1.0 的工程设计基线，正文（第 1–30 章）保持基线语义不变；
> 实现过程中的偏差、增补与验收状态统一记录在 **附录 C：实现状态与偏差记录**。
> 截至 2026-10-07，实现版本为 V1.0.1。
> 配套工程文档见 `docs/`：架构、数据库、API 契约、界面规范、开发规范、变更记录（CHANGELOG）、下一阶段计划（PLAN_V1.1）。
> 依据 1.1 节要求「若实现过程中需要偏离本文档，应先更新设计文档，再修改代码」，
> 任何新的偏差都必须先登记到附录 C，再进入编码。

---

# 1. 文档说明

## 1.1 文档目的

本文档是 Personal Knowledge OS V1.0 的统一工程设计基线，用于指导：

- 产品需求分析
- 系统架构设计
- 数据库设计
- UI/UX 设计
- 前后端开发
- AI 编程工具协作
- 测试与验收
- 后续版本演进

所有开发任务应优先参考本文档。若实现过程中需要偏离本文档，应先更新设计文档，再修改代码。

## 1.2 目标读者

- 项目负责人
- 产品设计人员
- 前端开发人员
- Rust/Tauri 开发人员
- AI 编程助手
- 测试人员
- 未来商业化产品团队

## 1.3 文档范围

本文档覆盖：

1. 产品定位
2. 用户场景
3. 功能需求
4. 非功能需求
5. 技术选型
6. 系统架构
7. 模块拆分
8. 数据库设计
9. Markdown 存储规范
10. API 契约
11. UI 页面与交互
12. 搜索设计
13. 文件监听
14. 数据安全
15. 测试方案
16. AI 辅助开发规范
17. 开发路线
18. 验收标准
19. 未来扩展规划

---

# 2. 产品概述

## 2.1 产品定义

Personal Knowledge OS 是一款面向个人长期知识积累的跨平台桌面知识管理软件。

产品帮助用户：

- 快速记录知识
- 按分类组织内容
- 使用标签补充语义
- 通过全文搜索找回知识
- 使用开放的 Markdown 文件保存个人知识资产

## 2.2 产品愿景

让个人知识能够：

- 随时记录
- 清晰组织
- 快速检索
- 长期保存
- 自主掌控
- 平滑扩展

## 2.3 产品原则

### 本地优先

用户数据默认保存在本地，不依赖云端服务。

### 用户拥有数据

知识正文采用 Markdown 文件保存，避免被专有数据库完全锁定。

### 先解决核心问题

第一版优先做好记录、分类、搜索，不提前加入 AI、协作和复杂同步。

### 简单而强大

界面保持简洁，但底层架构允许未来扩展。

### 可恢复

索引和缓存应当可以重建，数据备份和导出必须可靠。

---

# 3. 用户与使用场景

## 3.1 目标用户

V1.0 主要面向：

- 需要长期积累技术知识的个人
- 软件开发人员
- 产品经理
- 独立开发者
- 研究人员
- 需要管理工作资料的个人用户
- 使用 AI 工具进行学习和项目实践的用户

## 3.2 核心场景

### 场景 A：快速记录

用户看到一篇技术文章，希望快速记录重点。

操作：

1. 打开软件
2. 点击新建笔记
3. 输入标题和内容
4. 保存
5. 后续再整理分类

### 场景 B：整理知识

用户定期整理 Inbox 中的内容。

操作：

1. 打开 Inbox
2. 选择笔记
3. 指定分类
4. 添加标签
5. 移动到目标目录

### 场景 C：查找知识

用户需要回忆之前记录的技术方案。

操作：

1. 使用全局搜索
2. 输入关键词
3. 查看标题和摘要
4. 打开目标笔记

### 场景 D：外部编辑

用户使用其他编辑器修改 Markdown 文件。

系统应：

1. 监听文件变化
2. 重新解析文件
3. 更新元数据
4. 更新搜索索引

---

# 4. V1.0 产品范围

## 4.1 必须实现

- 工作区创建
- 工作区打开
- 工作区切换
- Markdown 笔记创建
- 笔记编辑
- 笔记保存
- 自动保存
- 笔记删除
- 笔记重命名
- 分类树
- 笔记移动
- 标签基础能力
- 标题搜索
- 正文全文搜索
- 搜索结果摘要
- 搜索结果高亮
- 索引更新
- 索引重建
- 最近编辑
- Inbox
- 基础备份
- Markdown 导出
- Windows 构建
- macOS 构建

## 4.2 V1.0 不实现

- AI 问答
- 向量数据库
- 语义搜索
- 云端同步
- 用户注册登录
- 多用户协作
- 团队权限
- 移动端
- 插件市场
- 复杂所见即所得编辑器
- 自动知识图谱
- 自动摘要
- 自动标签生成
- 在线分享
- 复杂订阅系统

## 4.3 预留但不实现

- 双向链接
- 反向链接
- 知识图谱
- AI 检索
- 云同步
- 插件系统
- 商业授权
- 团队空间
- 企业私有部署

---

# 5. 功能需求规格

## 5.1 工作区管理

### FR-W01 创建工作区

用户可以选择一个本地目录创建知识库。

输入：

- 工作区名称
- 根目录

输出：

- 工作区 ID
- 工作区路径
- 初始化状态

验收：

- 目录不存在时可以创建
- 目录存在时可以初始化
- 已初始化目录不可重复破坏性覆盖
- 初始化失败时显示明确错误

### FR-W02 打开工作区

用户可以打开已有知识库目录。

系统应：

- 检查目录结构
- 检查数据库版本
- 初始化缺失索引
- 扫描 Markdown 文件
- 显示工作区内容

### FR-W03 工作区切换

支持多个本地工作区。

### FR-W04 工作区关闭

关闭后释放：

- 数据库连接
- 文件监听器
- 索引任务
- 相关缓存

---

## 5.2 笔记管理

### FR-N01 创建笔记

支持：

- 标题
- 初始内容
- 分类
- 标签
- 自动生成 ID
- 自动生成 Markdown 文件

默认创建到 Inbox 或当前分类。

### FR-N02 编辑笔记

支持 Markdown 源码编辑。

### FR-N03 保存笔记

保存时必须：

1. 校验笔记 ID
2. 生成规范化 Markdown
3. 写入临时文件
4. 原子替换目标文件
5. 更新元数据
6. 更新搜索索引

### FR-N04 自动保存

建议默认自动保存延迟为 800ms～1500ms。

自动保存必须：

- 防抖
- 避免并发写入
- 显示保存状态
- 保存失败时提示用户

### FR-N05 删除笔记

默认移动到回收站目录，不立即永久删除。

### FR-N06 重命名笔记

重命名标题时可同步修改文件名，但必须避免路径冲突。

### FR-N07 移动笔记

支持在分类目录之间移动。

### FR-N08 最近编辑

按 updated_at 倒序显示。

---

## 5.3 分类管理

### FR-C01 创建分类

支持创建根分类和子分类。

### FR-C02 重命名分类

修改分类名称时同步更新目录和数据库引用。

### FR-C03 删除分类

删除分类前必须明确处理子分类和笔记：

- 移动笔记到 Inbox
- 移动笔记到父分类
- 取消分类
- 取消操作

### FR-C04 分类树

支持：

- 展开
- 折叠
- 排序
- 选中
- 拖拽移动，后续版本实现

---

## 5.4 标签管理

### FR-T01 添加标签

标签名称在同一工作区内唯一，不区分大小写。

### FR-T02 删除标签

删除标签不删除笔记正文。

### FR-T03 笔记绑定标签

一篇笔记可以拥有多个标签。

---

## 5.5 搜索

### FR-S01 全局搜索

支持快捷键：

- Windows：Ctrl + K
- macOS：Command + K

### FR-S02 标题搜索

优先返回标题匹配结果。

### FR-S03 正文搜索

搜索 Markdown 正文中的关键词。

### FR-S04 结果摘要

显示：

- 标题
- 分类
- 更新时间
- 匹配摘要
- 标签

### FR-S05 结果高亮

高亮匹配关键词。

### FR-S06 过滤

V1.0 支持：

- 分类过滤
- 标签过滤
- 时间过滤，若开发成本可控

### FR-S07 重建索引

用户可以手动触发全量索引重建。

---

## 5.6 备份与导出

### FR-B01 创建备份

备份内容：

- Markdown 文件
- 附件
- 数据库
- 配置文件

### FR-B02 导出 Markdown

支持导出完整知识库目录。

### FR-B03 恢复

V1.0 可先支持手动恢复目录，自动恢复作为增强功能。

---

# 6. 非功能需求

## 6.1 性能

目标数据规模：

- 10,000 篇笔记
- 单篇笔记 1MB 以内为常见场景
- 100,000 条标签和关联关系以内

性能目标：

- 应用启动后 3 秒内显示主界面，具体取决于设备
- 普通笔记打开响应目标小于 300ms
- 常规搜索响应目标小于 500ms
- 搜索结果首屏最多显示 50 条
- 索引更新不阻塞编辑器

## 6.2 可靠性

- 保存失败不得覆盖原文件
- 索引损坏可重建
- 数据库迁移可追踪
- 关键异常写入日志
- 删除操作可恢复
- 备份文件可独立验证

## 6.3 安全性

- 默认不上传用户内容
- 不收集不必要的个人数据
- 文件访问限制在用户授权工作区
- 避免路径穿越
- 对外部 Markdown 内容进行安全渲染
- 不执行笔记中的脚本

## 6.4 可维护性

- TypeScript 开启 strict
- Rust 使用明确错误类型
- 模块职责单一
- 业务逻辑有测试
- 数据库通过迁移管理
- 不允许 UI 直接访问数据库

## 6.5 可扩展性

未来可替换：

- 存储实现
- 搜索实现
- 同步实现
- AI 服务
- 授权服务
- 导入导出模块

---

# 7. 技术选型

## 7.1 技术栈总览

| 层级 | 选型 |
|---|---|
| 桌面框架 | Tauri 2 |
| 前端框架 | React |
| 语言 | TypeScript |
| 构建工具 | Vite |
| UI | shadcn/ui + Tailwind CSS |
| 编辑器 | CodeMirror 6 |
| 核心语言 | Rust |
| 数据库 | SQLite |
| 数据访问 | rusqlite，或经验证的 SQLx 方案 |
| 全文搜索 | SQLite FTS5 |
| Markdown 解析 | pulldown-cmark 或 comrak |
| Frontmatter | YAML 解析库 |
| 文件监听 | notify |
| 序列化 | serde |
| 错误处理 | thiserror + anyhow |
| 日志 | tracing |
| ID | UUID 或 ULID |
| 状态管理 | Zustand |
| 测试 | Rust tests、Vitest、Playwright |
| 版本控制 | Git |

## 7.2 技术选型原则

- 优先成熟开源方案
- 优先减少外部运行依赖
- 优先本地运行
- 优先可替换性
- 避免为未来需求提前引入复杂基础设施

## 7.3 不采用的方案

### 不采用微服务

原因：

- 单机桌面应用不需要
- 增加部署和维护成本
- 不利于个人开发

### 不采用独立搜索服务器

暂不采用 Elasticsearch、OpenSearch、Meilisearch 等。

原因：

- V1.0 数据规模有限
- 增加安装和运维复杂度
- SQLite FTS5 足以作为第一阶段验证方案

### 不采用云数据库

V1.0 不需要云端依赖。

---

# 8. 系统总体架构

## 8.1 架构风格

采用：

> 模块化单体 + 本地优先 + 分层架构

分层：

1. 表现层
2. 应用层
3. 领域层
4. 基础设施层

## 8.2 架构原则

- UI 不直接操作 SQLite
- UI 不直接写 Markdown 文件
- 领域模型不依赖 React
- 应用服务编排业务流程
- Repository 隔离存储实现
- 搜索索引可以重建
- 所有文件操作必须经过路径安全检查

## 8.3 逻辑架构

```text
React UI
  ├── Layout
  ├── Note Editor
  ├── Note List
  ├── Search UI
  └── Category UI
        │
        ▼
Tauri Commands
        │
        ▼
Application Services
  ├── WorkspaceService
  ├── NoteService
  ├── CategoryService
  ├── TagService
  ├── SearchService
  └── BackupService
        │
        ▼
Domain Models
  ├── Workspace
  ├── Note
  ├── Category
  ├── Tag
  └── SearchQuery
        │
        ▼
Infrastructure
  ├── MarkdownStorage
  ├── SQLiteRepository
  ├── FTS5Index
  ├── FileWatcher
  └── BackupProvider
```

---

# 9. 项目目录结构

## 9.1 根目录

```text
personal-knowledge-os/
├── README.md
├── package.json
├── pnpm-lock.yaml
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.ts
├── components.json
├── docs/
│   ├── PRODUCT_REQUIREMENTS.md
│   ├── ARCHITECTURE.md
│   ├── DATABASE_SCHEMA.md
│   ├── API_CONTRACT.md
│   ├── UI_SPECIFICATION.md
│   ├── DEVELOPMENT_RULES.md
│   └── ROADMAP.md
├── src/
└── src-tauri/
```

## 9.2 前端目录

```text
src/
├── app/
│   ├── App.tsx
│   ├── router.tsx
│   └── providers.tsx
├── components/
│   ├── layout/
│   ├── editor/
│   ├── notes/
│   ├── search/
│   ├── categories/
│   ├── tags/
│   └── common/
├── features/
│   ├── dashboard/
│   ├── notes/
│   ├── categories/
│   ├── search/
│   ├── workspace/
│   └── settings/
├── stores/
├── services/
├── hooks/
├── types/
├── lib/
└── styles/
```

## 9.3 Rust 目录

```text
src-tauri/src/
├── main.rs
├── lib.rs
├── commands/
├── core/
├── domain/
├── application/
├── infrastructure/
├── repositories/
└── migrations/
```

---

# 10. 领域模型

## 10.1 Workspace

```text
Workspace
- id
- name
- root_path
- created_at
- updated_at
```

## 10.2 Note

```text
Note
- id
- workspace_id
- title
- file_path
- category_id
- status
- content_hash
- word_count
- created_at
- updated_at
- last_indexed_at
```

## 10.3 Category

```text
Category
- id
- workspace_id
- parent_id
- name
- sort_order
- created_at
```

## 10.4 Tag

```text
Tag
- id
- workspace_id
- name
- created_at
```

## 10.5 SearchQuery

```text
SearchQuery
- workspace_id
- keyword
- category_id
- tag_id
- sort
- page
- page_size
```

---

# 11. 数据库设计

## 11.1 数据存储原则

- Markdown 保存正文
- SQLite 保存元数据
- FTS5 保存搜索索引
- 数据库可重建
- 文件路径使用相对路径
- 所有时间统一保存为 ISO 8601 UTC 或明确带时区的时间字符串

## 11.2 表结构

### workspaces

```sql
CREATE TABLE workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### categories

```sql
CREATE TABLE categories (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    parent_id TEXT,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,

    FOREIGN KEY (workspace_id)
        REFERENCES workspaces(id)
        ON DELETE CASCADE,

    FOREIGN KEY (parent_id)
        REFERENCES categories(id)
        ON DELETE SET NULL
);
```

### notes

```sql
CREATE TABLE notes (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    title TEXT NOT NULL,
    file_path TEXT NOT NULL,
    category_id TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    content_hash TEXT,
    word_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_indexed_at TEXT,

    UNIQUE (workspace_id, file_path),

    FOREIGN KEY (workspace_id)
        REFERENCES workspaces(id)
        ON DELETE CASCADE,

    FOREIGN KEY (category_id)
        REFERENCES categories(id)
        ON DELETE SET NULL
);
```

### tags

```sql
CREATE TABLE tags (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,

    UNIQUE (workspace_id, name),

    FOREIGN KEY (workspace_id)
        REFERENCES workspaces(id)
        ON DELETE CASCADE
);
```

### note_tags

```sql
CREATE TABLE note_tags (
    note_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,

    PRIMARY KEY (note_id, tag_id),

    FOREIGN KEY (note_id)
        REFERENCES notes(id)
        ON DELETE CASCADE,

    FOREIGN KEY (tag_id)
        REFERENCES tags(id)
        ON DELETE CASCADE
);
```

### settings

```sql
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

## 11.3 索引

```sql
CREATE INDEX idx_notes_workspace
ON notes(workspace_id);

CREATE INDEX idx_notes_category
ON notes(category_id);

CREATE INDEX idx_notes_updated_at
ON notes(updated_at DESC);

CREATE INDEX idx_categories_workspace
ON categories(workspace_id);

CREATE INDEX idx_categories_parent
ON categories(parent_id);
```

## 11.4 FTS5

```sql
CREATE VIRTUAL TABLE search_documents_fts
USING fts5(
    note_id UNINDEXED,
    title,
    content,
    keywords,
    tokenize = 'unicode61'
);
```

## 11.5 中文搜索策略

V1.0 必须通过真实中文笔记进行验证。

验证内容：

- 中文连续词搜索
- 中文短语搜索
- 英文与数字混合搜索
- 标题搜索
- 多关键词搜索
- 搜索结果排序

若默认 tokenizer 效果不足，再评估：

- 自定义分词
- 外部搜索引擎
- 预处理关键词字段
- 后续语义检索

---

# 12. Markdown 文件规范

## 12.1 工作区目录

```text
MyKnowledge/
├── .knowledge/
│   ├── database.sqlite
│   ├── config.json
│   └── index/
├── 00_Inbox/
├── 01_技术/
├── 02_工作/
├── 03_项目/
├── 04_生活/
├── 99_Archive/
└── attachments/
```

## 12.2 文件命名

建议：

```text
YYYYMMDD-HHmmss-title.md
```

或：

```text
note_<short-id>-title.md
```

文件名必须：

- 避免非法字符
- 避免路径穿越
- 避免重名
- 支持中文
- 长度受控

## 12.3 Frontmatter

```yaml
---
id: note_01JXYZ123
title: SQLite 全文搜索学习笔记
type: note
status: active
category: 技术/数据库
tags:
  - sqlite
  - search
created_at: 2026-09-23T12:00:00+08:00
updated_at: 2026-09-23T12:00:00+08:00
---
```

## 12.4 必填字段

```yaml
id:
title:
created_at:
updated_at:
```

## 12.5 可选字段

```yaml
type:
status:
category:
tags:
```

## 12.6 正文约定

支持：

- 标题
- 段落
- 列表
- 引用
- 代码块
- 表格
- 图片
- 链接
- 未来双向链接语法

允许：

```markdown
[[相关笔记]]
```

V1.0 可先按普通文本保存。

---

# 13. 存储与一致性设计

## 13.1 保存顺序

```text
用户编辑
  ↓
生成规范化 Markdown
  ↓
写入临时文件
  ↓
刷新文件
  ↓
原子替换目标文件
  ↓
更新 SQLite 元数据
  ↓
更新 FTS5 索引
  ↓
返回保存状态
```

## 13.2 失败处理

### 文件写入失败

- 不更新数据库
- 保留原文件
- 显示错误
- 写入日志

### 数据库更新失败

- 保留 Markdown 文件
- 标记需要重新扫描
- 下次启动时修复元数据

### 索引更新失败

- 不影响正文保存
- 标记索引过期
- 支持手动重建

## 13.3 外部修改

文件监听器发现变化后：

1. 判断是否为工作区内 Markdown 文件
2. 忽略临时文件
3. 读取文件
4. 解析 Frontmatter
5. 计算哈希
6. 更新元数据
7. 更新索引

---

# 14. API 契约

## 14.1 命名规范

- 命令使用 snake_case
- 输入对象使用明确结构
- 返回 DTO 不暴露数据库内部实现
- 错误使用统一结构

## 14.2 Workspace API

```text
workspace_create
workspace_open
workspace_close
workspace_get_current
workspace_list
```

## 14.3 Note API

```text
note_create
note_get
note_update
note_delete
note_list
note_move
note_duplicate
note_save
```

## 14.4 Category API

```text
category_create
category_update
category_delete
category_list
category_move
```

## 14.5 Tag API

```text
tag_create
tag_delete
tag_list
note_set_tags
```

## 14.6 Search API

```text
search_notes
search_rebuild_index
search_get_status
```

## 14.7 Backup API

```text
backup_create
backup_validate
backup_restore
export_markdown
```

## 14.8 示例 DTO

```typescript
export interface CreateNoteInput {
  workspaceId: string;
  title: string;
  categoryId?: string;
  initialContent?: string;
}

export interface NoteDto {
  id: string;
  workspaceId: string;
  title: string;
  filePath: string;
  categoryId?: string;
  status: string;
  createdAt: string;
  updatedAt: string;
}
```

## 14.9 统一错误结构

```typescript
export interface AppError {
  code: string;
  message: string;
  details?: unknown;
}
```

错误码示例：

```text
WORKSPACE_NOT_FOUND
WORKSPACE_ALREADY_EXISTS
NOTE_NOT_FOUND
NOTE_SAVE_FAILED
FILE_ACCESS_DENIED
PATH_INVALID
DATABASE_ERROR
SEARCH_INDEX_ERROR
BACKUP_FAILED
```

---

# 15. 前端架构

## 15.1 状态分类

### UI 状态

使用 Zustand：

- 侧边栏开关
- 编辑器模式
- 当前选中笔记
- 搜索关键词
- 面板宽度

### 服务端数据

通过 Tauri API 获取：

- 笔记
- 分类
- 标签
- 搜索结果
- 工作区信息

### 编辑器内部状态

由编辑器管理：

- 光标
- 撤销栈
- 临时文本
- 选区

## 15.2 前端组件

```text
AppShell
├── Sidebar
│   ├── WorkspaceSwitcher
│   ├── NavigationMenu
│   ├── CategoryTree
│   └── TagList
├── TopBar
│   ├── GlobalSearch
│   ├── SaveStatus
│   └── SettingsButton
└── MainWorkspace
    ├── NoteListPanel
    └── EditorPanel
```

---

# 16. UI 页面设计

## 16.1 总体布局

采用三栏工作台：

```text
┌─────────────────────────────────────────────────────────────┐
│ Logo / 知识库名称       全局搜索              设置 / 状态     │
├──────────────┬──────────────────────┬───────────────────────┤
│ 侧边导航     │ 笔记列表              │ 编辑器                │
│              │                      │                       │
│ 首页         │ 搜索 / 筛选           │ 标题                  │
│ 全部笔记     │ 笔记 A                │ Markdown 编辑区       │
│ Inbox        │ 笔记 B                │                       │
│ 分类         │ 笔记 C                │                       │
│ 标签         │                      │                       │
│ 收藏         │                      │                       │
└──────────────┴──────────────────────┴───────────────────────┘
```

建议默认宽度：

- 左栏：220px
- 中栏：300px
- 右栏：剩余空间

## 16.2 页面清单

| 页面 | 路由 | 优先级 |
|---|---|---|
| 首页 | `/` | P0 |
| 全部笔记 | `/notes` | P0 |
| 笔记编辑 | `/notes/:id` | P0 |
| 搜索 | `/search` | P0 |
| Inbox | `/inbox` | P0 |
| 分类管理 | `/categories` | P0 |
| 设置 | `/settings` | P1 |
| 标签管理 | `/tags` | P1 |
| 关于 | `/about` | P2 |

## 16.3 首页

首页包含：

- 快速搜索
- 新建笔记
- Inbox 入口
- 最近编辑
- 常用分类
- 当前工作区信息

不建议第一版加入复杂图表。

## 16.4 笔记列表

每一项显示：

- 标题
- 更新时间
- 分类
- 标签
- 内容摘要

支持：

- 单击选中
- 双击打开
- 右键菜单
- 新建
- 删除
- 移动
- 排序

## 16.5 编辑器

支持：

- Markdown 编辑
- 编辑/预览切换
- 分屏模式
- 自动保存
- 手动保存
- 撤销/重做
- 代码块
- 图片路径
- 快捷键

编辑器状态：

- 未保存
- 保存中
- 已保存
- 保存失败

## 16.6 搜索页面

支持：

- 全局搜索框
- 搜索结果列表
- 标题和摘要
- 关键词高亮
- 分类过滤
- 标签过滤
- 时间排序

## 16.7 设置页面

V1.0 设置：

- 默认工作区
- 自动保存开关
- 自动保存延迟
- 编辑器主题
- 字体大小
- 默认新建位置
- 备份目录
- 关于与版本信息

---

# 17. 交互规范

## 17.1 快捷键

| 操作 | Windows | macOS |
|---|---|---|
| 新建笔记 | Ctrl+N | Cmd+N |
| 全局搜索 | Ctrl+K | Cmd+K |
| 保存 | Ctrl+S | Cmd+S |
| 关闭当前笔记 | Ctrl+W | Cmd+W |
| 删除笔记 | Delete | Delete |
| 聚焦编辑器 | Ctrl+E | Cmd+E |

## 17.2 反馈原则

所有重要操作必须提供反馈：

- 保存成功
- 保存失败
- 删除成功
- 移动成功
- 索引进行中
- 索引完成
- 备份成功
- 备份失败

## 17.3 空状态

每个列表必须设计空状态：

- 没有笔记
- 没有搜索结果
- 没有分类
- 没有标签
- 没有打开工作区

空状态应提供下一步操作按钮。

---

# 18. 搜索系统设计

## 18.1 搜索流程

```text
用户输入关键词
  ↓
前端防抖
  ↓
调用 search_notes
  ↓
构建安全查询
  ↓
FTS5 检索
  ↓
排序
  ↓
生成摘要和高亮
  ↓
返回结果
```

## 18.2 搜索排序

建议排序因素：

1. 标题匹配优先
2. 完整短语匹配优先
3. BM25 相关性
4. 最近更新时间作为次级排序

## 18.3 搜索安全

- 不直接拼接未经处理的 SQL
- 对 FTS 查询语法进行处理
- 处理空字符串
- 限制查询长度
- 防止异常搜索表达式导致崩溃

## 18.4 索引生命周期

### 创建笔记

立即建立索引。

### 更新笔记

保存后更新索引。

### 删除笔记

从索引移除。

### 外部修改

文件监听器触发更新。

### 数据库打开

检查索引状态。

### 手动重建

清理并重新扫描全部 Markdown 文件。

---

# 19. 文件监听设计

## 19.1 监听范围

只监听当前工作区中的：

- `.md`
- `.markdown`
- 必要的附件目录变化

忽略：

- `.knowledge`
- 临时文件
- 隐藏系统文件
- 备份目录
- 回收站目录

## 19.2 事件去重

文件系统通常会产生多个重复事件。

需要：

- 防抖
- 事件合并
- 延迟读取
- 忽略应用自身写入事件

## 19.3 冲突处理

如果用户在应用中编辑，同时外部修改同一文件：

V1.0 可采用：

1. 检测内容哈希变化
2. 提示用户
3. 提供重新加载或保留当前内容选项

不要静默覆盖用户内容。

---

# 20. 数据安全与备份

## 20.1 删除策略

```text
删除笔记
  ↓
移动到 .trash
  ↓
更新状态
  ↓
从搜索结果移除
  ↓
支持恢复
```

## 20.2 备份内容

- Markdown
- 附件
- SQLite
- 配置
- 版本信息

## 20.3 备份格式

```text
KnowledgeBackup_YYYYMMDD_HHmmss.zip
```

## 20.4 备份验证

备份完成后检查：

- 文件数量
- 总大小
- 压缩包可打开
- 数据库存在
- Markdown 数量一致
- 附件数量一致

## 20.5 数据恢复原则

恢复操作必须：

- 明确提示目标目录
- 防止覆盖现有数据
- 支持恢复前预览
- 失败时保留原数据

---

# 21. 日志与错误处理

## 21.1 日志等级

- ERROR：不可恢复错误
- WARN：可恢复异常
- INFO：关键生命周期事件
- DEBUG：开发调试信息

## 21.2 日志内容

记录：

- 时间
- 模块
- 操作
- 错误码
- 简短上下文

不要记录：

- 用户笔记正文
- 敏感个人信息
- 完整文件内容

## 21.3 错误处理原则

- 不吞掉错误
- 不向用户显示技术堆栈
- 内部日志保留诊断信息
- 用户界面显示可理解的提示
- 关键错误提供恢复操作

---

# 22. 测试方案

## 22.1 单元测试

覆盖：

- ID 生成
- 文件名规范化
- Frontmatter 解析
- Markdown 序列化
- 路径安全检查
- 数据库 CRUD
- 搜索查询构建
- 哈希计算

## 22.2 集成测试

覆盖：

- 创建工作区
- 创建笔记
- 保存笔记
- 更新数据库
- 更新索引
- 外部文件修改
- 删除和恢复
- 索引重建

## 22.3 UI 测试

覆盖：

- 新建笔记
- 编辑和保存
- 创建分类
- 移动笔记
- 搜索笔记
- 删除笔记
- 打开工作区

## 22.4 跨平台测试

Windows：

- 安装
- 启动
- 文件读写
- 快捷键
- 打包

macOS：

- 安装
- 启动
- 文件读写
- 快捷键
- 权限提示
- 打包

## 22.5 数据恢复测试

必须测试：

- 保存中断
- 数据库损坏
- 索引损坏
- 文件被外部删除
- 文件被外部重命名
- 应用异常退出
- 重复打开工作区

---

# 23. AI 编程协作规范

## 23.1 AI 工作原则

AI 编程工具必须：

1. 先阅读相关设计文档
2. 先输出实施计划
3. 只修改必要文件
4. 不擅自改变架构
5. 增加必要测试
6. 运行构建和测试
7. 输出变更摘要
8. 报告未解决问题

## 23.2 推荐提示词模板

```text
请先阅读以下文档：
- ARCHITECTURE.md
- DATABASE_SCHEMA.md
- DEVELOPMENT_RULES.md

本次任务：
[填写单一功能]

要求：
1. 先说明实施方案
2. 列出预计修改文件
3. 不修改无关模块
4. 遵循现有接口和命名规范
5. 增加必要测试
6. 运行检查
7. 最后输出：
   - 修改文件
   - 实现内容
   - 测试结果
   - 已知问题
```

## 23.3 禁止行为

AI 不得：

- 未经确认引入新框架
- 直接删除已有数据
- 修改数据库结构而不提供迁移
- 绕过 Repository 直接访问数据库
- 将业务逻辑全部写在 UI 组件中
- 为了通过编译而删除测试
- 修改无关模块
- 自动引入 AI 或云服务

---

# 24. Git 工作流

## 24.1 分支

```text
main
develop
feature/workspace
feature/markdown-storage
feature/note-editor
feature/categories
feature/search
```

个人开发可简化为：

```text
main
feature/*
```

## 24.2 提交规范

```text
feat: add workspace creation
feat: implement markdown storage
feat: add note editor
feat: add category tree
feat: implement full text search
fix: handle duplicate file events
test: add note repository tests
docs: update database schema
```

## 24.3 提交要求

每次提交应：

- 只包含一个主题
- 可以编译
- 不破坏已有功能
- 说明测试结果

---

# 25. 开发路线

## Phase 0：技术验证

交付：

- 开发环境
- Tauri 启动
- SQLite 读写
- Markdown 保存
- CodeMirror 验证
- FTS5 验证
- Windows/macOS 构建验证

## Phase 1：基础设施

交付：

- 项目骨架
- 数据库迁移
- 工作区管理
- 日志
- 错误处理
- 路径安全

## Phase 2：笔记闭环

交付：

- 新建
- 编辑
- 保存
- 自动保存
- 打开
- 删除
- 重命名

## Phase 3：组织能力

交付：

- 分类树
- 分类 CRUD
- 笔记移动
- 标签
- Inbox
- 最近编辑

## Phase 4：搜索

交付：

- 标题搜索
- 正文搜索
- 结果摘要
- 高亮
- 过滤
- 索引更新
- 索引重建

## Phase 5：稳定性

交付：

- 备份
- 导出
- 恢复
- 文件监听
- 冲突提示
- 异常恢复

## Phase 6：跨平台发布

交付：

- Windows 安装包
- macOS 安装包
- 图标
- 版本信息
- 安装与升级验证

---

# 26. MVP 验收标准

## 26.1 工作区

- [ ] 可以创建工作区
- [ ] 可以打开已有工作区
- [ ] 可以切换工作区
- [ ] 工作区关闭后资源释放正常

## 26.2 笔记

- [ ] 可以新建笔记
- [ ] 可以编辑笔记
- [ ] 可以保存笔记
- [ ] 自动保存正常
- [ ] 重启后内容完整
- [ ] 可以删除和恢复
- [ ] 可以重命名
- [ ] 可以移动

## 26.3 分类

- [ ] 可以创建分类
- [ ] 可以创建子分类
- [ ] 可以重命名
- [ ] 可以删除
- [ ] 可以按分类查看笔记

## 26.4 搜索

- [ ] 标题搜索有效
- [ ] 正文搜索有效
- [ ] 结果有摘要
- [ ] 关键词有高亮
- [ ] 索引可以重建
- [ ] 中文搜索经过真实数据验证

## 26.5 安全

- [ ] 保存失败不覆盖原文件
- [ ] 删除可恢复
- [ ] 备份可打开
- [ ] Markdown 可独立读取
- [ ] 数据库损坏后可重建索引

## 26.6 发布

- [ ] Windows 可安装
- [ ] Windows 可运行
- [ ] macOS 可安装
- [ ] macOS 可运行
- [ ] 基础功能在两平台一致

---

# 27. 未来版本规划

## V1.1：体验增强

可能包含：

- 更好的中文搜索
- 批量操作
- 收藏
- 搜索历史
- 图片拖拽
- 更完善的回收站
- 导入工具

## V2.0：知识关联

可能包含：

- 双向链接
- 反向链接
- 知识图谱
- 文档关系
- 模板系统
- 更多导入格式

## V3.0：AI 能力

可能包含：

- 语义搜索
- 本地模型
- AI 摘要
- 自动标签
- 知识问答
- 基于引用的回答

## 商业化版本

可能包含：

- 授权管理
- 订阅
- 云同步
- 团队空间
- 权限体系
- 企业私有部署
- 插件系统

---

# 28. 架构演进原则

未来扩展必须遵循：

1. 不破坏已有 Markdown 数据
2. 不强制用户迁移到专有格式
3. 不让 AI 成为核心数据依赖
4. 不让云端成为基本使用前提
5. 不让商业授权影响数据导出
6. 不让插件直接拥有无限文件权限
7. 所有重大数据结构变化必须有迁移方案

---

# 29. 第一阶段开发任务清单

## Sprint 1：项目初始化

- [ ] 初始化 Git
- [ ] 创建 Tauri + React 项目
- [ ] 配置 TypeScript strict
- [ ] 配置 ESLint/Formatter
- [ ] 建立 docs 目录
- [ ] 建立基础 UI Shell
- [ ] 验证开发启动

## Sprint 2：数据库与工作区

- [ ] SQLite 初始化
- [ ] 数据库迁移
- [ ] Workspace 表
- [ ] Notes 表
- [ ] Categories 表
- [ ] Tags 表
- [ ] 创建工作区
- [ ] 打开工作区

## Sprint 3：Markdown 存储

- [ ] Markdown 文件创建
- [ ] Frontmatter 解析
- [ ] 文件名规范化
- [ ] 文件安全写入
- [ ] 内容哈希
- [ ] 读取和更新

## Sprint 4：笔记编辑器

- [ ] CodeMirror 集成
- [ ] 编辑模式
- [ ] 预览模式
- [ ] 自动保存
- [ ] 保存状态
- [ ] 快捷键

## Sprint 5：分类与列表

- [ ] 笔记列表
- [ ] 分类树
- [ ] 创建分类
- [ ] 移动笔记
- [ ] Inbox
- [ ] 最近编辑

## Sprint 6：搜索

- [ ] FTS5 表
- [ ] 索引服务
- [ ] 标题搜索
- [ ] 正文搜索
- [ ] 摘要
- [ ] 高亮
- [ ] 索引重建

## Sprint 7：稳定性

- [ ] 文件监听
- [ ] 删除恢复
- [ ] 备份
- [ ] 导出
- [ ] 错误日志
- [ ] 恢复测试

## Sprint 8：发布

- [ ] Windows 打包
- [ ] macOS 打包
- [ ] 安装测试
- [ ] 数据读写测试
- [ ] 用户验收
- [ ] 发布说明

---

# 30. 最终工程决策

## 30.1 正式技术基线

```text
Tauri 2
+ React
+ TypeScript
+ Vite
+ shadcn/ui
+ Tailwind CSS
+ CodeMirror 6
+ Rust
+ SQLite
+ FTS5
+ Markdown
```

## 30.2 正式架构基线

```text
模块化单体
+ 本地优先
+ Markdown 正文
+ SQLite 元数据
+ FTS5 搜索
+ 可重建索引
+ 可替换 Repository
```

## 30.3 正式产品基线

```text
第一版只做好：

记录
分类
搜索

同时保证：

数据自主
跨平台
可备份
可扩展
可维护
```

## 30.4 项目成功标准

Personal Knowledge OS V1.0 不以功能数量作为成功标准，而以以下结果作为成功标准：

> 用户愿意每天使用它记录知识，并且在未来需要时能够快速找回曾经记录的内容。

---

# 附录 A：AI 编程首个任务

```text
你现在是 Personal Knowledge OS 项目的资深桌面软件架构师和开发工程师。

请阅读项目根目录中的：
- Personal-Knowledge-OS-Engineering-Design-V1.0.md
- docs/DEVELOPMENT_RULES.md

现在只执行“项目初始化”任务：

1. 创建 Tauri 2 + React + TypeScript + Vite 项目
2. 建立前端和 Rust 目录结构
3. 配置 TypeScript strict mode
4. 配置基础代码规范
5. 创建基础三栏 UI Shell
6. 暂时不实现数据库、笔记和搜索业务
7. 确保项目能够启动
8. 运行构建检查

执行前先输出：
- 实施计划
- 修改文件列表
- 风险点

执行后输出：
- 实际修改文件
- 构建结果
- 未解决问题
- 下一步建议
```

# 附录 B：版本记录

| 版本 | 日期 | 说明 |
|---|---|---|
| V1.0 | 2026-09-23 | 初始工程设计基线 |
| V1.0.1（实现状态附录） | 2026-10-07 | 正文不变，新增附录 C：实现状态与偏差记录；同步 `docs/` 配套文档 |

---

# 附录 C：实现状态与偏差记录

> 目的：在**不改动正文基线**的前提下，记录实现与本文档的差异、超出 V1.0 范围的增补、
> 各 Sprint 的完成情况，以及待评估项。新增偏差必须在此登记后再编码。
> 详细实现说明见 `docs/ARCHITECTURE.md`（第 5 节列出偏差清单）；
> 用户可见变化见 `docs/CHANGELOG.md`。

## C.1 版本与状态

| 项目 | 状态 |
|---|---|
| 文档基线 | V1.0（2026-09-23） |
| 实现版本 | **V1.2.0**（2026-10-07） |
| 已交付 | 第 4.1 节「V1.0 必须实现」全部条目；另有附录 C.3 的增补 |
| 开发中 | 无（V1.1 的 M1/M2/M3、V1.2.0 的知识资产/导入/性能批次均已完成）；M4 的 Windows 打包等待指令 |
| 明确未做 | 第 4.2 节「V1.0 不实现」全部条目（AI、云同步、协作、移动端等）保持不做 |
| 下一阶段 | V1.1：双向链接与编辑体验，见 `docs/PLAN_V1.1.md` |

## C.2 与本文档的偏差

| 编号 | 本文档要求 | 实现方式 | 原因 |
|---|---|---|---|
| D1 | 10.2 / 11.2 的 `notes` 字段清单 | 增加 `favorite`（收藏）与 `trash_path`（回收站位置）两列，其余字段一致，并提供迁移 | 支撑 FR-N05 删除到回收站与侧边栏「收藏」入口；避免把回收站路径写进正文 |
| D2 | 12.3 使用 `note_01JXYZ123` 形式 ID | 自实现 26 位 Crockford Base32 ULID，同毫秒内单调递增 | 保持文档形式且保证字符串排序 = 时间序，减少依赖 |
| D3 | 12.4 必填字段 `id`/`title`/`created_at`/`updated_at` | 打开已有目录时**不改写**用户文件；缺失字段在首次保存时补全，id 通过「路径 → 记录」复用保持稳定 | 遵守「不破坏用户既有文件」；避免每次扫描重写文件 |
| D4 | 12.6 正文约定 | 正文首尾空行规范化；`tags` 输出为缩进块序列；frontmatter 未知字段（含嵌套）原样保留 | 与 12.3 示例风格一致，且不丢用户自定义元数据 |
| D5 | 19.x 文件监听使用 `notify` | 优先原生事件；不可用时自动降级为 `PollWatcher`（3 秒）并在界面提示 | 沙箱、网络盘等环境收不到原生事件，需保证「外部编辑」场景可用 |
| D6 | 16.2 页面路由 | 使用 HashRouter（路由语义与文档一致） | 桌面外壳以静态资源加载，哈希路由避免刷新 404 |
| D7 | 12.1 工作区目录 | `00_Inbox` 代表「未分类」，不为其创建分类记录（其余目录约定一致） | Inbox 是虚拟位置而非分类，避免出现名为 Inbox 的分类 |
| D8 | 7.1 导出能力 | 额外提供 Word(`.docx`) 与 HTML 导出；PDF 通过「打印视图 + 系统打印面板」另存 | 若自行排版 PDF 需引入大型引擎与字体嵌入，体积与许可成本高；docx 可再另存 PDF |
| D9 | 11.1 时间字符串 | `settings` 中新增 `editorSplitRatio`（旧数据缺省 0.5）；数值型设置放宽为「整数或小数均可，四舍五入」 | 支持分屏比例记忆；兼容 Retina 屏拖拽产生的浮点像素值 |

## C.3 超出 V1.0 范围的增补（V1.0.1）

以下能力不在正文 4.1 的清单内，但属于同一阶段的产品闭环，已实现并纳入验收：

| 增补 | 说明 | 对应文档 |
|---|---|---|
| 外部文件导入 | `.md`/`.markdown`/`.txt` 文件与目录递归导入；保留 frontmatter 元数据；复制语义 | `docs/API_CONTRACT.md`、`docs/UI_SPECIFICATION.md` |
| 单篇多格式导出 | Markdown / HTML / Word(docx) / PDF 打印视图 | 同上 |
| 三处可拖拽分栏 | 侧边栏、笔记列表、编辑与预览比例；双击复位、方向键微调 | `docs/UI_SPECIFICATION.md` |
| 布局与视图记忆 | 分栏宽度、分屏比例、主题、字号、视图模式随工作区保存 | `docs/DATABASE_SCHEMA.md`（settings 键） |
| 宽屏布局优化 | 首页/设置/分类/标签页占满宽度并使用两列布局 | `docs/UI_SPECIFICATION.md` |

### C.3.1 超出 V1.0 范围的增补（V1.1 M1：双向链接）

正文未定义双向链接（原计划 V2.0「知识图谱」的前置能力），V1.1 提前实现数据层与界面，
图谱可视化仍按 C.5 待评估。

| 增补 | 说明 | 对应文档 |
|---|---|---|
| `note_links` 表（迁移 003） | 链接表定义为**可重建派生数据**，不参与正文存储 | `docs/DATABASE_SCHEMA.md` |
| 双向链接解析器 | `[[标题]]` / `[[标题\|别名]]` / `[[标题#小节]]` / 相对 Markdown 链接；忽略代码块与行内代码 | `docs/API_CONTRACT.md` |
| `LinkService` + 5 个 `links_*` 命令 | 增量维护、重命名重连并改写正文、删除解绑/恢复重绑、全量重建 | `docs/ARCHITECTURE.md` 5.1 |
| 反向链接面板与引用计数 | 编辑器底部面板、元信息栏计数、首页未解析链接卡片 | `docs/UI_SPECIFICATION.md` |

### C.3.2 超出 V1.0 范围的增补（V1.1 M2-1：编辑体验）

正文的编辑器一章只定义了「编辑 / 预览 / 自动保存」，V1.1 增补了下列高频能力：

| 增补 | 说明 | 对应文档 |
|---|---|---|
| 图片与附件粘贴落盘 | 粘贴/拖入图片写入 `attachments/YYYY-MM/`，正文插入相对链接；预览内联显示 | `docs/ARCHITECTURE.md` 5.2 |
| 附件命令 | `attachment_save`（20 MB 上限）、`attachment_read`（仅工作区内相对路径，预览 8 MB 上限） | `docs/API_CONTRACT.md` |
| 编辑器大纲 | 解析 `#` 层级生成右侧大纲，点击跳转 | `docs/UI_SPECIFICATION.md` |
| `⌘/Ctrl + P` 快速切换 | 按标题模糊匹配快速打开笔记（与 ⌘K 的正文检索分工） | `docs/UI_SPECIFICATION.md` |
| `[[` 自动补全 | 输入 `[[` 弹出标题候选并自动补 `]]` | `docs/UI_SPECIFICATION.md` |
| 预览 chip | `[[标题]]` 渲染为 chip：已解析跳转、未解析一键创建 | `docs/ARCHITECTURE.md` 5.1 |
| 本地模板 | `.knowledge/templates/*.md` + 新建时选择 + `{{title}}`/`{{date}}` 等占位符 | `docs/API_CONTRACT.md` 第 11 节 |
| 知识图谱视图 | `graph_get` 节点/边/过滤/聚焦 + `/graph` Canvas 力导向布局（提前自 V1.2） | `docs/API_CONTRACT.md` 第 12 节 |
| 知识资产元数据 | `notes` 新增来源/时效（004）与文件快照（005）列；`note_relations` 类型化关系（004） | `docs/DATABASE_SCHEMA.md` |
| FTS rowid 映射 | `search_documents_map`（006）：避免按 UNINDEXED 的 note_id 全表删除（1 万篇首次导入 6.3 分钟 → 4 秒） | `docs/DATABASE_SCHEMA.md` |
| AI 决策层框架 | 只定义分层与接口（决策层 / 模型接入层 / 检索层 / 工具层）与隐私开关，不含模型调用 | `docs/AI_ARCHITECTURE_PLAN.md` |

## C.4 第 29 章 Sprint 完成情况

| Sprint | 内容 | 状态 |
|---|---|---|
| Sprint 1 | 项目初始化（Git、Tauri + React、TS strict、Lint、docs、UI Shell） | ✅ |
| Sprint 2 | 数据库与工作区（迁移、四张表、创建/打开工作区） | ✅ |
| Sprint 3 | Markdown 存储（文件名规范化、frontmatter、原子写入、哈希） | ✅ |
| Sprint 4 | 编辑器（CodeMirror、预览、自动保存、状态、快捷键） | ✅ |
| Sprint 5 | 分类与列表（分类树、创建、移动、Inbox、最近编辑） | ✅ |
| Sprint 6 | 搜索（FTS5、索引服务、标题/正文、摘要、高亮、重建） | ✅ |
| Sprint 7 | 稳定性（文件监听、删除恢复、备份、导出、日志） | ✅（另见 C.2 / C.3） |
| Sprint 8 | 发布（Windows/macOS 打包、安装测试、验收） | 🟡 macOS 已完成；Windows 安装包为 V1.0.0 构建，等待同步指令后再打包并做真机验收 |

## C.5 待评估与后续

| 项目 | 结论 |
|---|---|
| 中文分词增强（11.5） | 暂不引入分词依赖：先以 1 万笔记基准测量索引体积与召回率，达到阈值再评估自定义分词或预分词字段 |
| 代码签名与自动更新 | 暂不需要：当前以本地运行与自测为主 |
| 知识图谱（27 章 V2.0） | M1/M2 已完成；图谱视图经确认提前到 V1.1.0 交付（节点/边/过滤/聚焦 + Canvas 力导向） |
| AI 能力（V3.0） | 已产出框架规划（AI 决策层 + 模型接入层 + 检索层 + 工具层），**V1.1.0 不引入任何模型调用**；落地排期与隐私开关见 `docs/AI_ARCHITECTURE_PLAN.md` |

---

**文档结束**
