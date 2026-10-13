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

### search_documents_map（迁移 006）

```sql
CREATE TABLE IF NOT EXISTS search_documents_map (
    note_id TEXT PRIMARY KEY,
    row_id  INTEGER NOT NULL UNIQUE
);
```

用途：FTS5 表的 `note_id` 是 `UNINDEXED` 列，按它删除会全表扫描（逐篇保存/采纳退化为 O(N²)）；
映射表把 `note_id` 关联到 FTS 的 **rowid**，删除改走主键。迁移时会清空既有 FTS 索引，
应用下次打开工作区自动重建一次。

### note_chunks（迁移 011，分块索引 · N5）

```sql
CREATE TABLE IF NOT EXISTS note_chunks (
    note_id     TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    heading     TEXT NOT NULL,   -- 标题路径（`A > B`）；空串 = 位于任何标题之前
    char_start  INTEGER NOT NULL,
    char_end    INTEGER NOT NULL,
    PRIMARY KEY (note_id, chunk_index)
);
CREATE INDEX IF NOT EXISTS idx_note_chunks_note ON note_chunks(note_id);
```

用途：让**检索命中能跳回原文的精确区间**——命中带 `SearchHit.location`
（`{ heading, charStart, charEnd }`），编辑器据此滚过去并选中那一段。

- 只存"分块在哪里"，**不存正文**：正文是 Markdown 文件的投影，删库重扫可完整重建；
- 区间是**正文（不含 frontmatter）里的字符下标**（Unicode 标量值），
  与 `read_note` 的 `body`、编辑器里的正文同一坐标系；
- 分块规则：标题行断开；一段超上限（900 字）时**从行边界**断开（绝不切断一行）。
  实现见 `src-tauri/src/core/chunk.rs`；
- **升级路径**：老库「有笔记索引、没有分块」时，打开工作区会判定索引落后并**自动重建**一次。

### decision_cache（迁移 012，决策缓存 · D2/N6）

```sql
CREATE TABLE IF NOT EXISTS decision_cache (
    cache_key  TEXT PRIMARY KEY,   -- sha256(kind + 查询 + 候选内容 + 候选链档位)
    kind       TEXT NOT NULL,
    provider   TEXT NOT NULL,
    payload    TEXT NOT NULL,      -- DecisionResult[] 的 JSON
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    hits       INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_decision_cache_expires ON decision_cache(expires_at);
```

用途：同一批材料在一次研究会话里会被反复判定（打分 / 冲突 / 后续步骤），
每次都调模型既慢又费额度。

- 键是**内容寻址**的：材料内容一变，键就变 → **天然不命中**，不需要额外的失效逻辑；
  候选链（可用档位）也算进键里，**用户开关模型后不会命中旧档位的结果**；
- TTL 10 分钟兜底；**过期或坏数据一律当作没有**并顺手删掉（不留着坑人）；
- 全部是派生数据：删掉整张表只是让下次重算一遍。

### chunk_embeddings（迁移 013，向量索引 · D3/N7）

```sql
CREATE TABLE IF NOT EXISTS chunk_embeddings (
    note_id      TEXT NOT NULL,
    chunk_index  INTEGER NOT NULL,
    model        TEXT NOT NULL,   -- 嵌入模型标识（含 provider 前缀）
    dim          INTEGER NOT NULL,
    vector       BLOB NOT NULL,   -- f32 小端数组
    content_hash TEXT NOT NULL,   -- 分块内容指纹（增量构建据此跳过没变的）
    created_at   TEXT NOT NULL,
    PRIMARY KEY (note_id, chunk_index, model)
);
```

用途：让检索多一路**向量召回**，与 BM25 用 RRF 融合。

- **只存向量与指纹**，不存正文（正文仍以 Markdown 文件为准）；删掉这张表只是重算一遍；
- 主键带 `model`：换嵌入模型要各存一份，**不同模型的向量不能混用**；
- 检索是**本地暴力余弦**（不引入向量数据库依赖）；10 万条以内够用，超过要重新评估（已知边界）；
- 构建**必须显式确认**（会把片段文本发给嵌入端点一次），且是**增量**的。

### notes 的文件快照列（迁移 005）

| 列 | 含义 |
| --- | --- |
| `file_size` | 文件字节数快照 |
| `file_mtime` | 文件修改时间快照（ISO 8601） |

用途：扫描/监听先用「mtime + size」比对，未变化的文件直接短路，
不再读取全文与计算 sha256（1 万篇规模下稳态扫描的主要成本）。

### notes 的子文档列（迁移 007，子文档层级）

```sql
ALTER TABLE notes ADD COLUMN parent_id TEXT;
CREATE INDEX IF NOT EXISTS idx_notes_parent ON notes(parent_id);
```

| 列 | 含义 | 对应 frontmatter |
| --- | --- | --- |
| `parent_id` | 父文档 id（NULL = 顶层文档） | 子文档的 `parent`（可写父文档 id 或标题） |

