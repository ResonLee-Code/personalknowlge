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
  /** 命中在原文里的精确区间（N5）；`undefined` = 没能定位（例如只命中标签/分类） */
  location?: SearchMatch;
}

/**
 * 命中在原文里的精确区间（N5）。
 *
 * **区间按码点（Unicode 标量值）计**，与后端 `chunk_markdown` 同一口径——
 * 换算到 CodeMirror / JS 的 UTF-16 下标请用 `codepointRangeToUtf16`。
 */
export interface SearchMatch {
  /** 命中所在分块的标题路径（空串 = 标题之前） */
  heading: string;
  /** 命中所在分块的下标（检索通道化用它做去重身份） */
  chunkIndex: number;
  charStart: number;
  charEnd: number;
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
  /** 大纲显示位置：`list`（默认，与笔记列表共用一栏）| `side`（正文右侧独立一列） */
  outlinePlacement: string;
  /** 选中笔记后是否自动把列表栏切到大纲（默认 false：浏览列表时不该被切走） */
  autoOutlineOnSelect: boolean;
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
  /** `list` | `side` */
  outlinePlacement?: string;
  autoOutlineOnSelect?: boolean;
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
  /** 逐次外发确认（Track C2，可选闸门）：开启后每次把内容交给在线模型都要单独确认 */
  confirmEachOutbound: boolean;
  /** 向量检索（D3/N7，默认关闭）：开启后检索多一路「向量召回」 */
  vectorEnabled: boolean;
  /** 嵌入模型名（例如 `text-embedding-3-small`）；为空 = 向量档不可用 */
  remoteEmbeddingModel?: string | null;
  /** 向量检索授权时间：查询词会发给嵌入端点，需要确认一次 */
  vectorAuthorizedAt?: string | null;
  /**
   * 决策模型档（D4）：`rule`（默认，离线规则）/ `system_one`
   * （TypeSafe Jev 或 Perplexity Decisions —— 两者协议相同，只有端点与模型名不同）
   */
  decisionProvider?: 'rule' | 'system_one' | null;
  /** 决策模型端点基地址（为空 = 按 provider 预设） */
  decisionBaseUrl?: string | null;
  /** 决策模型名（为空 = 按 provider 预设） */
  decisionModel?: string | null;
  /** 决策档授权时间：打分会把材料片段发给该端点，需确认一次 */
  decisionAuthorizedAt?: string | null;
  /** 是否已配置决策模型 Key（**只回显布尔**） */
  decisionKeySet: boolean;
  /** 出站代理（默认跟随系统代理）；给"只有浏览器能上外网"的环境兜底 */
  outboundProxy?: string | null;
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
  /** 逐次外发确认开关（Track C2，默认关闭） */
  confirmEachOutbound?: boolean;
  /** 向量检索开关（D3，默认关闭） */
  vectorEnabled?: boolean;
  /** 嵌入模型名；传空字符串表示清除（换模型会作废向量授权） */
  remoteEmbeddingModel?: string | null;
  /** 置 true 表示用户确认了「查询词与片段会发给嵌入端点」 */
  confirmVector?: boolean;
  /** 决策模型档：`rule` / `system_one` */
  decisionProvider?: 'rule' | 'system_one';
  /** 决策模型基地址；传空字符串表示回到预设 */
  decisionBaseUrl?: string | null;
  /** 决策模型名；传空字符串表示清除 */
  decisionModel?: string | null;
  /** 置 true 表示用户确认了「材料片段会发给决策端点」 */
  confirmDecision?: boolean;
  /** 决策模型 API Key：**只写不读**；传空字符串表示清除 */
  decisionApiKey?: string | null;
  /** 出站代理；传空字符串表示跟随系统 */
  outboundProxy?: string | null;
  budget?: AiResearchBudget;
}

/** 决策模型契约探针的一条题目（D4） */
export interface DecisionProbeItem {
  name: string;
  question: string;
  /** 标准答案；`null` = 只观察（自洽性/矛盾判别） */
  expectedYes?: boolean | null;
  pYes: number;
}

/** 校准分箱（D4）：说多少 vs 实际对多少 */
export interface CalibrationBin {
  range: string;
  count: number;
  meanStated: number;
  actualShare: number;
}

/** 校准评估（D4） */
export interface Calibration {
  samples: number;
  /** 期望校准误差；公开研究用的线是 0.05 */
  ece: number;
  brier: number;
  bins: CalibrationBin[];
}

