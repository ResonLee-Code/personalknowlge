# API 契约

对应设计文档第 14 章。前端通过 `invoke(command, args)` 调用，
参数使用 **camelCase**（Tauri 自动映射到 Rust 的 snake_case），返回 DTO 均为 camelCase。

## 1. 统一错误结构

```ts
export interface AppError {
  code: ErrorCode;
  message: string;
  details?: unknown;
}
```

错误码：`INVALID_INPUT`、`NOT_INITIALIZED`、`WORKSPACE_NOT_FOUND`、`WORKSPACE_ALREADY_EXISTS`、
`WORKSPACE_INVALID`、`NOTE_NOT_FOUND`、`NOTE_SAVE_FAILED`、`NOTE_CONFLICT`、`CATEGORY_NOT_FOUND`、
`CATEGORY_NAME_CONFLICT`、`CATEGORY_MOVE_INVALID`、`TAG_NOT_FOUND`、`FILE_ACCESS_DENIED`、
`PATH_INVALID`、`DATABASE_ERROR`、`SEARCH_INDEX_ERROR`、`BACKUP_FAILED`、`EXPORT_FAILED`、
`IO_ERROR`、`INTERNAL_ERROR`。

## 2. 事件（后端 → 前端）

| 事件名 | 负载 | 说明 |
| --- | --- | --- |
| `workspace://files-changed` | `ScanReport` | 外部文件变化（新增/更新/缺失统计） |
| `notes://changed` | `{ kind, noteId }` | 笔记被创建 / 更新 / 移动 / 删除 |
| `search://index-status` | `IndexStatus` 或 `{ warning }` | 索引状态变化、监听降级提示 |

## 3. 工作区（设计文档 14.2）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `workspace_create` | `{ name, root }` | `WorkspaceOpenResult` |
| `workspace_open` | `{ root }` | `WorkspaceOpenResult` |
| `workspace_switch` | `{ id }` | `WorkspaceOpenResult` |
| `workspace_close` | — | `void` |
| `workspace_get_current` | — | `WorkspaceOpenResult \| null` |
| `workspace_list` | — | `WorkspaceSummary[]` |
| `workspace_stats` | — | `WorkspaceStats` |
| `workspace_scan` | — | `ScanReport` |
| `workspace_rebuild_index` | — | `IndexStatus` |
| `workspace_forget` | `{ id }` | `void` |
| `workspace_active_id` | — | `string \| null` |
| `workspace_root_path` | — | `string` |
| `settings_get` | — | `AppSettings` |
| `settings_update` | `{ input: UpdateSettingsInput }` | `AppSettings` |

`WorkspaceOpenResult`：`{ workspace, stats, settings, indexStatus, warnings, initialized, scan }`。
`workspace_create` 在目录已初始化时返回 `WORKSPACE_ALREADY_EXISTS`，不会破坏已有数据；
`workspace_open` 可打开普通目录并就地初始化（`initialized = true`）。

## 4. 笔记（设计文档 14.3）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `note_create` | `{ input: CreateNoteInput }` | `NoteDetail` |
| `note_get` | `{ id }` | `NoteDetail` |
| `note_save` / `note_update` | `{ input: SaveNoteInput }` | `SaveNoteResult` |
| `note_list` | `{ query: NoteListQuery }` | `NoteListResult` |
| `note_recent` | `{ limit? }` | `NoteSummary[]` |
| `note_list_deleted` / `note_list_missing` | — | `NoteSummary[]` |
| `note_delete` | `{ id }` | `void`（移入回收站） |
| `note_restore` | `{ id }` | `NoteDetail` |
| `note_purge` | `{ id }` | `void` |
| `note_rename` | `{ id, title, renameFile? }` | `NoteDetail` |
| `note_move` | `{ id, categoryId }` | `NoteDetail` |
| `note_duplicate` | `{ id }` | `NoteDetail` |
| `note_set_favorite` | `{ id, favorite }` | `NoteDetail` |
| `note_set_tags` | `{ id, tags: string[] }` | `NoteDetail` |
| `note_count` | `{ status? }` | `number` |
| `note_import` | `{ input: ImportNotesInput }` | `ImportReport` |
| `note_export` | `{ input: ExportNoteInput }` | `ExportNoteResult` |

