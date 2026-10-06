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

## 9. 前端服务层

前端不直接拼命令字符串，统一经 `src/services/api.ts` 的 `call()`：

- 归一化错误（字符串 / 对象 / 未知异常 → `AppError`）；
- 浏览器预览模式（`pnpm dev` 且无 Tauri 运行时）下自动切换到内存后端 `src/services/mock.ts`，
  便于在没有桌面外壳时开发与验证界面。