/** 决策模型契约探针报告（D4）：用我们自己的中文题跑出来的实测指标 */
export interface DecisionProbeReport {
  provider: string;
  model: string;
  host: string;
  items: DecisionProbeItem[];
  /** 有标准答案的题目上的准确率 */
  accuracy?: number | null;
  scoredItems: number;
  /** 期望校准误差（ECE）+ Brier + 分箱表 */
  calibration: Calibration;
  /** 只算「材料里能推出来」那一族 */
  calibrationDeterminable: Calibration;
  /** 只算「材料里没有答案」那一族：校准的模型应当给 ~0.5 */
  calibrationUnderdetermined: Calibration;
  /** 「材料里没有答案」那族的平均陈述概率（理想 ≈0.5） */
  meanStatedUnderdetermined?: number | null;
  /** 折外校准误差（一半拟合、一半评估）——"拟合到底有没有用"的诚实答案 */
  outOfSampleEce?: number | null;
  /** 自洽性：P(A) + P(¬A)，理想 ≈1 */
  coherence?: number | null;
  /** 同一请求跑两次是否逐位相同 */
  deterministic: boolean;
  conflictPairYes?: number | null;
  nonConflictPairYes?: number | null;
  calls: number;
  inputTokens: number;
  latencyMs: number;
  /** 按公开单价估算的美元成本（估算值） */
  estimatedUsd: number;
  note: string;
  /** A/B 对比（D4 收尾）：同一批打分任务上，规则档 vs 决策档 */
  ab: AbReport;
}

/** A/B：一个档位在同一批打分任务上的表现 */
export interface AbOutcome {
  provider: string;
  /** 判定为「该用」的准确率（阈值 0.5） */
  accuracy: number;
  meanRelevant: number;
  meanIrrelevant: number;
  /** 区分度：相关均分 − 不相关均分（越大越能分开；为负说明分反了） */
  separation: number;
  correct: number;
  total: number;
}

/** A/B 报告 */
export interface AbReport {
  cases: number;
  rule: AbOutcome;
  decision: AbOutcome;
  decisionBetter: boolean;
  note: string;
}

/** 向量索引状态（D3/N7） */
export interface VectorIndexStatus {
  available: boolean;
  builtChunks: number;
  totalChunks: number;
  model?: string | null;
  builtAt?: string | null;
  note: string;
}

/** 构建向量索引之前的预览（要发多少、发给谁） */
export interface VectorIndexPreview {
  willSend: boolean;
  host?: string | null;
  notes: number;
  chunksTotal: number;
  /** 本次真正要外发的片段数（增量） */
  chunksToEmbed: number;
  reason: string;
}

/** 一次向量索引构建的结果 */
export interface VectorIndexReport {
  model: string;
  totalChunks: number;
  embedded: number;
  skipped: number;
  tookMs: number;
}

/** 本地模型的「低配机型预设」（Track C3）：值来自后端常量，前端只展示与填充 */
export interface LocalModelPreset {
  /** Ollama 模型名（可直接 `ollama pull`） */
  name: string;
  label: string;
  /** 近似下载体积（字节）；仅用于粗估，不作为精确值 */
  approxBytes: number;
  /** 展示用的体积文案（例如 `≈1.9 GB`） */
  approxSize: string;
  /** 内存提示（定性） */
  memoryHint: string;
  note: string;
}

/** 可用档位（Track C4） */
export type AiTier = 'online' | 'local' | 'template';

/** 模型自检的逐档核对 */
export interface AiDiagnosisCheck {
  tier: AiTier;
  label: string;
  available: boolean;
  /** 为什么可用 / 不可用（人话） */
  detail: string;
}