`SaveNoteInput`：`{ id, content, baseContentHash?, title?, categoryId?, tags? }`。
`ImportNotesInput`：`{ paths: string[], categoryId? }`（`paths` 可以是文件或目录，目录会递归展开）；
`ImportReport`：`{ imported: ImportedNote[], skipped: { sourcePath, reason }[], total, failed }`。
导入是**复制**语义，源文件不会被修改；单文件上限 5 MB、单次上限 500 个文件，
Markdown 会保留源 frontmatter 的 `id` / `title` / `tags` / 时间字段。

`ExportNoteInput`：`{ id, format: 'markdown' | 'html' | 'docx' | 'pdf', targetPath? }`。
`ExportNoteResult`：`{ format, path, fileName, bytes, printReady, html? }`。
`markdown` 直接复制原文件（含 frontmatter）；`html` 输出自包含单文件（内联样式，可打印为 PDF）；
`docx` 输出 OOXML（Word / WPS 可编辑，也可另存 PDF）；`pdf` 写出打印视图 HTML 并返回内容
（`printReady = true`），由前端打开系统打印流程选择「存储为 PDF」。

`UpdateSettingsInput` 的数值字段（`sidebarWidth`、`listWidth`、`fontSize`、
`autosaveDelayMs`）接受整数或小数：后端会四舍五入为整数，
便于前端直接发送拖拽产生的浮点像素值；`editorSplitRatio` 接受 0.3–0.7 的小数并夹取。
当磁盘哈希与 `baseContentHash` 不一致时返回 `NOTE_CONFLICT`，
`details` 内含 `{ noteId, filePath, currentHash, expectedHash, currentContent }`，供界面提示用户选择。

## 5. 分类与标签（设计文档 14.4 / 14.5）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `category_list` | — | `Category[]` |
| `category_tree` | — | `CategoryNode[]`（含 `noteCount`、`depth`） |
| `category_create` | `{ input: { name, parentId?, createDirectory? } }` | `Category` |
| `category_update` | `{ id, name }` | `Category`（同步目录与笔记路径） |
| `category_move` | `{ id, parentId }` | `Category` |
| `category_delete` | `{ input: { id, strategy } }` | `CategoryDeleteReport` |
| `tag_list` | — | `Tag[]`（含 `noteCount`） |
| `tag_create` | `{ name }` | `Tag` |
| `tag_rename` | `{ id, name }` | `Tag` |
| `tag_delete` | `{ id }` | `void` |
| `note_set_tag_names` | `{ id, tags }` | `Tag[]` |

`strategy` ∈ `moveToInbox | moveToParent | unassign`；删除分类会连带删除子分类，
但可选择如何处置其中的笔记，且不会删除笔记正文。

## 6. 搜索（设计文档 14.6）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `search_notes` | `{ query: SearchQuery }` | `SearchResponse` |
| `search_quick` | `{ keyword, limit? }` | `NoteSummary[]` |
| `search_rebuild_index` | — | `IndexStatus` |
| `search_get_status` | — | `IndexStatus` |

`SearchQuery`：`{ keyword, categoryId?, tagId?, favoriteOnly?, updatedAfter?, updatedBefore?, sort?, page?, pageSize?, scopeNoteId? }`。
`SearchResponse`：`{ keyword, hits, total, page, pageSize, tookMs, indexStale }`；
每个 `SearchHit` 含 `{ note, score, titleMatched, snippet, matchedTerms }`，
`snippet` 为已切分好的高亮片段（`{ text, matched }[]`），前端无需再计算下标。

