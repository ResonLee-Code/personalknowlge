/**
 * 与 Rust 后端 DTO 一一对应的类型定义。
 * 后端统一使用 camelCase 序列化，字段命名保持与设计文档 14.8 一致。
 */

export type ErrorCode =
  | 'INVALID_INPUT'
  | 'NOT_INITIALIZED'
  | 'WORKSPACE_NOT_FOUND'
  | 'WORKSPACE_ALREADY_EXISTS'
  | 'WORKSPACE_INVALID'
  | 'NOTE_NOT_FOUND'
  | 'NOTE_SAVE_FAILED'
  | 'NOTE_CONFLICT'
  | 'CATEGORY_NOT_FOUND'
  | 'CATEGORY_NAME_CONFLICT'
  | 'CATEGORY_MOVE_INVALID'
  | 'TAG_NOT_FOUND'
  | 'FILE_ACCESS_DENIED'
  | 'PATH_INVALID'
  | 'DATABASE_ERROR'
  | 'SEARCH_INDEX_ERROR'
  | 'BACKUP_FAILED'
  | 'EXPORT_FAILED'
  | 'IO_ERROR'
  | 'INTERNAL_ERROR';

export interface AppError {
  code: ErrorCode;
  message: string;
  details?: unknown;
}

export interface Workspace {
  id: string;
  name: string;
  rootPath: string;
  createdAt: string;
  updatedAt: string;
}

export interface WorkspaceStats {
  noteCount: number;
  inboxCount: number;
  trashCount: number;
  categoryCount: number;
  tagCount: number;
  attachmentCount: number;
  /** 全部活跃笔记的正文字数合计 */
  totalWords: number;
  /** 最近 7 天有更新的笔记数 */
  updatedLast7d: number;
  /** 最近 30 天有更新的笔记数 */
  updatedLast30d: number;
  /** 置顶笔记数 */
  pinnedCount: number;
  /** 近 14 天写作活跃度（只含有更新的日子） */
  activity: { date: string; count: number }[];
}

export interface WorkspaceSummary {
  workspace: Workspace;
  stats: WorkspaceStats;
  isActive: boolean;
  isAvailable: boolean;
}

export type NoteStatus = 'active' | 'deleted' | 'missing';

export interface Note {
  id: string;
  workspaceId: string;
  title: string;
  filePath: string;
  categoryId?: string | null;
  status: NoteStatus;
  contentHash?: string | null;
  wordCount: number;
  favorite: boolean;
  createdAt: string;
  updatedAt: string;
  lastIndexedAt?: string | null;
  /** 父文档 id（子文档层级），无则为顶层文档 */
  parentId?: string | null;
  /** 手动排序位次（同层内，1 起；0 表示未显式排序） */
  sortOrder?: number;
  /** 置顶：所有排序方式下都排在最前 */
  pinned?: boolean;
  /** 来源与时效（V1.1.2 N1） */
  sourceType?: string | null;
  sourceRef?: string | null;
  sourceReliability?: number | null;
  validUntil?: string | null;
}

export interface TextSegment {
  text: string;
  matched: boolean;
}

export interface NoteSummary {
  id: string;
  title: string;
  /** 父文档 id（子文档层级），无则为顶层文档 */
  parentId?: string | null;
  /** 手动排序位次（配合「自定义顺序」排序使用） */
  sortOrder?: number;
  /** 置顶（所有排序方式下都在最前） */
  pinned?: boolean;
  filePath: string;
  categoryId?: string | null;
  categoryPath: string;
  status: NoteStatus;
  favorite: boolean;
  wordCount: number;
  createdAt: string;
  updatedAt: string;
  tags: string[];
  snippet?: TextSegment[];
}

export interface NoteDetail {
  note: Note;
  content: string;
  category?: Category | null;
  categoryPath: string;
  tags: Tag[];
}

export interface NoteListQuery {
  categoryId?: string | null;
  tagId?: string | null;
  status?: NoteStatus | null;
  favoriteOnly?: boolean;
  inboxOnly?: boolean;
  /** 只看没有被父文档收纳的顶层文档（对标语雀「不在目录中」） */
  rootOnly?: boolean;
  keyword?: string | null;
  page?: number;
  pageSize?: number;
  sort?: NoteSort;
}