- **派生数据**：以文件为准。应用内「创建子文档 / 升级为顶层文档」会写回子文档 frontmatter 的
  `parent:`，再由写入路径投影到该列；外部编辑改 `parent:` 后扫描时会重算；
- 解析规则：先按 id 精确匹配，再按标题（忽略大小写、只取 active、按更新时间倒序取第一条）；
  解析不到父文档时 `parent_id` 置空（子文档回到顶层展示，不会从列表中消失）；
- 应用内写入时 `parent:` 存**父文档 id**（稳定，不随父文档改名失效）；
  手写 frontmatter 时也可以写父文档标题，扫描 / 保存时按「id 优先、标题兜底」解析；
- 防环：`set_parent` / 创建子文档时沿父链上溯，命中自身或超过 64 跳报 `INVALID_INPUT`；
- 列表展示为「父文档 → 子文档」目录树，父子关系不改变笔记的文件位置（与分类目录相互独立）。

### notes 的父文档声明原文列（迁移 010，待解析声明）

```sql
ALTER TABLE notes ADD COLUMN declared_parent TEXT;
CREATE INDEX IF NOT EXISTS idx_notes_declared_parent ON notes(workspace_id, declared_parent);
```

| 列 | 含义 | 对应 frontmatter |
| --- | --- | --- |
| `declared_parent` | `parent:` 的**原文**（可能是父文档 id，也可能是标题） | 子文档的 `parent` |

- **为什么需要它**：`parent_id` 只保存解析结果，解析不到（父文档还没出现）时线索就丢了，
  子文档会一直停在顶层，直到它下次被外部编辑。保留声明原文后，扫描每轮都能对
  「声明了父文档但还没解析成功」的子文档重试一次（`retry_unresolved_declarations`），
  无需为了找它们去读全库文件；
- **派生数据**：与 `parent_id` / `sort_order` / `pinned` 一样由文件投影而来，
  删掉数据库重新扫描即可重建；应用内重命名父文档时，子文档的声明原文与文件一起改写；
- 解析成功时 `parent_id` 与 `declared_parent` 同时写入；解析失败时只保留原文、
  `parent_id` 留空（子文档按顶层展示，不会从列表消失）。

### notes 的手动排序列（迁移 008，自定义顺序）

```sql
ALTER TABLE notes ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;
CREATE INDEX IF NOT EXISTS idx_notes_sort_order ON notes(sort_order);
```

| 列 | 含义 | 对应 frontmatter |
| --- | --- | --- |
| `sort_order` | 同层内的显示顺序（1 起，越小越靠前；0 = 未排序） | `order`（也接受 `sort_order` / `sortOrder`） |

- **派生数据**：应用内拖拽排序会写回 frontmatter 的 `order:`（写成数字，如 `order: 3`），
  外部编辑改 `order:` 后扫描时重新投影；
- `NoteSort::Manual` 的排序键是 `sort_order ASC, updated_at DESC, id DESC`：
  0（未排序）与已排序混排时不会抖动，未排序笔记仍按更新时间稳定排列；
- **按层归一化**：重排会把**同一父文档下的全部活跃笔记**一起写入 order
  （传入顺序在前、其余保持原相对顺序），因此一旦拖过，该层的顺序就是确定的；
- 排序不改写 `updated_at`——它属于展示顺序而非内容变更；
- 索引：`idx_notes_sort_order` 支撑「自定义顺序」列表的低成本排序。

### notes 的收藏列（自 V1.2.1 起为派生列）

| 列 | 含义 | 对应 frontmatter |
| --- | --- | --- |
| `favorite` | 收藏标记（1 = 收藏） | `favorite`（也接受 `is_favorite` / `isFavorite`） |

- **派生数据（2026-10-11 起）**：应用内「收藏 / 取消收藏」写回 frontmatter 的 `favorite: true`
  （取消时**移除该字段**，不留 `favorite: false` 噪声），外部编辑器手改后由扫描同步；
- 此前它只存在数据库里：删除 `.knowledge/database.sqlite` 重建（文档推荐的恢复手段）
  会静默丢掉全部收藏——现在与 `pinned` 同口径，删库重建后仍在；
- 索引：`idx_notes_favorite` 支撑侧边栏「收藏」列表。

### notes 的置顶列（迁移 009，置顶文档）

```sql
ALTER TABLE notes ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
```

| 列 | 含义 | 对应 frontmatter |
| --- | --- | --- |
| `pinned` | 置顶标记（1 = 置顶） | `pinned`（也接受 `is_pinned` / `isPinned`，布尔宽容解析） |

- **派生数据**：应用内「置顶 / 取消置顶」写回 frontmatter 的 `pinned: true`，
  外部编辑器手改后由扫描同步（`apply_external_edit` 投影）；