## 7. 备份与导出（设计文档 14.7）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `backup_create` | `{ input?: { targetDir?, note? } }` | `BackupInfo` |
| `backup_list` | — | `BackupInfo[]` |
| `backup_validate` | `{ path }` | `BackupValidation` |
| `backup_restore` | `{ input: { archivePath, targetDir } }` | `RestoreReport` |
| `export_markdown` | `{ input: { targetDir, includeAttachments } }` | `ExportResult` |
| `backup_directory` | — | `string` |

`backup_validate` 返回逐项检查结果：压缩包可打开、ZIP 结构有效、清单存在、
数据库存在、Markdown 数量一致、附件数量一致、逐文件 SHA-256 校验通过。
恢复到非空目录会被拒绝，避免覆盖现有数据。

## 8. 应用信息

| 命令 | 返回 |
| --- | --- |
| `app_info` | `{ name, version, schemaVersion, dataDir, platform, sqliteVersion }` |
| `app_log_path` | `string \| null` |
| `app_ready` | `boolean` |

## 9. 双向链接（V1.1 M1）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `links_get_backlinks` | `{ noteId }` | `Backlink[]`（来源笔记 + 上下文 + 锚点 + 更新时间） |
| `links_get_outgoing` | `{ noteId }` | `OutgoingLink[]`（目标标题、是否已解析、类型、上下文） |
| `links_get_unresolved` | — | `UnresolvedLink[]`（全库未解析链接，可一键建笔记） |
| `links_get_counts` | `{ noteId }` | `{ backlinks, outgoing, unresolved }` |
| `links_rebuild` | — | `LinkRebuildReport`（扫描笔记数、链接数、已解析/未解析、耗时） |

解析规则：`[[标题]]`、`[[标题|别名]]`、`[[标题#小节]]` 与相对 Markdown 链接；
忽略围栏代码块与行内代码中的链接；目标解析顺序为
标题（大小写不敏感）→ 工作区相对路径 → 文件名 → 笔记 id；
外链以 `linkType = "url"` 记录但不参与反链与未解析统计。
链接属于**可重建派生数据**：`links_rebuild` 可从 Markdown 全量恢复。
保存、导入、外部修改、重命名、删除与恢复都会自动维护链接。

## 10. 附件（V1.1 M2）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `attachment_save` | `{ input: { noteId?, fileName?, mimeType?, dataBase64 } }` | `SavedAttachment`（工作区相对路径、可写入正文的 `linkPath`、绝对路径、字节数、是否图片） |
| `attachment_read` | `{ input: { noteId?, source } }` | `AttachmentData`（`dataUrl`、`mimeType`、`bytes`） |

- `dataBase64` 接受原始 Base64 或完整 `data:image/png;base64,…`；
  单文件上限 20 MB，扩展名优先取文件名、其次由 MIME 推断、未知类型显示为 `.bin`；
- 落盘位置：`attachments/YYYY-MM/<时间戳>-<清洗后的原名>-<短ID>.<扩展名>`，
  附件不进入 `notes` 表，也不会被笔记扫描当作笔记；
- `attachment_read` 只接受**工作区内的相对路径**（`http/https/data/asset` 与越界路径一律拒绝），
  单张预览上限 8 MB，返回 data URL 供 Markdown 预览内联显示。

## 11. 本地模板（V1.1 M2）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `templates_list` | — | `TemplateSummary[]`（`name`、`fileName`、`title`、`bytes`） |
| `template_render` | `{ input: { name, title? } }` | `RenderedTemplate`（`name`、渲染后的 `content`） |