export type NoteSort =
  | 'updatedDesc'
  | 'updatedAsc'
  | 'createdDesc'
  | 'titleAsc'
  /** 自定义顺序（拖拽排序写入的 order） */
  | 'manual';

export interface NoteListResult {
  items: NoteSummary[];
  total: number;
  page: number;
  pageSize: number;
}

export interface SaveNoteResult {
  note: Note;
  contentHash: string;
  savedAt: string;
  indexUpdated: boolean;
  indexWarning?: string | null;
}

export interface Category {
  id: string;
  workspaceId: string;
  parentId?: string | null;
  name: string;
  sortOrder: number;
  dirName: string;
  createdAt: string;
  noteCount: number;
}

export interface CategoryNode {
  category: Category;
  depth: number;
  children: CategoryNode[];
}

export type CategoryDeleteStrategy = 'moveToInbox' | 'moveToParent' | 'unassign';

export interface CategoryDeleteReport {
  deletedCategories: number;
  movedNotes: number;
  unassignedNotes: number;
  warnings: string[];
}

export interface Tag {
  id: string;
  workspaceId: string;
  name: string;
  createdAt: string;
  noteCount: number;
}

export type IndexState = 'ready' | 'stale' | 'rebuilding' | 'failed';

export interface IndexStatus {
  state: IndexState;
  indexedNotes: number;
  totalNotes: number;
  lastRebuiltAt?: string | null;
  lastError?: string | null;
}

export type SearchSort = 'relevance' | 'updatedDesc' | 'createdDesc' | 'titleAsc';

export interface SearchQuery {
  keyword: string;
  categoryId?: string | null;
  tagId?: string | null;
  favoriteOnly?: boolean;
  updatedAfter?: string | null;
  updatedBefore?: string | null;
  sort?: SearchSort;
  page?: number;
  pageSize?: number;
  scopeNoteId?: string | null;
}

export interface SearchHit {
  note: NoteSummary;
  score: number;
  titleMatched: boolean;
  snippet: TextSegment[];
  matchedTerms: string[];
}

export interface SearchResponse {
  keyword: string;
  hits: SearchHit[];
  total: number;
  page: number;
  pageSize: number;
  tookMs: number;
  indexStale: boolean;
}

export interface AppSettings {
  autosaveEnabled: boolean;
  autosaveDelayMs: number;
  editorTheme: string;
  fontSize: number;
  defaultNewLocation: string;
  backupDir?: string | null;
  editorMode: string;
  /** 分屏模式下编辑器占比（0.3 ~ 0.7） */
  editorSplitRatio: number;
  sidebarWidth: number;
  listWidth: number;
  /** 笔记列表排序（含 `manual` = 自定义顺序） */
  listSort: string;
  lastOpenedNoteId?: string | null;
  lastCategoryId?: string | null;
}

export interface UpdateSettingsInput {
  autosaveEnabled?: boolean;
  autosaveDelayMs?: number;
  editorTheme?: string;
  fontSize?: number;
  defaultNewLocation?: string;
  backupDir?: string;
  editorMode?: string;
  editorSplitRatio?: number;
  sidebarWidth?: number;
  listWidth?: number;
  /** 笔记列表排序（含 `manual` = 自定义顺序） */
  listSort?: string;
  lastOpenedNoteId?: string;
  lastCategoryId?: string;
  theme?: string;
}

export interface ScanReport {
  added: number;
  updated: number;
  missing: number;
  unchanged: number;
  durationMs: number;
  warnings: string[];
}

export interface WorkspaceOpenResult {
  workspace: Workspace;
  stats: WorkspaceStats;
  settings: AppSettings;
  indexStatus: IndexStatus;
  warnings: string[];
  initialized: boolean;
  scan: ScanReport;
}

export interface BackupInfo {
  fileName: string;
  path: string;
  createdAt: string;
  sizeBytes: number;
  noteCount: number;
  attachmentCount: number;
  includesDatabase: boolean;
}

