# 数据库设计

对应设计文档第 11 章。数据库文件位于工作区 `.knowledge/database.sqlite`，
开启 `journal_mode = WAL`、`synchronous = NORMAL`、`foreign_keys = ON`，连接级 `busy_timeout = 5s`。

## 1. 表结构

### workspaces

```sql
CREATE TABLE IF NOT EXISTS workspaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    root_path TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

一个工作区目录对应一行记录；`root_path` 唯一，避免同一目录被重复初始化。

### categories

```sql
CREATE TABLE IF NOT EXISTS categories (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    parent_id TEXT,
    name TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    dir_name TEXT,              -- 目录名（顶层含 01_ 之类排序前缀）
    created_at TEXT NOT NULL,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_id) REFERENCES categories(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX idx_categories_name_unique
    ON categories(workspace_id, IFNULL(parent_id, ''), lower(name));
```

同级分类名不区分大小写唯一。`dir_name` 是**单级目录名**，完整目录由祖先链拼接得到。

### notes

```sql
CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    title TEXT NOT NULL,
    file_path TEXT NOT NULL,       -- 工作区相对路径，使用 '/'
    category_id TEXT,
    status TEXT NOT NULL DEFAULT 'active',   -- active | deleted | missing
    content_hash TEXT,
    word_count INTEGER NOT NULL DEFAULT 0,
    favorite INTEGER NOT NULL DEFAULT 0,
    trash_path TEXT,               -- 回收站相对路径
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_indexed_at TEXT,
    UNIQUE (workspace_id, file_path),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE SET NULL
);
```

`content_hash` 为整份 Markdown 文件的 SHA-256，用于保存冲突检测与外部修改识别。

### tags / note_tags / settings

```sql
CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (workspace_id, name),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX idx_tags_name_unique ON tags(workspace_id, lower(name));