- 模板目录固定为 `.knowledge/templates`，只识别 `.md` / `.markdown`；单文件上限 1 MB；
- 模板名不允许包含 `/`、`\` 或 `..`（路径穿越防护）；
- 占位符：`{{title}}`、`{{date}}`、`{{time}}`、`{{datetime}}`、`{{weekday}}`
  （另接受 `{{标题}}`、`{{日期}}`、`{{时间}}`），未知占位符原样保留；
- `template_render` 只返回内容，写入仍走 `note_create`，保证索引 / 链接 / 统计路径唯一。

## 12. 来源与时效（V1.1.2 N1）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `note_set_source` | `{ input: { id, sourceType?, sourceRef?, sourceReliability?, validUntil? } }` | `NoteDetail` |

- 字段写入笔记 frontmatter（`source_type` / `source_ref` / `source_reliability` / `valid_until`），
  保证「文件是唯一事实来源」；外部编辑器改 frontmatter 后，扫描会同步回数据库列；
- `source_reliability` 只接受 1–5，越界忽略；空字符串视为清空该字段；
- 来源类型建议取值：`official` / `book` / `paper` / `experiment` / `personal` / `conversation` / `ai` / `unknown`；
- 界面：编辑器元信息栏显示来源与星标，逾期显示「可能已过期」，30 天内显示「即将到期」。

## 13. 知识图谱（V1.1 M3）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `graph_get` | `{ query: { categoryId?, tagId?, keyword?, focusNoteId?, hops?, includeOrphans?, updatedAfter?, maxNodes?, maxEdges? } }` | `GraphData`（`nodes`、`edges`、`truncated`、`totalNodes`、`totalEdges`、`durationMs`） |

- 节点来自 `notes`（沿用列表查询的过滤：分类含子分类、标签、关键词），
  边来自 `note_links` 中**已解析**的链接（不含外链与自链接）；
- `hops` 为聚焦跳数（1–3，缺省 1）；`includeOrphans` 缺省 `true`；
- 容量上限缺省 2000 节点 / 5000 边，超出时按连接度优先保留并令 `truncated = true`；
- `degree` 是**本次结果内**的邻居数（同一对笔记多条链接算一个邻居），截断后重新计算；
- 图谱是派生数据：不落库，`links_rebuild` 可从 Markdown 整体修复。

## 14. 批量操作（V1.1.2）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `notes_move_batch` | `{ input: { ids, categoryId }}` | `BatchReport` |
| `notes_tag_batch` | `{ input: { ids, tags, mode? }}` | `BatchReport` |
| `notes_delete_batch` | `{ input: { ids }}` | `BatchReport` |

- `BatchReport = { succeeded: number, failures: { noteId, message }[] }`；
- `categoryId = null` 表示移回 Inbox；`mode` 取值 `add`（默认，合并）/ `remove` / `replace`；
- ids 会去重；单篇失败不中断整批；删除为移入回收站（可恢复）。

## 15. 附件管理（V1.1.2）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `attachment_list` | — | `{ items: AttachmentEntry[], totalBytes, referencedCount }`；每个条目含 `relativePath / fileName / month / bytes / modifiedAt / references[]` |
| `attachment_delete` | `{ relativePath }` | `u64`（释放的字节数） |
| `attachment_references` | `{ relativePath }` | `AttachmentReference[]`（引用它的笔记） |

- 引用识别：遍历活跃笔记正文，提取 `attachments/…` 形式的路径（含 `../attachments/…` 相对写法）；
- 删除只允许 `attachments/` 目录内的文件，越界路径直接拒绝；
- 附件是普通文件，不进入 `notes` 表；删除后正文里的引用会失效（界面在确认框中列出被引用情况）。

### 导入新增选项（V1.1.2）

- `preserveStructure?: boolean`：为 `true` 时，目录导入会在目标分类下重建子目录并保留原文件名；
- `mode?: 'auto' | 'plain' | 'obsidian' | 'notion'`：导入格式（默认 `auto` 自动识别）；
  Obsidian 模式会把 `![[文件]]` 嵌入改写为标准图片链接，Notion 模式会去掉名称里的
  `<32 位十六进制>` 后缀并解码链接目标；两种模式都会把附件复制到 `attachments/YYYY-MM/`；
- `ImportReport` 新增 `mode`（实际采用的模式）与 `attachments`（附件数量）。

## 16. 类型化关系（N2）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `relations_list` | `{ noteId }` | `{ outgoing: OutgoingRelation[], incoming: IncomingRelation[] }` |
| `relations_add` | `{ input: { sourceId, targetId, relationType, note? } }` | `RelationView` |
| `relations_remove` | `{ id }` | `RelationView`（来源笔记的视图） |
| `relations_rebuild` | — | `usize`（重建的关系统数） |

- 关系类型：`RELATED_TO` / `DERIVED_FROM` / `SUPERSEDES` / `CONTRADICTS` / `CONFIRMS` /
  `DEPENDS_ON` / `PART_OF` / `EXAMPLE_OF` / `REFERENCES`（大小写、空格、短横线都会归一）；
- 声明写在来源笔记 frontmatter 的 `relations:`；`note_relations` 表是**派生数据**，
  `relations_rebuild` 可从正文整表恢复；
- 重命名目标笔记会改写来源 frontmatter 里的 `target`；目标被删除时派生关系解绑
  （声明保留，目标恢复后重建即可复原）。

## 17. 子文档层级（V1.2.1）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `note_set_parent` | `{ input: { id, parentId } }` | `NoteDetail` |

- `parentId` 可以是父文档 **id** 或**标题**；传 `null`（或空串）表示**移到顶层**；
- 找不到父文档时返回 `INVALID_INPUT`；把笔记设为自己的子孙会返回 `INVALID_INPUT`（防环，含 > 64 跳兜底）；
- 父子关系以 **子文档 frontmatter 的 `parent:`** 为唯一事实来源，`notes.parent_id` 是可重建的派生列；
- `CreateNoteInput` 新增可选 `parentId`：创建时即成为该文档的子文档，并且
  **未显式指定 `categoryId` 时会继承父文档的分类**；
- `NoteSummary` 与 `Note` 新增 `parentId`（无则为 `null`），列表据此渲染目录树；
- 父文档被删除 / 重命名后：派生列指向失效时子文档按顶层展示（不会隐藏），
  重命名会同步子文档 frontmatter 里的 `parent:`（写 id 时无影响）。

## 18. 手动排序（V1.2.1）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `notes_reorder` | `{ input: { ids: string[] } }` | `usize`（实际写入 order 的篇数） |

- `ids` 是**同一层**兄弟笔记的新顺序（靠前者在上），位次 1 起写入 frontmatter 的 `order:`
  并投影到 `notes.sort_order`；空数组返回 `INVALID_INPUT`；不存在的 id 会被跳过（不报错）；
- **按层归一化**：若 `ids` 只覆盖同层的一部分，其余同层活跃笔记会保持原相对顺序
  **排在后面**，保证该层在「自定义顺序」下是确定的；
- 排序**不改写 `updated_at`**（展示顺序不是内容变更），因此「最近修改」排序结果不变；
- 列表查询新增 `sort: "manual"`，排序键为 `sort_order ASC, updated_at DESC, id DESC`
  （0 = 未排序的兜底顺序）；
- 列表查询新增 `rootOnly: true`（只看 `parent_id IS NULL` 的顶层文档，对标语雀「不在目录中」）；
- 前端契约：把条目拖到另一条的中间 = 先 `note_set_parent`（成为子文档），
  拖到上下边缘 = `notes_reorder`（同层插入）；拖拽落点判定在
  `src/lib/noteDragDrop.ts`（纯函数，已覆盖单元测试）。

工作区设置新增 `listSort`（`settings_update`）：持久化列表排序选择
（`updatedDesc` / `updatedAsc` / `createdDesc` / `titleAsc` / `manual`），
未知值会被忽略而不是写库。

## 19. 置顶文档（V1.2.1）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `note_set_pinned` | `{ id, pinned: boolean }` | `NoteDetail` |

- 写回 frontmatter 的 `pinned:`（`true` 时写入，取消时移除字段）并投影到 `notes.pinned`；
- **所有排序键前置 `pinned DESC`**：`updatedDesc` / `updatedAsc` / `createdDesc` /
  `titleAsc` / `manual` 下置顶笔记都排在最前；
- 置顶只在同一层内生效（子文档仍跟在父文档之后），且**不改写 `updated_at`**；
- `NoteSummary` 与 `Note` 新增 `pinned`（布尔，缺失视为 false）；
- 扫描同步路径（`apply_external_edit`）会投影 `pinned` 与 `order`，
  因此用户用外部编辑器手改 frontmatter 也能生效。

## 20. 本地模板：另存为模板（V1.2.1）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `template_save` | `{ input: { name?, content, overwrite? } }` | `TemplateSummary` |

- 把笔记正文写入 `.knowledge/templates/<name>.md`（模板是普通 Markdown，用户可直接编辑）；
- 正文没有一级标题时自动补 `# {{title}}`，保证套用时标题占位符可用；
- `overwrite` 默认 `false`：同名模板要求显式覆盖，避免误删用户模板；
- 模板名做校验：去扩展名、禁止路径分隔符与 `..`、过滤 `: * ? " < > |`、限制 60 字符。
- 读取与套用仍走既有的 `templates_list` / `template_render`（第 11 节）。