export interface BackupCheck {
  name: string;
  passed: boolean;
  detail: string;
}

export interface BackupValidation {
  valid: boolean;
  checks: BackupCheck[];
  noteCount: number;
  attachmentCount: number;
  createdAt?: string | null;
  workspaceName?: string | null;
}

export interface ExportResult {
  targetDir: string;
  noteCount: number;
  attachmentCount: number;
}

export interface RestoreReport {
  targetDir: string;
  extractedFiles: number;
  validation: BackupValidation;
}

export interface AppInfo {
  name: string;
  version: string;
  schemaVersion: number;
  dataDir: string;
  platform: string;
  sqliteVersion: string;
}

export interface FileChange {
  path: string;
  kind: 'created' | 'modified' | 'removed';
}

export interface FileChangeBatch {
  changes: FileChange[];
  directories: string[];
  attachmentsChanged: boolean;
}

export interface CreateNoteInput {
  title: string;
  categoryId?: string | null;
  initialContent?: string | null;
  tags?: string[] | null;
  /** 父文档 id：创建为该文档的子文档（继承其分类） */
  parentId?: string | null;
}

/** 设置父文档（子文档层级），parentId 为 null 表示移到顶层 */
export interface SetParentInput {
  id: string;
  parentId?: string | null;
}

/** 手动排序：ids 是同一层兄弟笔记的新顺序（靠前者在上） */
export interface ReorderNotesInput {
  ids: string[];
}

export interface SaveNoteInput {
  id: string;
  content: string;
  baseContentHash?: string | null;
  title?: string | null;
  categoryId?: string | null;
  tags?: string[] | null;
}

/** 外部文件导入 */
export interface ImportNotesInput {
  /** 绝对路径，可以是文件或目录 */
  paths: string[];
  /** 目标分类；为空时导入到 Inbox */
  categoryId?: string | null;
  /** 目录导入时是否保留源目录结构（重建子目录 + 保留原文件名） */
  preserveStructure?: boolean | null;
  /** 导入格式：auto（默认，自动识别）/ plain / obsidian / notion */
  mode?: string | null;
}

export interface ImportedNote {
  sourcePath: string;
  noteId: string;
  title: string;
  filePath: string;
}

export interface SkippedFile {
  sourcePath: string;
  reason: string;
}

export interface ImportReport {
  imported: ImportedNote[];
  skipped: SkippedFile[];
  total: number;
  failed: number;
  /** 一并导入的附件数量（Obsidian / Notion） */
  attachments: number;
  /** 实际采用的导入模式（auto 时由识别结果决定） */
  mode: string;
}

/** 笔记导出 */
export type ExportFormat = 'markdown' | 'html' | 'docx' | 'pdf';

export interface ExportNoteInput {
  id: string;
  format: ExportFormat;
  /** 目标文件路径；PDF 打印视图可省略 */
  targetPath?: string | null;
}

export interface ExportNoteResult {
  format: ExportFormat;
  path: string;
  fileName: string;
  bytes: number;
  /** PDF：需要前端打开打印视图 */
  printReady: boolean;
  html?: string | null;
}

/** 双向链接（V1.1 M1） */
export type LinkType = 'wikilink' | 'markdown' | 'url';

export interface Backlink {
  noteId: string;
  title: string;
  filePath: string;
  context: string;
  anchor?: string | null;
  updatedAt: string;
}

export interface OutgoingLink {
  targetTitle: string;
  targetNoteId?: string | null;
  targetExists: boolean;
  anchor?: string | null;
  linkType: LinkType;
  context: string;
}

export interface UnresolvedLink {
  sourceNoteId: string;
  sourceTitle: string;
  targetTitle: string;
  context: string;
}

export interface LinkCounts {
  backlinks: number;
  outgoing: number;
  unresolved: number;
}

export interface LinkRebuildReport {
  scannedNotes: number;
  links: number;
  resolved: number;
  unresolved: number;
  durationMs: number;
}

/** 批量操作（V1.1.2） */
export interface BatchFailure {
  noteId: string;
  message: string;
}

