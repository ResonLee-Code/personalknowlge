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
}

export interface TextSegment {
  text: string;
  matched: boolean;
}

export interface NoteSummary {
  id: string;
  title: string;
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
  keyword?: string | null;
  page?: number;
  pageSize?: number;
  sort?: NoteSort;
}

export type NoteSort = 'updatedDesc' | 'updatedAsc' | 'createdDesc' | 'titleAsc';

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