## 21. 版本历史（V1.2.1 P2）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `note_history_list` | `{ noteId }` | `HistoryEntry[]`（新的在前：entryId / savedAt / bytes / title / preview） |
| `note_history_read` | `{ noteId, entryId }` | `HistoryContent`（content = 完整 Markdown，body = 纯正文） |
| `note_history_restore` | `{ noteId, entryId }` | `NoteDetail` |
| `note_history_remove` | `{ noteId, entryId }` | `usize`（剩余快照数） |
| `note_history_save_version` | `{ noteId, label }` | `HistoryEntry`（把当前内容存成命名版本） |
| `note_history_label` | `{ noteId, entryId, label }` | `HistoryEntry[]`（`label` 为 null / 空串 = 清除命名） |

- 快照存 `.knowledge/history/<noteId>/<stamp>-<hash8>.md`，命名版本另存
  `labels.json`（`entryId → label`，见 DATABASE_SCHEMA 第 4 节）；
- 留档时机：`note_save` 覆盖写入前（正文确实变化时）；还原前会强制留档当前内容；
- `note_history_restore` **只回滚正文**（标题 / 标签 / 分类 / 父文档保持当前值），
  写入路径仍走 `note_save`，因此索引、双链、关系都会照常刷新；
- id 校验：`noteId` / `entryId` 不允许包含路径分隔符或 `..`。