export interface BatchReport {
  succeeded: number;
  failures: BatchFailure[];
}

/** 附件管理（V1.1.2） */
export interface AttachmentReference {
  noteId: string;
  title: string;
}

export interface AttachmentEntry {
  relativePath: string;
  fileName: string;
  month: string;
  bytes: number;
  modifiedAt: string;
  references: AttachmentReference[];
}

export interface AttachmentListReport {
  items: AttachmentEntry[];
  totalBytes: number;
  referencedCount: number;
}

/** 类型化关系（N2） */
export interface OutgoingRelation {
  id: string;
  relationType: string;
  targetNoteId: string;
  targetTitle: string;
  note?: string | null;
}

export interface IncomingRelation {
  id: string;
  relationType: string;
  sourceNoteId: string;
  sourceTitle: string;
  note?: string | null;
}

export interface RelationView {
  outgoing: OutgoingRelation[];
  incoming: IncomingRelation[];
}

export interface AddRelationInput {
  sourceId: string;
  targetId: string;
  relationType: string;
  note?: string | null;
}

/** 更新来源与时效（V1.1.2 N1） */
export interface SetSourceInput {
  id: string;
  sourceType?: string | null;
  sourceRef?: string | null;
  sourceReliability?: number | null;
  validUntil?: string | null;
}

/** 附件落盘（V1.1 M2） */
export interface SaveAttachmentInput {
  noteId?: string | null;
  fileName?: string | null;
  mimeType?: string | null;
  /** 原始 Base64 或完整的 data URL */
  dataBase64: string;
}

export interface SavedAttachment {
  relativePath: string;
  linkPath: string;
  absolutePath: string;
  fileName: string;
  bytes: number;
  isImage: boolean;
}

export interface ReadAttachmentInput {
  noteId?: string | null;
  /** 正文里的原始链接目标 */
  source: string;
}

export interface AttachmentData {
  dataUrl: string;
  mimeType: string;
  bytes: number;
}

/** 本地模板（V1.1 M2-2） */
export interface TemplateSummary {
  name: string;
  fileName: string;
  title: string;
  bytes: number;
}

export interface RenderTemplateInput {
  name: string;
  title?: string | null;
}

/** 版本历史快照（对标语雀「查看历史版本」） */
export interface HistoryEntry {
  entryId: string;
  /** 快照时间（ISO 8601） */
  savedAt: string;
  bytes: number;
  /** 正文首个 `#` 标题 */
  title: string;
  /** 正文前 120 字预览 */
  preview: string;
  /** 用户命名的版本名（对标语雀「保存为版本」）；未命名为 null */
  label?: string | null;
}

export interface HistoryContent {
  entryId: string;
  /** 完整 Markdown（含 frontmatter） */
  content: string;
  /** 仅正文（还原时使用） */
  body: string;
}

/** 把当前笔记正文另存为模板（对标语雀「另存为模板」） */
export interface SaveTemplateInput {
  /** 模板名（不含扩展名） */
  name?: string | null;
  /** 模板正文（Markdown，不含 frontmatter） */
  content: string;
  /** 覆盖同名模板 */
  overwrite?: boolean;
}

export interface RenderedTemplate {
  name: string;
  content: string;
}

/** 知识图谱（V1.1 M3） */
export interface GraphQuery {
  categoryId?: string | null;
  tagId?: string | null;
  keyword?: string | null;
  focusNoteId?: string | null;
  hops?: number | null;
  includeOrphans?: boolean | null;
  updatedAfter?: string | null;
  maxNodes?: number | null;
  maxEdges?: number | null;
}

export interface GraphNode {
  id: string;
  title: string;
  categoryId?: string | null;
  updatedAt: string;
  /** 本次结果里的邻居数（同一对笔记多条链接算一个邻居） */
  degree: number;
  tags: string[];
  orphan: boolean;
}

export interface GraphEdge {
  source: string;
  target: string;
  linkType: LinkType;
  weight: number;
}

export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
  truncated: boolean;
  totalNodes: number;
  totalEdges: number;
  durationMs: number;
}