CREATE TABLE IF NOT EXISTS note_tags (
    note_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    PRIMARY KEY (note_id, tag_id),
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

`settings` 保存工作区级设置 JSON 与索引状态：
`app.settings`、`index.state`、`index.last_rebuilt_at`、`index.last_error`。

### note_links（迁移 003，双向链接）

```sql
CREATE TABLE IF NOT EXISTS note_links (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id   TEXT NOT NULL,
    source_note_id TEXT NOT NULL,
    target_note_id TEXT,              -- 解析不到目标时为 NULL（未解析链接）
    target_title   TEXT NOT NULL,     -- 链接原文，便于重命名后重连
    target_anchor  TEXT,              -- [[标题#小节]]
    link_type      TEXT NOT NULL DEFAULT 'wikilink',  -- wikilink | markdown | url
    context        TEXT,              -- 命中行上下文（≤ 120 字，以链接为中心截断）
    position       INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL,
    UNIQUE (source_note_id, target_title, position)
);

CREATE INDEX idx_note_links_source ON note_links(source_note_id);
CREATE INDEX idx_note_links_target ON note_links(target_note_id);
CREATE INDEX idx_note_links_title  ON note_links(workspace_id, lower(target_title));
CREATE INDEX idx_note_links_type   ON note_links(link_type);
```

要点：该表是**可重建的派生数据**（`links_rebuild` 可从 Markdown 全量恢复），
外链以 `link_type='url'` 记录但不进入图谱；`target_note_id` 允许为空以支持
「未解析链接」。解析规则见 [PLAN_V1.1.md](./PLAN_V1.1.md) 的 F1。

### note_relations（迁移 004，类型化关系 · 预留）

```sql
CREATE TABLE IF NOT EXISTS note_relations (
    id             TEXT PRIMARY KEY,
    workspace_id   TEXT NOT NULL,
    source_note_id TEXT NOT NULL,
    target_note_id TEXT NOT NULL,
    relation_type  TEXT NOT NULL,   -- SUPERSEDES / CONTRADICTS / DERIVED_FROM / ...
    note           TEXT,            -- 用户写的说明
    position       INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL,
    UNIQUE (source_note_id, target_note_id, relation_type)
);
```

要点：与 `note_links` 同样属于**派生数据**，事实来源是来源笔记 frontmatter 里的 `relations:`；
重命名 / 删除 / 重建都会按正文重新生成。**当前版本已建表并完成数据模型预留，
服务与界面（关系面板、图谱边样式）在下一轮实现。**

### notes 的来源与时效列（迁移 004）

| 列 | 含义 | 对应 frontmatter |
| --- | --- | --- |
| `source_type` | 来源类型（official / book / experiment / personal …） | `source_type` |
| `source_ref` | 来源引用（URL、书名、出处） | `source_ref` |
| `source_reliability` | 可靠性 1–5 | `source_reliability` |
| `valid_until` | 时效（超过该日期提示「可能已过期」） | `valid_until` |

这些列以文件为准：应用内修改会写回 frontmatter，外部编辑器改动会在扫描时同步回库。

### notes 的文件快照列（迁移 005）

| 列 | 含义 |
| --- | --- |
| `file_size` | 文件字节数快照 |
| `file_mtime` | 文件修改时间快照（ISO 8601） |

用途：扫描/监听先用「mtime + size」比对，未变化的文件直接短路，
不再读取全文与计算 sha256（1 万篇规模下稳态扫描的主要成本）。

### 索引

```sql
CREATE INDEX idx_notes_workspace ON notes(workspace_id);
CREATE INDEX idx_notes_category ON notes(category_id);
CREATE INDEX idx_notes_updated_at ON notes(updated_at DESC);
CREATE INDEX idx_notes_status ON notes(status);
CREATE INDEX idx_notes_favorite ON notes(favorite);
CREATE INDEX idx_categories_workspace ON categories(workspace_id);
CREATE INDEX idx_categories_parent ON categories(parent_id);
CREATE INDEX idx_tags_workspace ON tags(workspace_id);
CREATE INDEX idx_note_tags_tag ON note_tags(tag_id);
```

## 2. 全文索引（FTS5）

```sql
CREATE VIRTUAL TABLE search_documents_fts USING fts5(
    note_id UNINDEXED,
    title,
    content,
    keywords,
    tokenize = 'unicode61 remove_diacritics 2'
);
```

写入规则（`SearchDocument::build`）：

| 列 | 内容 |
| --- | --- |
| `title` | 原始标题 |
| `content` | Markdown 抽取的纯文本（最长 20 万字符） |
| `keywords` | 标签名（小写）+ 分类路径 + **中文单字与相邻双字扩展** |

## 3. 中文检索策略（设计文档 11.5）

`unicode61` 会把「全文搜索」当成一个整词，因此「全文」无法命中。解决方案：

1. 建索引时把中日韩连续片段展开为**单字**（支持单字查询）与**相邻双字**；
2. 查询时把中文片段转换为**双字短语**，例如「全文搜索」→ `"全文 文搜 搜索"`；
3. 单字查询 → 单字词条；英文与数字 → 前缀匹配（`sqlite*`）。

已验证场景：中文连续词、中文短语、中文单字、英文与数字混合、标题命中优先。
索引体积约为中文正文的 2~3 倍，属于可接受范围（V1.1 可视数据规模再评估分词器）。

## 4. 迁移

- 迁移脚本：`src-tauri/migrations/*.sql`，通过 `include_str!` 编入二进制；
- 记录表：`schema_migrations(version, name, applied_at)`，同时写入 `PRAGMA user_version`；
- 打开工作区时若 `user_version` 高于当前应用支持版本 → 返回 `WORKSPACE_INVALID`（提示升级应用）；
- 索引可重建：`search_rebuild_index` 清空 FTS5 并按 `notes` 重新写入，不依赖外部文件。

当前迁移列表：`001_init`（表结构）、`002_search_index`（FTS5）、`003_note_links`（双向链接），
`SCHEMA_VERSION = 3`。

## 5. 数据恢复能力

| 故障 | 处理 |
| --- | --- |
| 索引损坏 | 手动重建索引（FTS5 表可整体清空） |
| 元数据损坏 | 删除 `.knowledge/database.sqlite` 后重新打开工作区，会重新扫描 Markdown 并采纳 |
| 文件被外部删除 | 记录标记为 `missing`，索引移除，界面提示 |
| 保存中断 | 原子写入：临时文件 + rename，原文件保持完整 |
| 误删笔记 | `.trash/` 保留原相对路径，可恢复或永久删除 |