## 22. 网页剪藏（V1.2.1 C1）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `note_clip_url` | `{ url }` | `ClippedPage`（url / finalUrl / title / site / markdown / wordCount / fetchedAt / bytes） |

- **只读**：抓取 + 转换，不写库；调用方（前端）走常规 `note_create` 落库，
  再用 `note_set_source` 写 `source_type: web` 与 `source_ref`；
- 只接受 http/https 公开页面，缺协议自动补 `https://`；非 HTML 的 `Content-Type` 直接报错；
- 单页读取上限 3 MB、整体超时 20 秒、正文 Markdown 上限 12 万字符（超出截断并注明）；
- 抽取失败（如需要登录）返回 `INVALID_INPUT` 并提示原因。

## 23. AI 策略与工具（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §6、§9.5）

| 命令 | 参数 | 返回 |
| --- | --- | --- |
| `ai_policy_get` | — | `AiPolicy`（默认 `enabled=false` / `network=localOnly` / `allowFetchBody=false`） |
| `ai_policy_set` | `{ input: UpdateAiPolicyInput }` | `AiPolicy` |
| `ai_audit_tail` | `{ limit }` | `AiAuditEntry[]`（新的在前，不含正文） |
| `ai_kb_search` | `{ keyword, limit }` | `KbHit[]`（复用 FTS5，不联网） |
| `ai_web_fetch` | `{ url, confirmed }` | `ClippedPage` |
| `ai_web_search` | `{ keyword, limit }` | `KbHit[]`（阶段 D3 接入前返回「未接入」） |

