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
  | 'INTERNAL_ERROR'
  | 'IO_ERROR'
  /** AI 能力总开关关闭 */
  | 'AI_DISABLED'
  /** 联网档位为「仅本地」或本次外发未确认 */
  | 'NETWORK_DISABLED';

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

/** 待复核笔记（`valid_until` 已过期或即将到期，V1.2.1 D4） */
export interface ReviewDueNote {
  noteId: string;
  title: string;
  /** `技术/数据库` 形式的分类路径（Inbox 为空串） */
  categoryPath: string;
  validUntil: string;
  /** >0 已过期天数；<=0 表示还有 `-overdueDays` 天到期 */
  overdueDays: number;
  sourceType?: string | null;
  sourceReliability?: number | null;
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
  /** 需要让用户知道的提醒（例如「源文件不是 UTF-8，已按 GBK 解码」）；不是失败 */
  warnings?: string[];
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

/** AI 联网档位（AI_ARCHITECTURE_PLAN §9.7：默认仅本地） */
export type AiNetworkMode = 'localOnly' | 'allowNetwork';

/** 研究预算上限 */
export interface AiResearchBudget {
  maxSubquestions: number;
  maxPages: number;
  maxDurationSecs: number;
}

/** AI 策略（持久化在工作区设置里，默认最保守） */
export interface AiPolicy {
  enabled: boolean;
  network: AiNetworkMode;
  /** 联网档位已被用户确认过的时间（首次使用再确认） */
  networkConfirmedAt?: string | null;
  allowFetchBody: boolean;
  domainAllowlist: string[];
  domainDenylist: string[];
  /** 联网检索源地址模板（必须含 `{query}`，可选 `{limit}`）；默认不预置 */
  searchEndpoint?: string | null;
  /** 本地模型地址（Ollama HTTP，仅允许本机地址）；为空 = 不启用 */
  localModelEndpoint?: string | null;
  /** 本地模型名（例如 `qwen2.5:7b`）；为空 = 不启用 */
  localModelName?: string | null;
  /** 用本地模型写报告（生成侧；关闭时用模板实现） */
  localModelCompose: boolean;
  /** 用本地模型打分（决策侧近似；未校准，默认关闭，仅用于与规则档 A/B） */
  localModelScore: boolean;
  /** 在线模型基地址（OpenAI 兼容端点，例如 `https://api.example.com/v1`）；为空 = 不启用 */
  remoteModelBaseUrl?: string | null;
  /** 在线模型名（例如 `deepseek-chat`）；为空 = 不启用 */
  remoteModelName?: string | null;
  /** 用在线模型写报告（生成侧；内容会离开本机） */
  remoteModelCompose: boolean;
  /** 用在线模型打分（决策侧近似；内容会离开本机） */
  remoteModelScore: boolean;
  /** 在线模型的首次授权时间；地址或模型名变更后会被清空，需要重新确认 */
  remoteModelAuthorizedAt?: string | null;
  /** 是否已配置 API Key（**只回显「有没有」，Key 本身不返回**） */
  remoteModelKeySet: boolean;
  budget: AiResearchBudget;
  auditMaxEntries: number;
}

export interface UpdateAiPolicyInput {
  enabled?: boolean;
  network?: AiNetworkMode;
  /** 置 true 表示用户确认了联网档位（写入时间戳） */
  confirmNetwork?: boolean;
  allowFetchBody?: boolean;
  domainAllowlist?: string[];
  domainDenylist?: string[];
  /** 检索源地址模板；传空字符串表示清除 */
  searchEndpoint?: string | null;
  localModelEndpoint?: string | null;
  localModelName?: string | null;
  localModelCompose?: boolean;
  localModelScore?: boolean;
  /** 在线模型基地址；传空字符串表示清除 */
  remoteModelBaseUrl?: string | null;
  /** 在线模型名；传空字符串表示清除 */
  remoteModelName?: string | null;
  remoteModelCompose?: boolean;
  remoteModelScore?: boolean;
  /** 置 true 表示用户确认了「内容会离开本机」（写入授权时间戳） */
  confirmRemoteModel?: boolean;
  /** API Key：**只写不读**；传空字符串表示清除 */
  remoteModelApiKey?: string | null;
  budget?: AiResearchBudget;
}

/** 联网检索候选（D3）：尚未抓正文 */
export interface WebSearchHit {
  title: string;
  url: string;
  snippet: string;
  publishedAt?: string | null;
  host: string;
}

/** AI 行为审计（不含正文） */
export interface AiAuditEntry {
  at: string;
  capability: string;
  outbound: boolean;
  outcome: 'ok' | 'denied' | 'error';
  durationMs: number;
  note?: string | null;
}

/** 本地检索命中 */
export interface KbHit {
  noteId: string;
  title: string;
  snippet: string;
}

/** 研究会话状态（与 Rust `ResearchStatus` 一致，只能单向推进） */
export type ResearchStatus =
  | 'planned'
  | 'collected'
  | 'verified'
  | 'composed'
  | 'proposed'
  | 'written'
  | 'aborted';

/** 来源权威性级别 A–E（§9.3） */
export type ResearchSourceLevel = 'A' | 'B' | 'C' | 'D' | 'E';

/** 来源通道：本知识库笔记 / 外部网页（D3 起才有） */
export type ResearchSourceOrigin = 'local' | 'web';

/** 报告里的醒目警告类别 */
export type ResearchWarningKind =
  | 'unverifiedSources'
  | 'singleSource'
  | 'conflict'
  | 'stale'
  | 'externalNotAvailable'
  | 'noLocalEvidence'
  | 'templateGenerated'
  | 'modelGenerated'
  /** 有检索结果被域名策略（黑名单 / 白名单）丢弃，未进入来源列表 */
  | 'droppedSources';

/** 研究请求（`research_start` 输入；只有课题必填） */
export interface ResearchRequest {
  topic: string;
  scope?: string | null;
  timeWindowDays?: number | null;
  /** 用户是否明确表达「结果写入知识库」 */
  writeBack?: boolean;
  targetCategoryId?: string | null;
}

export interface ResearchSubQuestion {
  id: string;
  question: string;
  keywords: string[];
  localHits: number;
}

export interface ResearchKbBaseline {
  noteId: string;
  title: string;
  snippet: string;
}

export interface ResearchPlan {
  topic: string;
  scope?: string | null;
  subQuestions: ResearchSubQuestion[];
  localBaseline: ResearchKbBaseline[];
  timeWindowDays: number;
  budget: AiResearchBudget;
  /** 计划说明（含降级路径与局限） */
  notes: string[];
}

export interface ResearchSource {
  /** 会话内引用编号（报告里写作 [n]） */
  reference: number;
  title: string;
  url?: string | null;
  origin: ResearchSourceOrigin;
  noteId?: string | null;
  subQuestionId: string;
  level: ResearchSourceLevel;
  reliability: number;
  /** 分级依据（可读、可复核） */
  gradeReason: string;
  publishedAt?: string | null;
  fetchedAt: string;
  applicableVersion?: string | null;
  snippet: string;
  stale: boolean;
  /** 决策层给出的来源可信度 0–1（规则档或 Jev；旧会话可能缺省） */
  credibility?: number;
}

export interface ResearchClaim {
  subQuestionId: string;
  text: string;
  citations: number[];
  bestLevel: ResearchSourceLevel;
  /** ≥2 个独立来源才算交叉验证 */
  crossVerified: boolean;
  fresh: boolean;
  relatedNotes: string[];
  /** 决策层给出的结论置信度 0–1（旧会话可能缺省） */
  confidence?: number;
}

export interface ResearchWarning {
  kind: ResearchWarningKind;
  message: string;
  references: number[];
}

export interface ResearchKbRelation {
  noteId: string;
  title: string;
  relation: string;
}

export interface ResearchReport {
  generatedAt: string;
  /** 「资料截至 YYYY-MM-DD」 */
  dataCutoff: string;
  composer: string;
  /** 打分用的决策模型标识（规则档 `rule`；Jev 接入后可能变化） */
  decisionProvider?: string;
  conclusions: ResearchClaim[];
  keyFacts: ResearchClaim[];
  conflicts: ResearchClaim[];
  kbRelations: ResearchKbRelation[];
  pending: ResearchClaim[];
  references: ResearchSource[];
  /** 模型推测段（默认折叠，写入时默认不含） */
  speculation: string[];
  warnings: ResearchWarning[];
  sourceReliability: number;
  validUntil?: string | null;
  markdown: string;
}

export interface ResearchDuplicateNote {
  noteId: string;
  title: string;
  updatedAt: string;
}

export interface ResearchProposal {
  title: string;
  categoryId?: string | null;
  tags: string[];
  sourceType: string;
  sourceRef: string;
  sourceReliability: number;
  validUntil?: string | null;
  body: string;
  diffPreview: string;
  duplicates: ResearchDuplicateNote[];
  suggestSourceNote: boolean;
}

export interface ResearchStepLog {
  step: string;
  at: string;
  detail: string;
}

export interface ResearchSession {
  id: string;
  request: ResearchRequest;
  status: ResearchStatus;
  createdAt: string;
  updatedAt: string;
  plan?: ResearchPlan | null;
  sources: ResearchSource[];
  report?: ResearchReport | null;
  proposal?: ResearchProposal | null;
  steps: ResearchStepLog[];
  elapsedMs: number;
  abortReason?: string | null;
  writtenNoteId?: string | null;
  /** 被域名策略丢弃的检索结果（黑名单强制 / 白名单之外）：报告附录里会点名 */
  droppedSources?: ResearchDroppedSource[];
}

/** 被域名策略丢弃的一条检索结果（`reason`：`denylist` | `notAllowed`） */
export interface ResearchDroppedSource {
  url: string;
  host: string;
  reason: string;
}

/** 研究进度事件负载（事件 `ai://research-progress`；**只含元数据**，不含正文） */
export interface ResearchProgress {
  sessionId: string;
  step: string;
  detail: string;
  pagesFetched: number;
  sources: number;
}

export interface ResearchSessionSummary {
  id: string;
  topic: string;
  status: ResearchStatus;
  createdAt: string;
  updatedAt: string;
  sourceCount: number;
  writtenNoteId?: string | null;
}

export type ResearchWriteMode = 'create' | 'append' | 'abandon';

export interface ResearchWriteInput {
  sessionId: string;
  mode: ResearchWriteMode;
  targetNoteId?: string | null;
  /** 是否把「模型推测」段一并写入（默认 false） */
  includeSpeculation?: boolean;
  createSourceNote?: boolean;
  title?: string | null;
  categoryId?: string | null;
}

export interface ResearchWriteResult {
  sessionId: string;
  outcome: 'created' | 'appended' | 'abandoned';
  noteId?: string | null;
  noteTitle?: string | null;
  sourceNoteId?: string | null;
  sourceReliability: number;
  validUntil?: string | null;
  diffPreview: string;
}

/** 网页剪藏结果（V1.2.1 C1）：抓取 + 抽取后的网页正文 */
export interface ClippedPage {
  url: string;
  finalUrl: string;
  title: string;
  site?: string | null;
  markdown: string;
  wordCount: number;
  fetchedAt: string;
  bytes: number;
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