/** 模型自检结论：现在「写报告」会用哪一档、为什么 */
export interface AiDiagnosis {
  enabled: boolean;
  tier: AiTier;
  /** 与研究报告 `composer` 同源的 provider 标识 */
  providerId: string;
  reason: string;
  checks: AiDiagnosisCheck[];
  note?: string | null;
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
  | 'droppedSources'
  /** 证据不足（D5/N9）：材料撑不起结论，报告如实说「不够」，不硬答 */
  | 'insufficientEvidence';

/** 证据缺口的一类（D5/N9） */
export type ResearchEvidenceGapKind =
  | 'noMaterial'
  | 'partialCoverage'
  | 'noCrossCheck'
  | 'thinEvidence'
  | 'weakOnly'
  | 'staleOnly';

/** 一处证据缺口：差在哪、影响哪些子问题、涉及哪些来源 */
export interface ResearchEvidenceGap {
  kind: ResearchEvidenceGapKind;
  message: string;
  subQuestionIds?: string[];
  references?: number[];
}

/**
 * 证据充分性评估（D5/N9）：研究的「够不够」。
 * 判定看 `sufficient` 与 `gaps`；`score` 只是展示用的趋势指标。
 */
export interface ResearchEvidenceAssessment {
  sufficient: boolean;
  score: number;
  coveredSubQuestions: number;
  totalSubQuestions: number;
  independentSources: number;
  usableSources: number;
  gaps: ResearchEvidenceGap[];
  verdict: string;
  recommendation: string;
}

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
  /** 本次实际使用的检索查询（D5/N9 查询扩展）：第一条永远是课题原文 */
  queries?: string[];
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
  /** 证据充分性（D5/N9）：材料够不够撑结论；旧会话的报告没有这一段 */
  evidence?: ResearchEvidenceAssessment | null;
  composer: string;
  /** 打分用的决策模型标识（规则档 `rule`；Jev 接入后可能变化） */
  decisionProvider?: string;
  /**
   * 冲突判定用的档位（Track C5）：`rule`（规则档，极性词）或模型档
   * （`local.ollama` / `remote.openai-compatible`；多个用 ` / ` 连接）
   */
  conflictProvider?: string;
  /** 打分的可解释 Trace（Track D2/N6）：缓存是否命中、试过哪些档、为什么失败 */
  decisionTrace?: DecisionTrace | null;
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

/**
 * 「推测段」的生成增量（事件 `ai://research-compose-delta`，Track C1 流式）。
 *
 * `text` 是**当前这一档的累计文本**（不是增量）：界面直接替换即可。
 * 这一档失败时 `text` 为空串 + `state='failed'`——**不留下半截内容**。
 * 它只是预览，最终以命令返回的报告（`report.speculation`）为准。
 */
export interface ResearchComposeDelta {
  sessionId: string;
  providerId: string;
  text: string;
  state: 'streaming' | 'failed' | 'done';
}

/** 逐次外发确认预览里的一条片段（Track C2） */
export interface ResearchOutboundSnippet {
  title: string;
  snippet: string;
}

/**
 * 「这一次会把什么发给谁」的预览（Track C2；命令 `research_outbound_preview`）。
 * 只读：不发请求。桌面端用它渲染确认框。
 */
export interface ResearchOutboundPreview {
  willSend: boolean;
  /** 目标主机（不外发时为 null；不含 Key） */
  host?: string | null;
  snippets: ResearchOutboundSnippet[];
  reason: string;
}

/** 决策试过的一档（Track D2/N6） */
export interface DecisionAttempt {
  /** provider 标识：`rule` / `local.ollama` / `remote.openai-compatible` … */
  provider: string;
  ok: boolean;
  /** 人读原因（不含正文） */
  detail: string;
}

/** 一次决策的可解释 Trace（Track D2/N6）：缓存是否命中、试过谁、为什么失败 */
export interface DecisionTrace {
  /** 决策类型（`RELEVANCE` 等） */
  kind: string;
  /** `hit` = 吃缓存；`miss` = 真的走了一遍候选链 */
  cache: 'hit' | 'miss';
  /** 最终生效的档位（全军覆没时为空串） */
  provider: string;
  candidates: number;
  attempts: DecisionAttempt[];
  tookMs: number;
}

/** 一个检索通道的状态（D3/N7）：这次检索用了哪些通道、谁不可用、为什么 */
export interface RetrievalChannelStatus {
  /** `bm25` / `vector` / `graph` */
  id: string;
  available: boolean;
  note: string;
}

/** 融合后的一条检索命中（D3/N7） */
export interface RetrievalHit {
  noteId: string;
  chunkIndex: number;
  /** 标题路径（`A > B`） */
  heading: string;
  snippet: string;
  charStart: number;
  charEnd: number;
  /** RRF 融合分（越大越靠前） */
  score: number;
  /** 贡献这条命中的通道 */
  providers: string[];
}

/** 一次多通道检索的完整报告（命令 `ai_retrieval_search`） */
export interface RetrievalReport {
  channels: RetrievalChannelStatus[];
  hits: RetrievalHit[];
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