- 闸门顺序：总开关 → 联网档位 → 单次确认 → 抓取正文开关 → 域名黑名单 → 域名白名单；
  任一不满足即返回 `AI_DISABLED` / `NETWORK_DISABLED`，且**不发起任何请求**；
- 切换联网档位会清空既有的确认时间戳（切回「仅本地」后再切到联网需重新确认一次）；
- 审计写在 `.knowledge/logs/ai-audit.jsonl`，仅记录行为元数据（时间 / 能力 / 是否外发 / 结果 / 耗时 / 备注）。

## 24. 研究助手（V1.2.1 阶段 D1 / AI_ARCHITECTURE_PLAN §9）

五步固定流程，每步一个命令，便于界面逐步推进、显示进度、随时中止。
所有步骤都先过 §23 的策略闸门（总开关默认关闭 → `AI_DISABLED`）。

| 命令 | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `research_start` | `{ input: ResearchRequest }` | `ResearchSession` | 立项：课题 → 3–5 个子问题 + 本地「已知基线」；课题过于宽泛返回 `INVALID_INPUT` 并要求补充范围 |
| `research_get` | `{ sessionId }` | `ResearchSession` | 读取会话（刷新页面后恢复现场） |
| `research_list` | `{ limit }` | `ResearchSessionSummary[]` | 最近的会话（新的在前） |
| `research_collect` | `{ sessionId, confirmed }` | `ResearchSession` | 收集材料：本地 FTS5；**外部检索未接入（D3），`confirmed` 不影响本地路径** |
| `research_verify` | `{ sessionId }` | `ResearchSession` | 按子问题汇总，≥2 个独立来源才算交叉验证 |
| `research_compose` | `{ sessionId }` | `ResearchSession` | 渲染固定六段报告（顶部「资料截至 YYYY-MM-DD」） |
| `research_proposal` | `{ sessionId }` | `ResearchSession` | 写入提案（默认 Inbox、写前 diff、同标题提示）；**不落库** |
| `research_abort` | `{ sessionId, reason? }` | `ResearchSession` | 中止；已得材料保留为草稿 |

- 步骤状态机：`planned → collected → verified → composed → proposed`；顺序不可跳过
  （例如未收集就验证返回 `INVALID_INPUT`），`aborted` 后不可再推进；
  `research_proposal` 允许在 `composed` / `proposed` 重复调用（同标题冲突可能后出现）；
- 会话持久化在 `.knowledge/research/<sessionId>.json`；**不写入正文**以外的任何外部系统；
- 报告结构（顺序固定）：① 结论先行（只放交叉验证过的）② 关键事实 ③ 争议与不确定性
  ④ 与本知识库的关系（`[[标题]]`）⑤ 待验证清单 ⑥ 参考来源；推测段由模板 Provider 生成，
  默认不写入笔记，需要时在写入确认里勾选；
- 审计：每一步都写 `research_*` 记录（不含正文）；**默认口径下 `outbound` 恒为 `false`**。

## 25. 前端服务层

前端不直接拼命令字符串，统一经 `src/services/api.ts` 的 `call()`：

- 归一化错误（字符串 / 对象 / 未知异常 → `AppError`）；
- 浏览器预览模式（`pnpm dev` 且无 Tauri 运行时）下自动切换到内存后端 `src/services/mock.ts`，
  便于在没有桌面外壳时开发与验证界面。

按领域拆分的服务模块：`workspaceService`、`noteService`、`categoryService`、`tagService`、
`searchService`、`backupService`、`linkService`（第 9 节）、`attachmentService`（第 10 节）、
`templateService`（第 11 节）、`graphService`（第 12 节）。
浏览器预览后端对 `links_*` 提供与 Rust 端一致的解析实现，
因此反链面板、引用计数与未解析链接在纯前端预览下同样可用。