- 所有排序键都前置 `pinned DESC`，因此置顶笔记在**任何排序方式**下都排在最前；
- 置顶只在**同一层**内生效：子文档仍跟在父文档之后（不会被拽到顶层），
  这与语雀「置顶文档」在目录里的表现一致；
- 不建独立索引：`pinned` 基数极低（通常个位数），列表查询已带分页上限，
  单独索引收益小于维护成本。

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

## 4. 版本历史的存储位置（V1.2.1）

版本历史不落数据库，直接放在工作区里：

```
<工作区>/.knowledge/history/<笔记 id>/<YYYYMMDDThhmmssSSS>-<内容哈希前 8 位>.md
```

- 存的是**覆盖写入前的完整 Markdown**（含 frontmatter），因此快照本身可读、可 diff、可手工取回；
- 删掉数据库不影响历史；删掉 `.knowledge/history` 也不影响笔记（历史是附加价值）；
- 每篇最多 50 份、同一篇 60 秒内只留一份；永久删除笔记时对应目录一并清理；
- 与 `notes` 表无外键关系：历史是**旁路数据**，不参与索引与检索。

## 5. 研究会话的存储位置（V1.2.1 阶段 D1）

研究助手（`docs/AI_ARCHITECTURE_PLAN.md` §9）的会话同样落在工作区里，**不进数据库**：

```
<工作区>/.knowledge/research/<sessionId>.json     # sessionId = research_<ULID>
<工作区>/.knowledge/logs/ai-audit.jsonl           # AI 行为审计（不含正文）
```

- 一个会话一份 JSON：研究计划、来源（含分级与理由）、报告、写入提案、步骤日志；
- 是**可丢弃的中间产物**：删掉 `.knowledge/research` 只会丢研究草稿，不影响任何笔记；
- 写入知识库的研究报告走正常 `note_create` 路径（因此仍可用 Markdown 全量重建）；
- 审计只记行为元数据（时间 / 能力 / 是否外发 / 结果 / 耗时 / 备注），上限由策略里的
  `auditMaxEntries` 控制（默认 500）。

## 6. 迁移

- 迁移脚本：`src-tauri/migrations/*.sql`，通过 `include_str!` 编入二进制；
- 记录表：`schema_migrations(version, name, applied_at)`，同时写入 `PRAGMA user_version`；
- 打开工作区时若 `user_version` 高于当前应用支持版本 → 返回 `WORKSPACE_INVALID`（提示升级应用）；
- 索引可重建：`search_rebuild_index` 清空 FTS5 并按 `notes` 重新写入，不依赖外部文件。

当前迁移列表：`001_init`（表结构）、`002_search_index`（FTS5）、`003_note_links`（双向链接），
`004_note_assets`（类型化关系 + 来源时效）、`005_note_file_stamp`（文件快照）、
`006_search_rowid_map`（FTS rowid 映射）、`007_note_parent`（子文档层级）、
`008_notes_sort_order`（手动排序）、`009_notes_pinned`（置顶）、
`010_note_declared_parent`（父文档声明原文，用于重试解析层级）、
`011_note_chunks`（N5 分块索引：标题路径 + 原文字符区间）、
`012_decision_cache`（D2/N6 决策缓存：内容寻址的键 + 过期时间）、
`013_chunk_embeddings`（D3/N7 向量索引：分块向量 + 内容指纹），`SCHEMA_VERSION = 13`。

## 6. 数据恢复能力

| 故障 | 处理 |
| --- | --- |
| 索引损坏 | 手动重建索引（FTS5 表可整体清空） |
| 元数据损坏 | 删除 `.knowledge/database.sqlite` 后重新打开工作区，会重新扫描 Markdown 并采纳 |
| 文件被外部删除 | 记录标记为 `missing`，索引移除，界面提示 |
| 保存中断 | 原子写入：临时文件 + rename，原文件保持完整 |
| 误删笔记 | `.trash/` 保留原相对路径，可恢复或永久删除 |

**删库重建会恢复什么、什么确实会丢（2026-10-11 核对）**

- 会恢复（都在工作区文件里）：笔记与稳定 id、标题、分类目录、标签、父文档层级、
  手动顺序、置顶、**收藏**、来源与时效、双向链接、类型化关系、正文全文索引；
  另外会把 **`.trash/` 里未登记的 Markdown 重新登记成「已删除」笔记**（回收站不再凭空清空）；
- 会丢（只在库里、且没有文件承载）：笔记的 **`missing` 标记**（文件的丢失状态）、
  回收站条目的**原 `trash_path`**（重建后按 `.trash/` 下的实际路径重新推断）、
  以及**工作区偏好**（自动保存间隔、列表排序选择、上次打开的笔记等）、
  备份目录设置与 **AI 策略 / 审计**。这些属于「运行态与偏好」，不是知识与元数据；
  删库前请先做一次 ZIP 备份（备份包含数据库快照，因而这些状态也能一起回来）。
