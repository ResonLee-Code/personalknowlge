/**
 * 浏览器开发模式下的内存后端。
 *
 * 仅在未运行于 Tauri 外壳时启用（`pnpm dev` 直接打开页面即可预览界面）。
 * 数据保存在 localStorage，便于反复调试界面。
 * 该实现对真正的工作区没有副作用。
 */

import { parseWikilinks, type WikiLinkToken } from '@/lib/wikilink';
import type {
  AiAuditEntry,
  AiPolicy,
  AppError,
  AppSettings,
  Backlink,
  BackupInfo,
  Category,
  CategoryNode,
  ClippedPage,
  CreateNoteInput,
  GraphData,
  GraphNode,
  GraphQuery,
  IndexStatus,
  LinkCounts,
  LinkRebuildReport,
  Note,
  NoteDetail,
  NoteListQuery,
  NoteSummary,
  OutgoingLink,
  ResearchClaim,
  ResearchDuplicateNote,
  ResearchKbRelation,
  ResearchProposal,
  ResearchReport,
  ResearchRequest,
  ResearchSession,
  ResearchSessionSummary,
  ResearchSource,
  ResearchSourceLevel,
  ResearchStatus,
  ResearchSubQuestion,
  ResearchWarning,
  ResearchWriteInput,
  ResearchWriteMode,
  ResearchWriteResult,
  SaveNoteInput,
  ReorderNotesInput,
  SearchQuery,
  SetParentInput,
  SetSourceInput,
  TextSegment,
  UnresolvedLink,
  UpdateAiPolicyInput,
  Workspace,
  WorkspaceOpenResult,
} from '@/types';

const STORAGE_KEY = 'pkos.mock.state.v1';

interface MockState {
  workspace: Workspace | null;
  notes: Note[];
  contents: Record<string, string>;
  categories: Category[];
  tags: { id: string; name: string; createdAt: string }[];
  noteTags: Record<string, string[]>;
  settings: AppSettings;
  backups: BackupInfo[];
  /** 版本历史（内存实现，供浏览器预览与 E2E 验证「历史版本」交互） */
  history: Record<string, MockHistoryEntry[]>;
  /** AI 策略与审计（默认最保守，与 Rust 端一致） */
  ai: AiPolicy;
  aiAudit: AiAuditEntry[];
  /** 研究会话（D1/D2）：key = sessionId */
  research: Record<string, ResearchSession>;
}

interface MockHistoryEntry {
  entryId: string;
  savedAt: string;
  content: string;
  /** 命名版本（对标语雀「保存为版本」） */
  label?: string;
}

type Listener = (payload: unknown) => void;

const listeners = new Map<string, Set<Listener>>();

/** 是否使用内存实现。 */
export function isMockRuntime(): boolean {
  const runtime = globalThis as { __TAURI_INTERNALS__?: unknown };
  return !runtime.__TAURI_INTERNALS__;
}

export function subscribeMockEvent(event: string, handler: Listener): () => void {
  const set = listeners.get(event) ?? new Set<Listener>();
  set.add(handler);
  listeners.set(event, set);
  return () => set.delete(handler);
}

function emit(event: string, payload: unknown) {
  listeners.get(event)?.forEach((handler) => handler(payload));
}

const emptySettings: AppSettings = {
  autosaveEnabled: true,
  autosaveDelayMs: 1000,
  editorTheme: 'light',
  fontSize: 15,
  defaultNewLocation: 'inbox',
  backupDir: null,
  editorMode: 'split',
  editorSplitRatio: 0.5,
  sidebarWidth: 240,
  listWidth: 320,
  listSort: 'updatedDesc',
  lastOpenedNoteId: null,
  lastCategoryId: null,
};

const defaultAiPolicy: AiPolicy = {
  enabled: false,
  network: 'localOnly',
  networkConfirmedAt: null,
  allowFetchBody: false,
  domainAllowlist: [],
  domainDenylist: [],
  budget: { maxSubquestions: 5, maxPages: 12, maxDurationSecs: 480 },
  auditMaxEntries: 500,
};

let state: MockState = load();

function load(): MockState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as MockState;
      return {
        ...parsed,
        settings: { ...emptySettings, ...parsed.settings },
        research: parsed.research ?? {},
      };
    }
  } catch {
    // 忽略损坏的本地数据
  }
  return {
    workspace: null,
    notes: [],
    contents: {},
    categories: [],
    tags: [],
    noteTags: {},
    settings: { ...emptySettings },
    backups: [],
    history: {},
    ai: { ...defaultAiPolicy },
    aiAudit: [],
    research: {},
  };
}

function persist() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // localStorage 不可用时忽略
  }
}

function fail(code: AppError['code'], message: string, details?: unknown): never {
  throw { code, message, details } satisfies AppError;
}

function now(): string {
  return new Date().toISOString();
}

/**
 * 解析父文档：优先按 id 匹配，其次按标题（忽略大小写）。
 * 与 Rust 端 `NoteService::resolve_parent` 的规则保持一致；
 * 找不到时返回 undefined（父文档被删除后子文档回到顶层展示）。
 */
function findParentNote(parentId?: string | null): Note | undefined {
  const raw = parentId?.trim();
  if (!raw) return undefined;
  return (
    state.notes.find((item) => item.id === raw && item.status !== 'deleted') ??
    state.notes.find(
      (item) => item.status !== 'deleted' && item.title.toLowerCase() === raw.toLowerCase(),
    )
  );
}

/** 防环：沿父链上溯，命中 `id` 说明会形成环。 */
function wouldFormCycle(id: string, parentId: string): boolean {
  let current: string | null = parentId;
  for (let guard = 0; current && guard < 64; guard += 1) {
    if (current === id) return true;
    current = state.notes.find((item) => item.id === current)?.parentId ?? null;
  }
  return false;
}

function ulid(): string {
  return Math.random().toString(36).slice(2, 12).toUpperCase();
}

function requireWorkspace(): Workspace {
  if (!state.workspace) fail('NOT_INITIALIZED', '请先创建或打开一个知识库。');
  return state.workspace;
}

function categoryPath(categoryId?: string | null): string {
  if (!categoryId) return '';
  const parts: string[] = [];
  let current = state.categories.find((item) => item.id === categoryId);
  let guard = 0;
  while (current && guard < 20) {
    parts.unshift(current.name);
    current = current.parentId
      ? state.categories.find((item) => item.id === current?.parentId)
      : undefined;
    guard += 1;
  }
  return parts.join('/');
}

function toSummary(note: Note): NoteSummary {
  return {
    id: note.id,
    title: note.title,
    parentId: note.parentId ?? null,
    sortOrder: note.sortOrder ?? 0,
    pinned: note.pinned ?? false,
    filePath: note.filePath,
    categoryId: note.categoryId ?? null,
    categoryPath: categoryPath(note.categoryId),
    status: note.status,
    favorite: note.favorite,
    wordCount: note.wordCount,
    createdAt: note.createdAt,
    updatedAt: note.updatedAt,
    tags: state.noteTags[note.id] ?? [],
  };
}

function toDetail(note: Note): NoteDetail {
  return {
    note,
    content: state.contents[note.id] ?? '',
    category: state.categories.find((item) => item.id === note.categoryId) ?? null,
    categoryPath: categoryPath(note.categoryId),
    tags: (state.noteTags[note.id] ?? []).map((name) => ({
      id: `tag_${name}`,
      workspaceId: state.workspace?.id ?? 'mock',
      name,
      createdAt: note.createdAt,
      noteCount: 1,
    })),
  };
}

/** 域名规范化（与 Rust 端 `normalize_domains` 一致）。 */
function normalizeDomains(list: string[]): string[] {
  const seen: string[] = [];
  list.forEach((item) => {
    const host = item
      .trim()
      .replace(/^https?:\/\//i, '')
      .split(/[/?#]/)[0]!
      .split('@')
      .pop()!
      .split(':')[0]!
      .replace(/^www\./i, '')
      .toLowerCase();
    if (host && !seen.includes(host)) seen.push(host);
  });
  return seen;
}

function domainMatches(rule: string, host: string): boolean {
  const normalized = rule.replace(/^www\./i, '').toLowerCase();
  return normalized.length > 0 && (host === normalized || host.endsWith(`.${normalized}`));
}

function hostOf(url: string): string {
  return url
    .trim()
    .replace(/^https?:\/\//i, '')
    .split(/[/?#]/)[0]!
    .split('@')
    .pop()!
    .split(':')[0]!
    .replace(/^www\./i, '')
    .toLowerCase();
}

/** AI 联网闸门（与后端 policy 的判定顺序一致）；返回非空表示应返回该错误。 */
function checkAiNetworkGate(url: string, confirmed: boolean): ClippedPage | null {
  const audit = (outcome: AiAuditEntry['outcome'], note: string) => {
    state.aiAudit = pushAudit(state.aiAudit, 'web_fetch', false, outcome, note);
  };
  if (!state.ai.enabled) {
    audit('denied', 'AI_DISABLED');
    fail('AI_DISABLED', 'AI 能力未开启：请到「设置 → AI」打开总开关（默认关闭，且不会自动联网）。');
  }
  if (state.ai.network !== 'allowNetwork') {
    audit('denied', 'NETWORK_DISABLED');
    fail('NETWORK_DISABLED', '联网检索当前是「仅本地」：需要到「设置 → AI」切到「允许联网检索」。');
  }
  if (!state.ai.networkConfirmedAt || !confirmed) {
    audit('denied', 'NETWORK_UNCONFIRMED');
    fail('NETWORK_DISABLED', '本次联网尚未确认：请先确认，本次不会发起任何请求。');
  }
  if (!state.ai.allowFetchBody) {
    audit('denied', 'FETCH_BODY_DISABLED');
    fail('NETWORK_DISABLED', '抓取网页正文未开启：请在「设置 → AI」打开。');
  }
  const host = hostOf(url);
  if (state.ai.domainDenylist.some((rule) => domainMatches(rule, host))) {
    audit('denied', 'DOMAIN_DENIED');
    fail('NETWORK_DISABLED', `域名 ${host} 在黑名单里，已拒绝抓取。`);
  }
  if (
    state.ai.domainAllowlist.length > 0 &&
    !state.ai.domainAllowlist.some((rule) => domainMatches(rule, host))
  ) {
    audit('denied', 'DOMAIN_NOT_ALLOWED');
    fail('NETWORK_DISABLED', `域名 ${host} 不在白名单里，已拒绝抓取。`);
  }
  return null;
}

function pushAudit(
  entries: AiAuditEntry[],
  capability: string,
  outbound: boolean,
  outcome: AiAuditEntry['outcome'],
  note: string | null,
): AiAuditEntry[] {
  const next = [...entries, { at: now(), capability, outbound, outcome, durationMs: 0, note }];
  const limit = Math.max(50, state.ai.auditMaxEntries);
  return next.slice(-limit);
}

/** 浏览器预览端的剪藏结果（不联网）。 */
function mockClip(url: string): ClippedPage {
  const normalized = url.startsWith('http') ? url : `https://${url}`;
  const host = hostOf(normalized);
  const markdown = `# 剪藏示例：${host}\n\n这是浏览器预览模式下的模拟抓取结果。`;
  return {
    url,
    finalUrl: normalized,
    title: `剪藏示例 · ${host}`,
    site: host,
    markdown,
    wordCount: markdown.replace(/\s/g, '').length,
    fetchedAt: now(),
    bytes: markdown.length,
  };
}

// ---------------------------------------------------------------- 研究助手（D1/D2）

/** 子问题的「意图标记」：把命中的材料按规则归属到子问题（与 Rust 端同一套表）。 */
const RESEARCH_INTENT_MARKERS: string[][] = [
  ['定义', '概念', '是什么', '概述', '介绍', '原理'],
  ['方案', '做法', '实现', '对比', '比较', '选型', '用法', '实践'],
  ['场景', '适用', '限制', '注意', '前提', '边界', '约束', '条件'],
  ['风险', '争议', '问题', '失败', '坑', '缺陷', '缺点', '事故'],
  ['进展', '新版', '更新', '发布', '最近', '趋势', '未来'],
];

/** 过于宽泛、必须先追问范围的课题（与 Rust 端一致）。 */
const RESEARCH_BROAD_TOPICS = [
  'ai',
  '人工智能',
  '机器学习',
  '深度学习',
  '大模型',
  '编程',
  '技术',
  '笔记',
  '知识库',
  '知识管理',
  '工作',
  '生活',
  '学习',
  '投资',
  '写作',
];

const RESEARCH_TEMPLATES = [
  '「{topic}」的核心概念与定义是什么？',
  '「{topic}」有哪些主流方案或做法？',
  '「{topic}」的适用场景与限制条件是什么？',
  '「{topic}」常见的争议、风险或失败案例有哪些？',
  '近两年「{topic}」有什么值得关注的新进展？',
];

const RESEARCH_HITS_PER_QUESTION = 6;
const RESEARCH_MAX_SOURCES = 24;
const RESEARCH_REVIEW_DAYS = 180;

/** 冲突判定的正/负面词表（D4 规则档，与 Rust 端同表）。 */
const RESEARCH_POSITIVE_MARKERS = ['支持', '可以', '推荐', '适用', '可行', '优势', '有效', '最佳', '建议使用'];
const RESEARCH_NEGATIVE_MARKERS = [
  '不支持',
  '不能',
  '无法',
  '禁止',
  '避免',
  '不要',
  '风险',
  '缺陷',
  '失败',
  '限制',
  '问题',
];

const RESEARCH_LEVEL_LABEL: Record<ResearchSourceLevel, string> = {
  A: 'A 权威',
  B: 'B 可信',
  C: 'C 一般',
  D: 'D 弱',
  E: 'E 不可用',
};

const RESEARCH_LEVEL_RELIABILITY: Record<ResearchSourceLevel, number> = {
  A: 5,
  B: 4,
  C: 3,
  D: 2,
  E: 1,
};

/** 浏览器预览端的分级规则（与 Rust `source_grade` 的内置表同义，只覆盖常用域名）。 */
const MOCK_DOMAIN_LEVELS: Array<[string, ResearchSourceLevel]> = [
  ['docs.rs', 'A'],
  ['doc.rust-lang.org', 'A'],
  ['developer.mozilla.org', 'A'],
  ['ietf.org', 'A'],
  ['github.com', 'B'],
  ['blog.rust-lang.org', 'B'],
  ['aws.amazon.com', 'B'],
  ['infoq.com', 'C'],
  ['medium.com', 'D'],
  ['csdn.net', 'D'],
  ['juejin.cn', 'D'],
  ['stackoverflow.com', 'D'],
  ['reddit.com', 'E'],
  ['twitter.com', 'E'],
  ['x.com', 'E'],
];

function mockGradeWeb(url: string, title: string, snippet: string): { level: ResearchSourceLevel; reason: string } {
  const haystack = `${title} ${snippet}`.toLowerCase();
  if (haystack.includes('ai 生成') || haystack.includes('generated by ai')) {
    return { level: 'E', reason: '内容自证为 AI 生成，只作线索' };
  }
  const host = hostOf(url);
  const matched = MOCK_DOMAIN_LEVELS.filter(
    ([rule]) => host === rule || host.endsWith(`.${rule}`),
  ).sort((left, right) => right[0].length - left[0].length)[0];
  if (matched) {
    return { level: matched[1], reason: `域名 ${host} 命中内置分级表（${matched[0]}）` };
  }
  if (title && /\d{4}[-/年]\d{1,2}/.test(snippet)) {
    return { level: 'C', reason: '未命中分级表，但标题与日期齐全，按「一般」处理' };
  }
  return { level: title ? 'D' : 'E', reason: '未命中分级表，按「弱」处理' };
}

/** 浏览器预览端的模拟检索结果（不联网；桌面端用你配置的检索源）。 */
function mockWebSearch(keyword: string, limit: number) {
  const results = [
    {
      title: 'Tokio 官方文档：任务调度',
      url: 'https://docs.rs/tokio/latest/tokio/task/index.html',
      snippet: `${keyword} 的调度实现说明（浏览器预览模式：模拟检索结果，未联网）。`,
      publishedAt: '2026-02-01',
    },
    {
      title: 'Rust 官方博客：异步运行时进展',
      url: 'https://blog.rust-lang.org/2026/01/15/async-update.html',
      snippet: `${keyword} 的最新进展（模拟结果）。更新于 2026-01-15。`,
      publishedAt: '2026-01-15',
    },
  ];
  return results.slice(0, Math.max(1, Math.min(limit, results.length))).map((item) => ({
    ...item,
    host: hostOf(item.url),
  }));
}

function LEVEL_FROM_RELIABILITY(value: number): ResearchSourceLevel {
  const levels: ResearchSourceLevel[] = ['E', 'D', 'C', 'B', 'A'];
  return levels[Math.min(5, Math.max(1, Math.round(value))) - 1] ?? 'C';
}

/** AI 总开关闸门（研究助手的每一步都要先过）。 */
function requireAi(): void {
  if (!state.ai.enabled) {
    fail('AI_DISABLED', 'AI 能力未开启：请到「设置 → AI」打开总开关（默认关闭，且不会自动联网）。');
  }
}

function auditResearch(capability: string, outcome: AiAuditEntry['outcome'], note: string | null) {
  state.aiAudit = pushAudit(state.aiAudit, capability, false, outcome, note);
}

function researchValidateTopic(raw: string): string {
  const topic = raw.trim();
  if (!topic) fail('INVALID_INPUT', '课题不能为空。');
  const chars = [...topic].length;
  if (chars < 4 || RESEARCH_BROAD_TOPICS.includes(topic.toLowerCase())) {
    fail(
      'INVALID_INPUT',
      `课题「${topic}」过于宽泛：请补充范围（领域、关注点或时间窗），例如「Rust 异步运行时的调度策略」。`,
    );
  }
  if (chars > 200) fail('INVALID_INPUT', '课题过长（上限 200 字），请精简后再试。');
  return topic;
}

function researchSubQuestions(topic: string, max: number): ResearchSubQuestion[] {
  const count = Math.max(1, Math.min(RESEARCH_TEMPLATES.length, Math.round(max)));
  return RESEARCH_TEMPLATES.slice(0, count).map((template, index) => ({
    id: `q${index + 1}`,
    question: template.replace('{topic}', topic),
    keywords: [topic],
    localHits: 0,
  }));
}

/** 关键词切分：ASCII 词 + 中文双字（近似后端的 FTS5 双字方案）。 */
function researchTokens(keyword: string): string[] {
  const tokens: string[] = [];
  (keyword.match(/[A-Za-z0-9_+-]{2,}/g) ?? []).forEach((word) =>
    tokens.push(word.toLowerCase()),
  );
  (keyword.match(/[\u4e00-\u9fff]+/g) ?? []).forEach((run) => {
    const chars = [...run];
    if (chars.length === 1) tokens.push(chars[0]!);
    for (let index = 0; index + 1 < chars.length; index += 1) {
      tokens.push(`${chars[index]}${chars[index + 1]}`);
    }
  });
  return Array.from(new Set(tokens));
}

/** 本地检索：按命中词数排序（浏览器预览不跑 SQLite，用等价规则近似）。 */
function researchLocalHits(topic: string, limit: number): Note[] {
  const tokens = researchTokens(topic);
  // 后端 FTS5 的 MATCH 是 AND + 中文连续短语，比「命中任意一个词」严格得多；
  // 这里要求命中过半的词，避免把只沾一个字的无关笔记也捞成「来源」。
  const required = Math.max(1, Math.ceil(tokens.length / 2));
  return state.notes
    .filter((note) => note.status === 'active')
    .map((note) => {
      const haystack = `${note.title} ${state.contents[note.id] ?? ''}`.toLowerCase();
      return { note, hits: tokens.filter((token) => haystack.includes(token)).length };
    })
    .filter((item) => item.hits >= required)
    .sort((left, right) => right.hits - left.hits || right.note.updatedAt.localeCompare(left.note.updatedAt))
    .slice(0, limit)
    .map((item) => item.note);
}

function researchAssign(
  title: string,
  snippet: string,
  questions: ResearchSubQuestion[],
): number {
  const haystack = `${title} ${snippet}`.toLowerCase();
  let best = 0;
  let bestScore = 0;
  questions.forEach((_, index) => {
    const markers = RESEARCH_INTENT_MARKERS[index] ?? [];
    const score = markers.filter((marker) => haystack.includes(marker.toLowerCase())).length;
    if (score > bestScore) {
      bestScore = score;
      best = index;
    }
  });
  return best;
}

function researchGrade(note: Note): { level: ResearchSourceLevel; reason: string } {
  if (typeof note.sourceReliability === 'number') {
    return {
      level: LEVEL_FROM_RELIABILITY(note.sourceReliability),
      reason: `沿用笔记已标注的 sourceReliability=${note.sourceReliability}`,
    };
  }
  switch ((note.sourceType ?? '').trim()) {
    case 'official':
      return { level: 'B', reason: '来源类型为 official，但未标注可靠性' };
    case 'book':
      return { level: 'B', reason: '来源类型为 book，但未标注可靠性' };
    case 'experiment':
      return { level: 'C', reason: '来源类型为 experiment（自测数据）' };
    case 'personal':
      return { level: 'D', reason: '来源类型为 personal（个人推测）' };
    default:
      return {
        level: 'D',
        reason: '笔记未标注 sourceReliability：按「弱」处理（建议在笔记里补标注）',
      };
  }
}

function researchFirstSentence(text: string): string {
  const trimmed = text.trim();
  const cut = trimmed.search(/[。！？；\n]/);
  const head = cut === -1 ? trimmed : trimmed.slice(0, cut + 1);
  return head.split(/\s+/).join(' ');
}

function researchIsoDate(offsetDays = 0): string {
  const date = new Date();
  date.setDate(date.getDate() + offsetDays);
  // 用本地日期（与 Rust 端 `today_date()` 一致）：toISOString 会按 UTC 差一天
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function researchCitations(claim: ResearchClaim): string {
  return claim.citations.map((value) => `[${value}]`).join(' ');
}

function researchStale(note: Note, windowDays: number): boolean {
  if (note.validUntil) return note.validUntil.slice(0, 10) < researchIsoDate();
  const ageDays = (Date.now() - new Date(note.updatedAt).getTime()) / 86_400_000;
  return ageDays > windowDays;
}

/** 验证阶段：按子问题汇总（≥2 个独立来源才算交叉验证）。 */
function researchVerifyClaims(session: ResearchSession): ResearchClaim[] {
  const questions = session.plan?.subQuestions ?? [];
  return questions.map((question) => {
    const evidence = session.sources.filter((source) => source.subQuestionId === question.id);
    if (evidence.length === 0) {
      return {
        subQuestionId: question.id,
        text: `「${question.question}」：本地知识库暂无材料。`,
        citations: [],
        bestLevel: 'E',
        crossVerified: false,
        fresh: true,
        relatedNotes: [],
      };
    }
    const independent = Array.from(
      new Set(evidence.map((source) => source.noteId ?? `ref${source.reference}`)),
    );
    const quotes = evidence
      .slice(0, 2)
      .map((source) => researchFirstSentence(source.snippet))
      .filter((value) => value.length > 0);
    const levels = evidence.map((source) => source.level);
    const bestLevel = levels.reduce<ResearchSourceLevel>(
      (best, level) => (level > best ? level : best),
      'E',
    );
    return {
      subQuestionId: question.id,
      text: `关于「${question.question}」：${quotes.join('；')}`,
      citations: evidence.map((source) => source.reference),
      bestLevel,
      crossVerified: independent.length >= 2,
      fresh: !evidence.some((source) => source.stale),
      relatedNotes: evidence
        .map((source) => source.noteId)
        .filter((value): value is string => Boolean(value)),
    };
  });
}

/** 汇总阶段：把验证结果拆成报告的六段 + 警告。 */
function researchBuildReport(session: ResearchSession, claims: ResearchClaim[]): ResearchReport {
  const seen: string[] = [];
  const conclusions: ResearchClaim[] = [];
  const keyFacts: ResearchClaim[] = [];
  const pending: ResearchClaim[] = [];
  const conflicts: ResearchClaim[] = [];
  // 同一子问题下同时出现支持与反对/限制表述 → 并列呈现、不选边（D4 规则档）
  const conflicted: string[] = [];
  (session.plan?.subQuestions ?? []).forEach((question) => {
    const evidence = session.sources.filter((source) => source.subQuestionId === question.id);
    if (evidence.length < 2) return;
    const polarities = evidence.map((source) => {
      const haystack = `${source.title} ${source.snippet}`.toLowerCase();
      const positive = RESEARCH_POSITIVE_MARKERS.filter((marker) => haystack.includes(marker)).length;
      const negative = RESEARCH_NEGATIVE_MARKERS.filter((marker) => haystack.includes(marker)).length;
      return positive - negative;
    });
    if (polarities.some((value) => value > 0) && polarities.some((value) => value < 0)) {
      conflicted.push(question.id);
    }
  });
  claims.forEach((claim) => {
    if (conflicted.includes(claim.subQuestionId)) {
      conflicts.push({
        ...claim,
        text: `「${
          session.plan?.subQuestions.find((item) => item.id === claim.subQuestionId)?.question ??
          claim.subQuestionId
        }」存在冲突：来源之间既有支持性表述、也有反对/限制性表述，本次不选边。${claim.text}`,
      });
      return;
    }
    if (!claim.crossVerified) {
      pending.push(claim);
      return;
    }
    const fingerprint = claim.citations.join(',');
    if (seen.includes(fingerprint)) return;
    seen.push(fingerprint);
    if (claim.fresh && conclusions.length < 5) conclusions.push(claim);
    if (keyFacts.length < 8) keyFacts.push(claim);
  });

  const relations: ResearchKbRelation[] = [];
  (session.plan?.localBaseline ?? []).forEach((baseline) => {
    const used = session.sources.some((source) => source.noteId === baseline.noteId);
    relations.push({
      noteId: baseline.noteId,
      title: baseline.title,
      relation: used ? '补充（已作为证据）' : '相关（本地已有笔记）',
    });
  });
  session.sources.forEach((source) => {
    if (!source.noteId || relations.some((item) => item.noteId === source.noteId)) return;
    relations.push({ noteId: source.noteId, title: source.title, relation: '证据来源' });
  });

  const warnings: ResearchWarning[] = [];
  const unverified = session.sources.filter((source) => source.level === 'E');
  if (unverified.length > 0) {
    warnings.push({
      kind: 'unverifiedSources',
      message: `有 ${unverified.length} 条来源属于「E 不可用」：只作线索，未进入结论段。`,
      references: unverified.map((source) => source.reference),
    });
  }
  const single = pending.flatMap((claim) => claim.citations);
  if (single.length > 0) {
    warnings.push({
      kind: 'singleSource',
      message: `有 ${single.length} 条来源是单一来源、没有交叉验证：只列在「待验证清单」，不进结论段。`,
      references: single,
    });
  }
  const stale = session.sources.filter((source) => source.stale);
  if (conflicts.length > 0) {
    warnings.push({
      kind: 'conflict',
      message: `有 ${conflicts.length} 个子问题的来源互相矛盾：已并列呈现在「争议与不确定性」段，本次不替你选边。`,
      references: conflicts.flatMap((claim) => claim.citations),
    });
  }
  if (stale.length > 0) {
    warnings.push({
      kind: 'stale',
      message: `有 ${stale.length} 条来源可能已过时（超出时间窗或 valid_until 已过）：请核对时效后再引用。`,
      references: stale.map((source) => source.reference),
    });
  }
  if (session.sources.length === 0) {
    warnings.push({
      kind: 'noLocalEvidence',
      message: '本地知识库没有找到任何材料：本报告只有计划与建议，没有结论。',
      references: [],
    });
  }
  if (state.ai.network === 'allowNetwork') {
    warnings.push({
      kind: 'externalNotAvailable',
      message: '联网档位已打开，但外部检索尚未接入（阶段 D3）：本次未发起任何网络请求。',
      references: [],
    });
  }
  warnings.push({
    kind: 'templateGenerated',
    message:
      '本报告由**模板实现**生成（未接入模型）：结论段是来源原句摘录，推测段只是待确认方向的罗列。',
    references: [],
  });

  return {
    generatedAt: now(),
    dataCutoff: researchIsoDate(),
    composer: 'template.draft',
    conclusions,
    keyFacts,
    conflicts,
    kbRelations: relations,
    pending,
    references: session.sources,
    speculation: [],
    warnings,
    sourceReliability: session.sources.reduce(
      (lowest, source) => Math.min(lowest, source.reliability),
      5,
    ),
    validUntil: researchIsoDate(RESEARCH_REVIEW_DAYS),
    markdown: '',
  };
}

function researchEscapeTable(value: string): string {
  return value.replace(/\|/g, '\\|').replace(/\n/g, ' ');
}

/** 渲染固定六段报告（顺序不可变）。 */
function researchRenderMarkdown(session: ResearchSession, report: ResearchReport): string {
  const plan = session.plan;
  const out: string[] = [];
  out.push(`# 调研报告：${plan?.topic ?? session.request.topic}`, '');
  out.push(`> **资料截至 ${report.dataCutoff}** · 生成方式 \`${report.composer}\`（未接入模型）`);
  out.push(
    `> 会话 \`${session.id}\` · 子问题 ${plan?.subQuestions.length ?? 0} 个 · 来源 ${report.references.length} 条`,
    '',
  );
  if (report.warnings.length > 0) {
    out.push('> **⚠️ 警告**');
    report.warnings.forEach((warning) => {
      const refs = warning.references.map((value) => `[${value}]`).join('');
      out.push(`> - ${warning.message}${refs ? `（${refs}）` : ''}`);
    });
    out.push('');
  }

  out.push('## 1. 结论先行', '');
  if (report.conclusions.length === 0) {
    out.push('_本次没有达到「≥2 个独立来源」的结论。_', '');
  } else {
    report.conclusions.forEach((claim, index) => {
      out.push(`${index + 1}. ${claim.text} ${researchCitations(claim)}`);
    });
    out.push('');
  }

  out.push('## 2. 关键事实', '');
  if (report.keyFacts.length === 0) {
    out.push('_没有交叉验证过的事实。_', '');
  } else {
    out.push('| 结论 | 支持来源（级别） | 是否交叉验证 | 时效 |');
    out.push('| --- | --- | --- | --- |');
    report.keyFacts.forEach((claim) => {
      const levels = report.references
        .filter((source) => claim.citations.includes(source.reference))
        .map((source) => `[${source.reference}] ${RESEARCH_LEVEL_LABEL[source.level]}`)
        .join('、');
      out.push(
        `| ${researchEscapeTable(claim.text)} | ${researchEscapeTable(levels)} | ${
          claim.crossVerified ? '是' : '否'
        } | ${claim.fresh ? '时间内' : '可能已过时'} |`,
      );
    });
    out.push('');
  }

  out.push('## 3. 争议与不确定性', '');
  if (report.conflicts.length === 0) {
    out.push(
      '_本次未检出冲突（规则档：只有同一子问题下同时出现支持与反对/限制表述才会判为冲突）。_',
      '',
    );
  } else {
    report.conflicts.forEach((claim) => out.push(`- ${claim.text} ${researchCitations(claim)}`));
    out.push('');
  }

  out.push('## 4. 与本知识库的关系', '');
  if (report.kbRelations.length === 0) {
    out.push('_没有关联的本地笔记。_', '');
  } else {
    report.kbRelations.forEach((relation) =>
      out.push(`- [[${relation.title}]] —— ${relation.relation}`),
    );
    out.push('');
  }

  out.push('## 5. 待验证清单', '');
  if (report.pending.length === 0) {
    out.push('_没有待验证项。_', '');
  } else {
    report.pending.forEach((claim) => {
      const why =
        claim.citations.length === 0
          ? '本地没有材料：建议补充或（阶段 D3 起）联网检索'
          : '只有单一来源，未交叉验证';
      out.push(`- ${claim.text} ${researchCitations(claim)} —— ${why}`);
    });
    out.push('');
  }

  out.push('## 6. 参考来源', '');
  if (report.references.length === 0) {
    out.push('_没有参考来源。_');
  } else {
    report.references.forEach((source) => {
      const location = source.url ?? (source.noteId ? `note://${source.noteId}` : '本地');
      out.push(
        `[${source.reference}] ${source.title} · ${RESEARCH_LEVEL_LABEL[source.level]} · published ${
          source.publishedAt ?? '时效未知'
        } · 获取 ${source.fetchedAt} · ${location}（${source.gradeReason}）`,
      );
    });
  }
  return out.join('\n');
}

function researchFindDuplicates(title: string): ResearchDuplicateNote[] {
  const key = title.split(/\s+/).join('').toLowerCase();
  return state.notes
    .filter((note) => note.status === 'active')
    .filter((note) => note.title.split(/\s+/).join('').toLowerCase() === key)
    .map((note) => ({ noteId: note.id, title: note.title, updatedAt: note.updatedAt }));
}

function researchBuildProposal(session: ResearchSession, report: ResearchReport): ResearchProposal {
  const title = `调研报告：${session.request.topic}`;
  const sourceRef = report.references
    .map((source) =>
      source.url
        ? `[${source.reference}] ${source.title} ${source.url}`
        : `[${source.reference}] ${source.title} (${RESEARCH_LEVEL_LABEL[source.level]} 笔记)`,
    )
    .join('；');
  const body = [
    report.markdown,
    '',
    '---',
    '',
    `> 本报告由研究助手生成（${report.composer}）：结论段为**来源原句摘录**，未做语义归并；`,
    '> 推测段默认不写入笔记，需要时请在写入确认里勾选。',
    `> 会话 \`${session.id}\` · 生成于 ${report.generatedAt}`,
  ].join('\n');
  return {
    title,
    categoryId: null,
    tags: ['调研报告', '待复核'],
    sourceType: 'research',
    sourceRef,
    sourceReliability: report.sourceReliability,
    validUntil: report.validUntil ?? null,
    body,
    diffPreview: body
      .split('\n')
      .map((line) => `+${line}`)
      .join('\n'),
    duplicates: researchFindDuplicates(title),
    suggestSourceNote: report.references.length > 3,
  };
}

function researchSave(session: ResearchSession): ResearchSession {
  session.updatedAt = now();
  // 存快照并按值返回：真实 IPC 每次都会反序列化成新对象，
  // 如果这里把同一个引用交给界面，React 的状态比较会认为「没变化」而不重渲染。
  state.research[session.id] = structuredClone(session);
  persist();
  return structuredClone(session);
}

function researchRequire(sessionId: string): ResearchSession {
  const session = state.research[sessionId];
  if (!session) fail('INVALID_INPUT', `找不到研究会话 ${sessionId}。`);
  return structuredClone(session);
}

function researchRequireStatus(session: ResearchSession, expected: ResearchStatus[], action: string) {
  if (!expected.includes(session.status)) {
    fail(
      'INVALID_INPUT',
      `会话当前状态是 ${session.status}，${action}需要状态为「${expected.join(' / ')}」。`,
    );
  }
}

function researchStep(session: ResearchSession, step: string, detail: string) {
  session.steps.push({ step, at: now(), detail });
}

function plainText(markdown: string): string {
  return markdown
    .replace(/^---[\s\S]*?---\n?/, '')
    .replace(/[#*_>`~]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
}

function buildSnippet(text: string, keyword: string, max = 120): TextSegment[] {
  const index = text.toLowerCase().indexOf(keyword.toLowerCase());
  if (index === -1) {
    return [{ text: text.slice(0, max), matched: false }];
  }
  const start = Math.max(0, index - 40);
  const end = Math.min(text.length, start + max);
  const window_ = text.slice(start, end);
  const localIndex = window_.toLowerCase().indexOf(keyword.toLowerCase());
  return [
    { text: start > 0 ? '…' : '', matched: false },
    { text: window_.slice(0, localIndex), matched: false },
    { text: window_.slice(localIndex, localIndex + keyword.length), matched: true },
    { text: window_.slice(localIndex + keyword.length), matched: false },
  ].filter((segment) => segment.text.length > 0);
}

function nextCategoryDirectory(name: string, sortOrder: number) {
  return `${String(sortOrder % 100).padStart(2, '0')}_${name.replace(/[:/\\*?"<>|]/g, '-')}`;
}

function openWorkspace(name: string, root: string): WorkspaceOpenResult {
  const workspace: Workspace = {
    id: state.workspace?.id ?? `ws_${ulid()}`,
    name,
    rootPath: root,
    createdAt: state.workspace?.createdAt ?? now(),
    updatedAt: now(),
  };
  state.workspace = workspace;
  state.settings = { ...state.settings, ...emptySettings };
  if (state.categories.length === 0) {
    ['01_技术', '02_工作', '03_项目', '04_生活', '99_Archive'].forEach((dir, index) => {
      state.categories.push({
        id: `cat_${ulid()}`,
        workspaceId: workspace.id,
        parentId: null,
        name: dir.replace(/^\d+_/, ''),
        sortOrder: index + 1,
        dirName: dir,
        createdAt: now(),
        noteCount: 0,
      });
    });
  }
  if (state.notes.length === 0) {
    const id = `note_${ulid()}`;
    const createdAt = now();
    state.notes.push({
      id,
      workspaceId: workspace.id,
      title: '欢迎使用 Personal Knowledge OS',
      filePath: `00_Inbox/${createdAt.slice(0, 10)}-welcome.md`,
      categoryId: null,
      status: 'active',
      contentHash: 'mock',
      wordCount: 120,
      favorite: false,
      createdAt,
      updatedAt: createdAt,
      lastIndexedAt: createdAt,
    });
    state.contents[id] = [
      '# 欢迎使用 Personal Knowledge OS',
      '',
      '这是浏览器预览模式的示例内容，真正的数据保存在 Tauri 本地工作区中。',
      '',
      '## 常用操作',
      '',
      '- `Cmd/Ctrl + N` 新建笔记',
      '- `Cmd/Ctrl + K` 全局搜索',
      '- 左侧分类树用于组织内容',
      '',
      '```rust',
      'let workspace = "MyKnowledge";',
      '```',
    ].join('\n');
    state.noteTags[id] = ['入门'];
    state.tags.push({ id: `tag_入门`, name: '入门', createdAt });
  }
  persist();
  return {
    workspace,
    stats: computeStats(),
    settings: state.settings,
    indexStatus: indexStatus(),
    warnings: [],
    initialized: true,
    scan: {
      added: 0,
      updated: 0,
      missing: 0,
      unchanged: state.notes.length,
      durationMs: 1,
      warnings: [],
    },
  };
}

function computeStats() {
  const active = state.notes.filter((note) => note.status === 'active');
  const daysAgo = (days: number) => Date.now() - days * 24 * 60 * 60 * 1000;
  const updatedSince = (days: number) =>
    active.filter((note) => new Date(note.updatedAt).getTime() >= daysAgo(days)).length;
  return {
    noteCount: active.length,
    inboxCount: active.filter((note) => !note.categoryId).length,
    trashCount: state.notes.filter((note) => note.status === 'deleted').length,
    categoryCount: state.categories.length,
    tagCount: state.tags.length,
    attachmentCount: 0,
    // 与后端 SQL 的语义保持一致（统计明细，对标语雀「统计」）
    totalWords: active.reduce((sum, note) => sum + note.wordCount, 0),
    updatedLast7d: updatedSince(7),
    updatedLast30d: updatedSince(30),
    pinnedCount: active.filter((note) => note.pinned).length,
    // 近 14 天写作活跃度：按本地日期聚合（与后端 SQL 的语义一致）
    activity: (() => {
      const buckets = new Map<string, number>();
      for (let offset = 13; offset >= 0; offset -= 1) {
        const day = new Date(Date.now() - offset * 24 * 60 * 60 * 1000);
        buckets.set(day.toISOString().slice(0, 10), 0);
      }
      active.forEach((note) => {
        const day = note.updatedAt.slice(0, 10);
        if (buckets.has(day)) buckets.set(day, (buckets.get(day) ?? 0) + 1);
      });
      return [...buckets.entries()].map(([date, count]) => ({ date, count }));
    })(),
  };
}

function indexStatus(): IndexStatus {
  const active = state.notes.filter((note) => note.status === 'active').length;
  return {
    state: 'ready',
    indexedNotes: active,
    totalNotes: active,
    lastRebuiltAt: now(),
    lastError: null,
  };
}

function listNotes(query: NoteListQuery = {}) {
  const status = query.status ?? 'active';
  let items = state.notes.filter((note) => note.status === status);
  if (query.categoryId) {
    const ids = new Set<string>([query.categoryId]);
    let added = true;
    while (added) {
      added = false;
      state.categories.forEach((category) => {
        if (category.parentId && ids.has(category.parentId) && !ids.has(category.id)) {
          ids.add(category.id);
          added = true;
        }
      });
    }
    items = items.filter((note) => note.categoryId && ids.has(note.categoryId));
  }
  if (query.inboxOnly) items = items.filter((note) => !note.categoryId);
  if (query.rootOnly) items = items.filter((note) => !note.parentId);
  if (query.favoriteOnly) items = items.filter((note) => note.favorite);
  if (query.tagId)
    items = items.filter((note) => (state.noteTags[note.id] ?? []).includes(query.tagId!));
  if (query.keyword) {
    const keyword = query.keyword.toLowerCase();
    items = items.filter(
      (note) =>
        note.title.toLowerCase().includes(keyword) ||
        (state.contents[note.id] ?? '').toLowerCase().includes(keyword),
    );
  }
  const sort = query.sort ?? 'updatedDesc';
  items = [...items].sort((a, b) => {
    // 置顶在所有排序方式下都排在最前（与后端 ORDER BY pinned DESC, ... 一致）
    if ((a.pinned ?? false) !== (b.pinned ?? false)) return a.pinned ? -1 : 1;
    if (sort === 'titleAsc') return a.title.localeCompare(b.title, 'zh-CN');
    if (sort === 'createdDesc') return b.createdAt.localeCompare(a.createdAt);
    if (sort === 'updatedAsc') return a.updatedAt.localeCompare(b.updatedAt);
    if (sort === 'manual') {
      // 与后端一致：order 升序，未排序（0）时用更新时间兜底
      const left = a.sortOrder ?? 0;
      const right = b.sortOrder ?? 0;
      if (left !== right) return left - right;
      return b.updatedAt.localeCompare(a.updatedAt);
    }
    return b.updatedAt.localeCompare(a.updatedAt);
  });
  const page = Math.max(1, query.page ?? 1);
  const pageSize = Math.min(500, Math.max(1, query.pageSize ?? 200));
  const start = (page - 1) * pageSize;
  return {
    items: items.slice(start, start + pageSize).map(toSummary),
    total: items.length,
    page,
    pageSize,
  };
}

function createNote(input: CreateNoteInput): NoteDetail {
  const workspace = requireWorkspace();
  // 子文档：parentId 可以是父笔记 id，也可以是标题（与后端解析规则一致）
  const parent = findParentNote(input.parentId);
  const createdAt = now();
  const id = `note_${ulid()}`;
  const title = input.title.trim() || '未命名笔记';
  const filePath = `00_Inbox/${createdAt.slice(0, 19).replace(/[-:T]/g, '')}-${title.replace(/\s+/g, '-')}.md`;
  const note: Note = {
    id,
    workspaceId: workspace.id,
    title,
    filePath,
    // 未显式指定分类时继承父文档分类
    categoryId: input.categoryId ?? parent?.categoryId ?? null,
    status: 'active',
    contentHash: ulid(),
    wordCount: plainText(input.initialContent ?? '').length,
    favorite: false,
    createdAt,
    updatedAt: createdAt,
    lastIndexedAt: createdAt,
    parentId: parent?.id ?? null,
    sortOrder: 0,
    pinned: false,
  };
  state.notes.push(note);
  state.contents[id] = input.initialContent ?? '';
  state.noteTags[id] = input.tags ?? [];
  (input.tags ?? []).forEach((name) => {
    if (!state.tags.some((tag) => tag.name === name)) {
      state.tags.push({ id: `tag_${name}`, name, createdAt });
    }
  });
  persist();
  emit('notes://changed', { kind: 'created', noteId: id });
  return toDetail(note);
}

function captureHistory(noteId: string, content: string) {
  if (!content.trim()) return;
  const entries = state.history[noteId] ?? [];
  if (entries[0]?.content === content) return; // 内容没变，不重复留档
  entries.unshift({
    entryId: `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    savedAt: now(),
    content,
  });
  state.history[noteId] = entries.slice(0, 50); // 与后端一致：每篇最多 50 份
}

function saveNote(input: SaveNoteInput) {
  const note = state.notes.find((item) => item.id === input.id);
  if (!note) fail('NOTE_NOT_FOUND', '笔记不存在或已被删除。');
  if (input.baseContentHash && input.baseContentHash !== note.contentHash) {
    fail('NOTE_CONFLICT', '文件已被其他程序修改，请选择重新加载或覆盖保存。', {
      currentContent: state.contents[note.id] ?? '',
    });
  }
  // 覆盖前留档（对标语雀「查看历史版本」）：只在正文真的变了时记，
  // 否则重复触发自动保存会把「当前内容」也存成历史，还原等于没还原
  if ((state.contents[note.id] ?? '') !== input.content) {
    captureHistory(note.id, state.contents[note.id] ?? '');
  }
  state.contents[note.id] = input.content;
  note.contentHash = ulid();
  note.updatedAt = now();
  note.wordCount = plainText(input.content).length;
  if (input.title) note.title = input.title;
  if (input.categoryId !== undefined) note.categoryId = input.categoryId;
  if (input.tags) state.noteTags[note.id] = input.tags;
  persist();
  emit('notes://changed', { kind: 'updated', noteId: note.id });
  return {
    note,
    contentHash: note.contentHash!,
    savedAt: note.updatedAt,
    indexUpdated: true,
    indexWarning: null,
  };
}

function searchNotes(query: SearchQuery) {
  const keyword = query.keyword.trim();
  const page = Math.max(1, query.page ?? 1);
  const pageSize = Math.min(100, Math.max(1, query.pageSize ?? 50));
  if (!keyword) {
    const result = listNotes({
      categoryId: query.categoryId,
      tagId: query.tagId,
      favoriteOnly: query.favoriteOnly,
      page,
      pageSize,
    });
    return {
      keyword,
      hits: result.items.map((note) => ({
        note,
        score: 0,
        titleMatched: false,
        snippet: [],
        matchedTerms: [],
      })),
      total: result.total,
      page,
      pageSize,
      tookMs: 2,
      indexStale: false,
    };
  }

  let notes = state.notes.filter((note) => note.status === 'active');
  if (query.categoryId) notes = notes.filter((note) => note.categoryId === query.categoryId);
  if (query.favoriteOnly) notes = notes.filter((note) => note.favorite);
  if (query.tagId)
    notes = notes.filter((note) => (state.noteTags[note.id] ?? []).includes(query.tagId!));
  if (query.scopeNoteId) notes = notes.filter((note) => note.id === query.scopeNoteId);

  const lower = keyword.toLowerCase();
  const hits = notes
    .map((note) => {
      const content = state.contents[note.id] ?? '';
      const titleMatched = note.title.toLowerCase().includes(lower);
      const contentHit = content.toLowerCase().includes(lower);
      if (!titleMatched && !contentHit) return null;
      return {
        note: toSummary(note),
        score: titleMatched ? -1 : -0.5,
        titleMatched,
        snippet: contentHit ? buildSnippet(plainText(content), keyword) : [],
        matchedTerms: [keyword],
      };
    })
    .filter((hit): hit is NonNullable<typeof hit> => hit !== null)
    .sort((a, b) => Number(b.titleMatched) - Number(a.titleMatched) || a.score - b.score);

  const start = (page - 1) * pageSize;
  return {
    keyword,
    hits: hits.slice(start, start + pageSize),
    total: hits.length,
    page,
    pageSize,
    tookMs: 3,
    indexStale: false,
  };
}

// ---------------------------------------------------------------- 双向链接（V1.1 M1）
//
// 浏览器预览模式下的链接解析。规则与 Rust 端 `core/text.rs` 保持一致：
// `[[标题]]` / `[[标题|别名]]` / `[[标题#小节]]`、忽略围栏代码块与行内代码、
// 上下文摘要取所在行（过长时截断）。这样在不启动桌面外壳时也能完整预览反链面板。

function wikiLinkRows(): Array<{ source: Note; link: WikiLinkToken; target?: Note }> {
  const rows: Array<{ source: Note; link: WikiLinkToken; target?: Note }> = [];
  activeNotes().forEach((source) => {
    parseWikilinks(state.contents[source.id] ?? '').forEach((link) => {
      rows.push({ source, link, target: resolveLinkTarget(link.target) });
    });
  });
  return rows;
}

function activeNotes(): Note[] {
  return state.notes.filter((note) => note.status === 'active');
}

/** 目标解析：标题（忽略大小写）→ 相对路径 → 文件名。 */
function resolveLinkTarget(target: string): Note | undefined {
  const needle = target.trim().toLowerCase();
  if (!needle) return undefined;
  const notes = activeNotes();
  const byTitle = notes.find((note) => note.title.toLowerCase() === needle);
  if (byTitle) return byTitle;

  const normalized = needle.replace(/\\/g, '/').replace(/^\.\//, '');
  const candidates = normalized.endsWith('.md') ? [normalized] : [normalized, `${normalized}.md`];
  return notes.find((note) => {
    const path = note.filePath.toLowerCase().replace(/\\/g, '/');
    const fileName = path.split('/').pop() ?? '';
    return candidates.some(
      (candidate) =>
        path === candidate ||
        path.endsWith(`/${candidate}`) ||
        fileName === candidate ||
        fileName.replace(/\.md$/, '') === candidate,
    );
  });
}

function mockBacklinks(noteId: string): Backlink[] {
  const seen = new Set<string>();
  const backlinks: Backlink[] = [];
  wikiLinkRows().forEach(({ source, link, target }) => {
    if (!target || target.id !== noteId) return;
    const key = `${source.id}\u0000${link.context}`;
    if (seen.has(key)) return;
    seen.add(key);
    backlinks.push({
      noteId: source.id,
      title: source.title,
      filePath: source.filePath,
      context: link.context,
      anchor: link.anchor,
      updatedAt: source.updatedAt,
    });
  });
  return backlinks;
}

function mockOutgoing(noteId: string): OutgoingLink[] {
  const note = state.notes.find((item) => item.id === noteId);
  if (!note) return [];
  return parseWikilinks(state.contents[noteId] ?? '').map((link) => {
    const target = resolveLinkTarget(link.target);
    return {
      targetTitle: link.target,
      targetNoteId: target?.id ?? null,
      targetExists: Boolean(target),
      anchor: link.anchor,
      linkType: 'wikilink',
      context: link.context,
    };
  });
}

function mockUnresolved(): UnresolvedLink[] {
  return wikiLinkRows()
    .filter((row) => !row.target)
    .map((row) => ({
      sourceNoteId: row.source.id,
      sourceTitle: row.source.title,
      targetTitle: row.link.target,
      context: row.link.context,
    }));
}

function mockCounts(noteId: string): LinkCounts {
  const rows = wikiLinkRows();
  const outgoing = rows.filter((row) => row.source.id === noteId);
  return {
    backlinks: rows.filter((row) => row.target?.id === noteId).length,
    outgoing: outgoing.length,
    unresolved: outgoing.filter((row) => !row.target).length,
  };
}

function mockRebuild(): LinkRebuildReport {
  const startedAt = Date.now();
  const rows = wikiLinkRows();
  const resolved = rows.filter((row) => Boolean(row.target)).length;
  return {
    scannedNotes: activeNotes().length,
    links: rows.length,
    resolved,
    unresolved: rows.length - resolved,
    durationMs: Date.now() - startedAt,
  };
}

/** 浏览器预览模式下的知识图谱（与 Rust 端同一套过滤/截断语义）。 */
function mockGraph(query: GraphQuery): GraphData {
  const startedAt = Date.now();
  const listed = listNotes({
    categoryId: query.categoryId ?? undefined,
    tagId: query.tagId ?? undefined,
    keyword: query.keyword ?? undefined,
    status: 'active',
    pageSize: 500,
  });
  const nodesById = new Map(listed.items.map((item) => [item.id, item]));

  // 事件边：只保留两端都在候选集合内的已解析链接
  const edgeMap = new Map<string, { source: string; target: string; weight: number }>();
  const neighbours = new Map<string, Set<string>>();
  const connect = (source: string, target: string) => {
    if (source === target) return;
    const key = `${source}\u0000${target}`;
    const edge = edgeMap.get(key) ?? { source, target, weight: 0 };
    edge.weight += 1;
    edgeMap.set(key, edge);
    if (!neighbours.has(source)) neighbours.set(source, new Set());
    if (!neighbours.has(target)) neighbours.set(target, new Set());
    neighbours.get(source)!.add(target);
    neighbours.get(target)!.add(source);
  };

  state.notes
    .filter((note) => note.status === 'active')
    .forEach((note) => {
      parseWikilinks(state.contents[note.id] ?? '').forEach((link) => {
        const target = resolveLinkTarget(link.target);
        if (!target) return;
        if (!nodesById.has(note.id) || !nodesById.has(target.id)) return;
        connect(note.id, target.id);
      });
    });

  // 聚焦：以某篇笔记为中心做 N 跳
  let focus: Set<string> | null = null;
  const focusId = query.focusNoteId ?? null;
  if (focusId && nodesById.has(focusId)) {
    const hops = Math.min(3, Math.max(1, query.hops ?? 1));
    focus = new Set([focusId]);
    let frontier = [focusId];
    for (let step = 0; step < hops; step += 1) {
      const next: string[] = [];
      frontier.forEach((id) => {
        neighbours.get(id)?.forEach((neighbor) => {
          if (!focus!.has(neighbor)) {
            focus!.add(neighbor);
            next.push(neighbor);
          }
        });
      });
      if (next.length === 0) break;
      frontier = next;
    }
  }

  const includeOrphans = query.includeOrphans ?? true;
  let nodes: GraphNode[] = [...nodesById.values()]
    .filter((item) => !focus || focus.has(item.id))
    .map((item) => {
      const degree = neighbours.get(item.id)?.size ?? 0;
      return {
        id: item.id,
        title: item.title,
        categoryId: item.categoryId ?? null,
        updatedAt: item.updatedAt,
        degree,
        tags: state.noteTags[item.id] ?? [],
        orphan: degree === 0,
      };
    })
    .filter((node) => includeOrphans || !node.orphan);

  const totalNodes = nodes.length;
  const maxNodes = Math.max(1, query.maxNodes ?? 2000);
  let truncated = false;
  if (nodes.length > maxNodes) {
    nodes = [...nodes]
      .sort((left, right) => right.degree - left.degree || right.updatedAt.localeCompare(left.updatedAt))
      .slice(0, maxNodes);
    truncated = true;
  }

  const kept = new Set(nodes.map((node) => node.id));
  let edges = [...edgeMap.values()].filter(
    (edge) => kept.has(edge.source) && kept.has(edge.target),
  );
  const totalEdges = edges.length;
  const maxEdges = Math.max(1, query.maxEdges ?? 5000);
  if (edges.length > maxEdges) {
    edges = [...edges].sort((left, right) => right.weight - left.weight).slice(0, maxEdges);
    truncated = true;
  }

  // 截断后按可见边集重算邻居数
  const visible = new Map<string, Set<string>>();
  edges.forEach((edge) => {
    if (!visible.has(edge.source)) visible.set(edge.source, new Set());
    if (!visible.has(edge.target)) visible.set(edge.target, new Set());
    visible.get(edge.source)!.add(edge.target);
    visible.get(edge.target)!.add(edge.source);
  });
  nodes = nodes.map((node) => {
    const degree = visible.get(node.id)?.size ?? 0;
    return { ...node, degree, orphan: degree === 0 };
  });

  return {
    nodes,
    edges: edges.map((edge) => ({
      source: edge.source,
      target: edge.target,
      linkType: 'wikilink' as const,
      weight: edge.weight,
    })),
    truncated,
    totalNodes,
    totalEdges,
    durationMs: Date.now() - startedAt,
  };
}

function categoryTree(): CategoryNode[] {
  const build = (parentId: string | null, depth: number): CategoryNode[] =>
    state.categories
      .filter((category) => (category.parentId ?? null) === parentId)
      .sort((a, b) => a.sortOrder - b.sortOrder)
      .map((category) => ({
        category: {
          ...category,
          noteCount: state.notes.filter(
            (note) => note.categoryId === category.id && note.status === 'active',
          ).length,
        },
        depth,
        children: build(category.id, depth + 1),
      }));
  return build(null, 0);
}

/** 命令分发。 */
export async function handleMockInvoke(
  command: string,
  args?: Record<string, unknown>,
): Promise<unknown> {
  const payload = (args ?? {}) as Record<string, unknown>;
  switch (command) {
    case 'app_info':
      return {
        name: 'Personal Knowledge OS',
        version: '1.0.0',
        schemaVersion: 2,
        dataDir: '（浏览器预览模式）',
        platform: 'web-preview',
        sqliteVersion: 'preview',
      };
    case 'app_ready':
      return true;
    case 'app_log_path':
      return null;
    case 'workspace_active_id':
      return state.workspace?.id ?? null;
    case 'workspace_root_path':
      return requireWorkspace().rootPath;
    case 'workspace_list':
      return state.workspace
        ? [
            {
              workspace: state.workspace,
              stats: computeStats(),
              isActive: true,
              isAvailable: true,
            },
          ]
        : [];
    case 'workspace_get_current':
      return state.workspace
        ? {
            workspace: state.workspace,
            stats: computeStats(),
            settings: state.settings,
            indexStatus: indexStatus(),
            warnings: [],
            initialized: false,
            scan: { added: 0, updated: 0, missing: 0, unchanged: 0, durationMs: 0, warnings: [] },
          }
        : null;
    case 'workspace_create':
    case 'workspace_open':
      return openWorkspace(
        String(payload.name ?? '预览知识库'),
        String(payload.root ?? '~/MockKnowledge'),
      );
    case 'workspace_switch':
      return openWorkspace(
        state.workspace?.name ?? '预览知识库',
        state.workspace?.rootPath ?? '~/MockKnowledge',
      );
    case 'workspace_close':
      state.workspace = null;
      persist();
      return null;
    case 'workspace_forget':
      state = {
        workspace: null,
        notes: [],
        contents: {},
        categories: [],
        tags: [],
        noteTags: {},
        settings: { ...emptySettings },
        backups: [],
        history: {},
        ai: { ...defaultAiPolicy },
        aiAudit: [],
        research: {},
      };
      persist();
      return null;
    case 'workspace_stats':
      requireWorkspace();
      return computeStats();
    case 'workspace_scan':
    case 'workspace_rebuild_index':
    case 'search_rebuild_index':
      return indexStatus();
    case 'search_get_status':
      return indexStatus();
    case 'settings_get':
      return state.settings;
    case 'settings_update': {
      const input = payload.input as Record<string, unknown>;
      state.settings = { ...state.settings, ...input } as AppSettings;
      persist();
      return state.settings;
    }
    case 'note_list': {
      requireWorkspace();
      return listNotes(payload.query as NoteListQuery);
    }
    case 'note_recent':
      requireWorkspace();
      return listNotes({ pageSize: Number(payload.limit ?? 10) }).items;
    case 'note_list_deleted':
      requireWorkspace();
      return listNotes({ status: 'deleted' }).items;
    case 'note_list_missing':
      return [];
    case 'note_get': {
      const note = state.notes.find((item) => item.id === payload.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在或已被删除。');
      return toDetail(note);
    }
    case 'note_create':
      return createNote(payload.input as CreateNoteInput);
    case 'note_save':
    case 'note_update':
      return saveNote(payload.input as SaveNoteInput);
    case 'note_delete': {
      const note = state.notes.find((item) => item.id === payload.id);
      if (note) {
        note.status = 'deleted';
        persist();
        emit('notes://changed', { kind: 'deleted', noteId: note.id });
      }
      return null;
    }
    case 'note_restore': {
      const note = state.notes.find((item) => item.id === payload.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      note.status = 'active';
      persist();
      return toDetail(note);
    }
    case 'note_purge': {
      state.notes = state.notes.filter((item) => item.id !== payload.id);
      delete state.contents[String(payload.id)];
      delete state.history[String(payload.id)];
      persist();
      return null;
    }
    case 'note_rename': {
      const note = state.notes.find((item) => item.id === payload.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      note.title = String(payload.title);
      note.updatedAt = now();
      persist();
      return toDetail(note);
    }
    case 'note_move': {
      const note = state.notes.find((item) => item.id === payload.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      note.categoryId = (payload.categoryId as string | null) ?? null;
      note.updatedAt = now();
      persist();
      return toDetail(note);
    }
    case 'note_set_parent': {
      const input = (payload.input ?? {}) as SetParentInput;
      const note = state.notes.find((item) => item.id === input.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      const parent = findParentNote(input.parentId);
      if (input.parentId && !parent) fail('INVALID_INPUT', '找不到目标父文档。');
      if (parent && parent.id === note.id) fail('INVALID_INPUT', '不能把笔记设为自己的子文档。');
      if (parent && wouldFormCycle(note.id, parent.id)) {
        fail('INVALID_INPUT', '不能把笔记移动到它自己的子文档下。');
      }
      note.parentId = parent?.id ?? null;
      note.updatedAt = now();
      persist();
      emit('notes://changed', { kind: 'modified', noteId: note.id });
      return toDetail(note);
    }
    case 'note_set_pinned': {
      const note = state.notes.find((item) => item.id === payload.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      note.pinned = Boolean(payload.pinned);
      persist();
      emit('notes://changed', { kind: 'modified', noteId: note.id });
      return toDetail(note);
    }
    case 'notes_reorder': {
      const input = (payload.input ?? {}) as ReorderNotesInput;
      const ids = (input.ids ?? []).filter((id) => typeof id === 'string' && id.length > 0);
      if (ids.length === 0) fail('INVALID_INPUT', '排序列表为空。');
      let updated = 0;
      ids.forEach((id, index) => {
        const note = state.notes.find((item) => item.id === id);
        if (!note) return; // 已删除的笔记跳过
        const order = index + 1;
        if ((note.sortOrder ?? 0) === order) return;
        note.sortOrder = order;
        updated += 1;
      });
      if (updated > 0) {
        persist();
        emit('notes://changed', { kind: 'modified', noteId: ids[0] });
      }
      return updated;
    }
    case 'note_duplicate': {
      const note = state.notes.find((item) => item.id === payload.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      return createNote({
        title: `${note.title} 副本`,
        categoryId: note.categoryId,
        initialContent: state.contents[note.id] ?? '',
        tags: state.noteTags[note.id] ?? [],
        parentId: note.parentId ?? null,
      });
    }
    case 'note_set_favorite': {
      const note = state.notes.find((item) => item.id === payload.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      note.favorite = Boolean(payload.favorite);
      persist();
      return toDetail(note);
    }
    case 'note_set_tags':
    case 'note_set_tag_names': {
      const note = state.notes.find((item) => item.id === payload.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      const tags = (payload.tags as string[]) ?? [];
      state.noteTags[note.id] = tags;
      tags.forEach((name) => {
        if (!state.tags.some((tag) => tag.name === name)) {
          state.tags.push({ id: `tag_${name}`, name, createdAt: now() });
        }
      });
      persist();
      return toDetail(note).tags;
    }
    case 'note_count': {
      const status = (payload.status as string) ?? 'active';
      return state.notes.filter((note) => note.status === status).length;
    }
    case 'note_due_for_review': {
      requireWorkspace();
      const days = Math.min(3650, Math.max(0, Number(payload.days ?? 14)));
      const today = new Date();
      const todayIso = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(
        today.getDate(),
      ).padStart(2, '0')}`;
      return state.notes
        .filter((note) => note.status === 'active' && (note.validUntil ?? '').trim().length > 0)
        .map((note) => {
          const validUntil = (note.validUntil ?? '').trim().slice(0, 10);
          const target = new Date(`${validUntil}T00:00:00`);
          const base = new Date(`${todayIso}T00:00:00`);
          if (Number.isNaN(target.getTime())) return null;
          const overdueDays = Math.round((base.getTime() - target.getTime()) / 86_400_000);
          if (overdueDays < 0 && -overdueDays > days) return null;
          return {
            noteId: note.id,
            title: note.title,
            categoryPath: categoryPath(note.categoryId),
            validUntil: note.validUntil ?? '',
            overdueDays,
            sourceType: note.sourceType ?? null,
            sourceReliability: note.sourceReliability ?? null,
          };
        })
        .filter((item): item is NonNullable<typeof item> => item !== null)
        .sort((left, right) => right.overdueDays - left.overdueDays);
    }
    case 'notes_move_batch': {
      requireWorkspace();
      const input = payload.input as { ids: string[]; categoryId?: string | null };
      const failures: Array<{ noteId: string; message: string }> = [];
      let succeeded = 0;
      new Set(input.ids).forEach((id) => {
        const note = state.notes.find((item) => item.id === id);
        if (!note) {
          failures.push({ noteId: id, message: '笔记不存在。' });
          return;
        }
        note.categoryId = input.categoryId ?? null;
        note.updatedAt = now();
        succeeded += 1;
      });
      persist();
      return { succeeded, failures };
    }
    case 'notes_tag_batch': {
      requireWorkspace();
      const input = payload.input as { ids: string[]; tags: string[]; mode?: string };
      const failures: Array<{ noteId: string; message: string }> = [];
      let succeeded = 0;
      new Set(input.ids).forEach((id) => {
        const note = state.notes.find((item) => item.id === id);
        if (!note) {
          failures.push({ noteId: id, message: '笔记不存在。' });
          return;
        }
        const current = state.noteTags[id] ?? [];
        const next =
          input.mode === 'remove'
            ? current.filter((name) => !input.tags.some((tag) => tag.toLowerCase() === name.toLowerCase()))
            : input.mode === 'replace'
              ? [...input.tags]
              : [...current, ...input.tags.filter((tag) => !current.some((name) => name.toLowerCase() === tag.toLowerCase()))];
        state.noteTags[id] = next;
        next.forEach((name) => {
          if (!state.tags.some((tag) => tag.name === name)) {
            state.tags.push({ id: `tag_${name}`, name, createdAt: now() });
          }
        });
        succeeded += 1;
      });
      persist();
      return { succeeded, failures };
    }
    case 'notes_delete_batch': {
      requireWorkspace();
      const input = payload.input as { ids: string[] };
      const failures: Array<{ noteId: string; message: string }> = [];
      let succeeded = 0;
      new Set(input.ids).forEach((id) => {
        const note = state.notes.find((item) => item.id === id);
        if (!note) {
          failures.push({ noteId: id, message: '笔记不存在。' });
          return;
        }
        note.status = 'deleted';
        succeeded += 1;
      });
      persist();
      emit('notes://changed', { kind: 'deleted' });
      return { succeeded, failures };
    }
    case 'note_import':
      fail('INVALID_INPUT', '浏览器预览模式无法读取本地文件，请在桌面应用中使用「导入外部文件」。');
      break;
    case 'links_get_backlinks':
      requireWorkspace();
      return mockBacklinks(String(payload.noteId ?? ''));
    case 'links_get_outgoing':
      requireWorkspace();
      return mockOutgoing(String(payload.noteId ?? ''));
    case 'links_get_unresolved':
      requireWorkspace();
      return mockUnresolved();
    case 'links_get_counts':
      requireWorkspace();
      return mockCounts(String(payload.noteId ?? ''));
    case 'links_rebuild':
      requireWorkspace();
      return mockRebuild();
    case 'relations_list':
      requireWorkspace();
      return { outgoing: [], incoming: [] };
    case 'relations_add':
    case 'relations_remove':
      fail('INVALID_INPUT', '浏览器预览模式不支持类型化关系（需要写入工作区文件）。');
      break;
    case 'relations_rebuild':
      requireWorkspace();
      return 0;
    case 'graph_get':
      requireWorkspace();
      return mockGraph((payload.query ?? {}) as GraphQuery);
    case 'note_export':
      fail('INVALID_INPUT', '浏览器预览模式不支持导出到本地文件，请在桌面应用中使用「导出」。');
      break;
    case 'attachment_save':
      fail('INVALID_INPUT', '浏览器预览模式无法写入本地文件，请在桌面应用中使用图片粘贴。');
      break;
    case 'attachment_list':
      requireWorkspace();
      return { items: [], totalBytes: 0, referencedCount: 0 };
    case 'attachment_delete':
      fail('INVALID_INPUT', '浏览器预览模式无法删除本地附件，请在桌面应用中使用附件管理。');
      break;
    case 'attachment_references':
      requireWorkspace();
      return [];
    case 'attachment_read':
      fail('INVALID_INPUT', '浏览器预览模式无法读取工作区内的图片。');
      break;
    case 'templates_list':
      return [];
    case 'template_render':
      fail(
        'INVALID_INPUT',
        '浏览器预览模式无法读取 .knowledge/templates，请在桌面应用中使用模板。',
      );
      break;
    case 'note_set_source': {
      requireWorkspace();
      const input = (payload.input ?? {}) as SetSourceInput;
      const note = state.notes.find((item) => item.id === input.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      const trim = (value?: string | null) => {
        const text = (value ?? '').trim();
        return text.length > 0 ? text : null;
      };
      note.sourceType = trim(input.sourceType);
      note.sourceRef = trim(input.sourceRef);
      note.sourceReliability =
        typeof input.sourceReliability === 'number' && input.sourceReliability >= 1
          ? Math.min(5, Math.round(input.sourceReliability))
          : null;
      note.validUntil = trim(input.validUntil);
      note.updatedAt = now();
      persist();
      emit('notes://changed', { kind: 'modified', noteId: note.id });
      return toDetail(note);
    }
    case 'ai_policy_get':
      requireWorkspace();
      return state.ai;
    case 'ai_policy_set': {
      requireWorkspace();
      const input = (payload.input ?? {}) as UpdateAiPolicyInput;
      const next: AiPolicy = { ...state.ai };
      if (typeof input.enabled === 'boolean') next.enabled = input.enabled;
      if (input.network && input.network !== next.network) {
        next.network = input.network;
        next.networkConfirmedAt = null; // 切档位要重新确认（与后端一致）
      }
      if (input.confirmNetwork && next.network === 'allowNetwork') {
        next.networkConfirmedAt = now();
      }
      if (typeof input.allowFetchBody === 'boolean') next.allowFetchBody = input.allowFetchBody;
      if (input.domainAllowlist) next.domainAllowlist = normalizeDomains(input.domainAllowlist);
      if (input.domainDenylist) next.domainDenylist = normalizeDomains(input.domainDenylist);
      if (input.searchEndpoint !== undefined) {
        const endpoint = (input.searchEndpoint ?? '').trim();
        next.searchEndpoint =
          endpoint.length > 0 &&
          /^https?:\/\//i.test(endpoint) &&
          endpoint.includes('{query}')
            ? endpoint
            : null;
      }
      if (input.budget) {
        next.budget = {
          maxSubquestions: Math.min(10, Math.max(1, Math.round(input.budget.maxSubquestions))),
          maxPages: Math.min(50, Math.max(1, Math.round(input.budget.maxPages))),
          maxDurationSecs: Math.min(1800, Math.max(30, Math.round(input.budget.maxDurationSecs))),
        };
      }
      state.ai = next;
      persist();
      return state.ai;
    }
    case 'ai_audit_tail':
      requireWorkspace();
      return [...state.aiAudit].reverse().slice(0, Math.max(1, Number(payload.limit ?? 20)));
    case 'ai_kb_search': {
      requireWorkspace();
      const keyword = String(payload.keyword ?? '').trim();
      if (!keyword) fail('INVALID_INPUT', '检索关键词不能为空。');
      const limit = Math.max(1, Math.min(50, Number(payload.limit ?? 8)));
      const result = listNotes({ keyword, pageSize: limit });
      return result.items.map((item) => ({
        noteId: item.id,
        title: item.title,
        snippet: (item.snippet ?? []).map((segment) => segment.text).join(''),
      }));
    }
    case 'ai_web_fetch': {
      requireWorkspace();
      const gate = checkAiNetworkGate(String(payload.url ?? ''), Boolean(payload.confirmed));
      if (gate) return gate;
      const clipped = mockClip(String(payload.url));
      state.aiAudit = pushAudit(state.aiAudit, 'web_fetch', true, 'ok', null);
      persist();
      return clipped;
    }
    case 'ai_web_search': {
      requireWorkspace();
      if (!state.ai.enabled) fail('AI_DISABLED', 'AI 能力未开启：请到「设置 → AI」打开总开关。');
      if (state.ai.network !== 'allowNetwork') {
        fail('NETWORK_DISABLED', '联网检索当前是「仅本地」：需要到「设置 → AI」切到「允许联网检索」。');
      }
      if (!state.ai.networkConfirmedAt || !payload.confirmed) {
        fail('NETWORK_DISABLED', '本次联网尚未确认：请先确认，本次不会发起任何请求。');
      }
      if (!state.ai.searchEndpoint) {
        fail(
          'INVALID_INPUT',
          '还没有配置联网检索源：请在「设置 → AI → 检索源地址」填入一个返回 JSON 的检索地址（含 {query} 占位符）。',
        );
      }
      state.aiAudit = pushAudit(state.aiAudit, 'web_search', true, 'ok', null);
      persist();
      return mockWebSearch(String(payload.keyword ?? ''), Number(payload.limit ?? 8));
    }
    case 'research_start': {
      requireWorkspace();
      requireAi();
      const input = (payload.input ?? {}) as ResearchRequest;
      const topic = researchValidateTopic(String(input.topic ?? ''));
      const timeWindowDays = Math.min(
        3650,
        Math.max(7, Math.round(input.timeWindowDays ?? 730)),
      );
      const maxSub = Math.min(10, Math.max(1, Math.round(state.ai.budget.maxSubquestions)));
      const subQuestions = researchSubQuestions(topic, maxSub);
      const localBaseline = researchLocalHits(topic, 3).map((note) => ({
        noteId: note.id,
        title: note.title,
        snippet: plainText(state.contents[note.id] ?? '').slice(0, 120),
      }));
      const planNotes = [
        '本次只用本地知识库材料（外部检索属于阶段 D3，接入前不会发起任何请求）。',
        `子问题 ${subQuestions.length} 个 · 时间窗 ${timeWindowDays} 天 · 抓取页上限 ${state.ai.budget.maxPages}（外部检索启用后生效）。`,
        '子问题与关键词由模板生成；语义化的查询改写要等模型接入（N5/D4）。',
        '来源按 §9.3 规则分级：域名内置表 → 内容自证（标题/作者/日期）→ 兜底「弱」。',
      ];
      if (localBaseline.length === 0) {
        planNotes.push('本地知识库暂无相关笔记：本次没有「已知基线」可复用。');
      }
      const createdAt = now();
      const session: ResearchSession = {
        id: `research_${ulid()}`,
        request: {
          topic,
          scope: input.scope ?? null,
          timeWindowDays,
          writeBack: Boolean(input.writeBack),
          targetCategoryId: input.targetCategoryId ?? null,
        },
        status: 'planned',
        createdAt,
        updatedAt: createdAt,
        plan: {
          topic,
          scope: input.scope ?? null,
          subQuestions,
          localBaseline,
          timeWindowDays,
          budget: { ...state.ai.budget },
          notes: planNotes,
        },
        sources: [],
        report: null,
        proposal: null,
        steps: [{ step: 'plan', at: createdAt, detail: '已立项：生成研究计划与本地基线' }],
        elapsedMs: 0,
        abortReason: null,
        writtenNoteId: null,
      };
      auditResearch('research_start', 'ok', null);
      return researchSave(session);
    }
    case 'research_get':
      requireWorkspace();
      return researchRequire(String(payload.sessionId ?? ''));
    case 'research_list': {
      requireWorkspace();
      const limit = Math.max(1, Math.min(200, Number(payload.limit ?? 20)));
      return Object.values(state.research)
        .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
        .slice(0, limit)
        .map<ResearchSessionSummary>((session) => ({
          id: session.id,
          topic: session.request.topic,
          status: session.status,
          createdAt: session.createdAt,
          updatedAt: session.updatedAt,
          sourceCount: session.sources.length,
          writtenNoteId: session.writtenNoteId ?? null,
        }));
    }
    case 'research_collect': {
      requireWorkspace();
      requireAi();
      const session = researchRequire(String(payload.sessionId ?? ''));
      researchRequireStatus(session, ['planned'], '收集材料');
      const plan = session.plan;
      if (!plan) fail('INVALID_INPUT', '会话缺少研究计划。');
      const candidates = researchLocalHits(plan.topic, RESEARCH_MAX_SOURCES * 2);
      const sources: ResearchSource[] = [];
      const perQuestion = plan.subQuestions.map(() => 0);
      const seen: string[] = [];
      candidates.forEach((note) => {
        if (sources.length >= RESEARCH_MAX_SOURCES) return;
        const snippet = plainText(state.contents[note.id] ?? '').slice(0, 160) || note.title;
        const index = researchAssign(note.title, snippet, plan.subQuestions);
        const question = plan.subQuestions[index];
        if (!question || (perQuestion[index] ?? 0) >= RESEARCH_HITS_PER_QUESTION) return;
        const fingerprint = `${question.id}:${note.id}`;
        if (seen.includes(fingerprint)) return;
        seen.push(fingerprint);
        perQuestion[index] = (perQuestion[index] ?? 0) + 1;
        const grade = researchGrade(note);
        sources.push({
          reference: sources.length + 1,
          title: note.title,
          url: note.sourceRef?.startsWith('http') ? note.sourceRef : null,
          origin: 'local',
          noteId: note.id,
          subQuestionId: question.id,
          level: grade.level,
          reliability: RESEARCH_LEVEL_RELIABILITY[grade.level],
          gradeReason: grade.reason,
          publishedAt: null,
          fetchedAt: note.updatedAt,
          applicableVersion: null,
          snippet,
          stale: researchStale(note, plan.timeWindowDays),
        });
      });
      plan.subQuestions.forEach((question, index) => {
        question.localHits = perQuestion[index] ?? 0;
      });
      // 联网（浏览器预览模式为模拟结果，用于验证「确认 → 外部来源 → 分级」链路）
      let webNote =
        state.ai.network === 'allowNetwork'
          ? payload.confirmed
            ? '已允许联网'
            : '联网档位已打开但本次未确认，未发起任何请求'
          : '未开启（联网档位为「仅本地」）';
      let webCount = 0;
      if (state.ai.network === 'allowNetwork' && payload.confirmed) {
        if (!state.ai.searchEndpoint) {
          webNote = '已允许联网，但尚未配置检索源地址：未发起任何请求';
        } else {
          mockWebSearch(plan.topic, Math.min(2, state.ai.budget.maxPages)).forEach((hit) => {
            if (sources.length >= RESEARCH_MAX_SOURCES) return;
            const index = researchAssign(hit.title, hit.snippet, plan.subQuestions);
            const question = plan.subQuestions[index];
            if (!question || (perQuestion[index] ?? 0) >= RESEARCH_HITS_PER_QUESTION) return;
            if (sources.some((source) => source.url === hit.url)) return;
            const grade = mockGradeWeb(hit.url, hit.title, hit.snippet);
            perQuestion[index] = (perQuestion[index] ?? 0) + 1;
            webCount += 1;
            sources.push({
              reference: sources.length + 1,
              title: hit.title,
              url: hit.url,
              origin: 'web',
              noteId: null,
              subQuestionId: question.id,
              level: grade.level,
              reliability: RESEARCH_LEVEL_RELIABILITY[grade.level],
              gradeReason: grade.reason,
              publishedAt: hit.publishedAt ?? null,
              fetchedAt: now(),
              applicableVersion: null,
              snippet: hit.snippet,
              stale: false,
            });
          });
          webNote =
            webCount > 0
              ? `外部来源 ${webCount} 条（浏览器预览为模拟结果，桌面端抓取真实网页）`
              : '联网检索没有返回可用来源';
        }
      }
      session.sources = sources;
      session.status = 'collected';
      researchStep(
        session,
        'collect',
        `本地材料 ${sources.length - webCount} 条；${webNote}`,
      );
      auditResearch(
        'research_collect',
        'ok',
        state.ai.network === 'allowNetwork' ? 'external_not_available' : 'local_only',
      );
      return researchSave(session);
    }
    case 'research_verify': {
      requireWorkspace();
      requireAi();
      const session = researchRequire(String(payload.sessionId ?? ''));
      researchRequireStatus(session, ['collected'], '验证');
      const report = researchBuildReport(session, researchVerifyClaims(session));
      session.report = report;
      session.status = 'verified';
      researchStep(
        session,
        'verify',
        `按 ${session.plan?.subQuestions.length ?? 0} 个子问题汇总：交叉验证 ${report.keyFacts.length} 条，单一来源 ${report.pending.length} 条`,
      );
      auditResearch('research_verify', 'ok', null);
      return researchSave(session);
    }
    case 'research_compose': {
      requireWorkspace();
      requireAi();
      const session = researchRequire(String(payload.sessionId ?? ''));
      researchRequireStatus(session, ['verified'], '生成报告');
      const report = session.report;
      if (!report) fail('INVALID_INPUT', '会话缺少验证结果。');
      if (session.sources.length === 0) {
        report.speculation = ['（模板草稿）本次没有可用材料，无法给出任何推测内容。'];
      } else {
        const speculation = [
          '（模板草稿·未接模型）以下是按材料顺序罗列的待确认方向，**不构成结论**：',
        ];
        session.sources.slice(0, 5).forEach((source, index) => {
          speculation.push(`${index + 1}. ${source.title} —— ${researchFirstSentence(source.snippet)}`);
        });
        speculation.push('下一步需要接入模型（或人工复核）才能把这些方向写成结论。');
        report.speculation = speculation;
      }
      report.generatedAt = now();
      report.markdown = researchRenderMarkdown(session, report);
      session.status = 'composed';
      researchStep(session, 'compose', '已生成报告（模板实现，未接模型）：template.draft');
      auditResearch('research_compose', 'ok', 'template.draft');
      return researchSave(session);
    }
    case 'research_proposal': {
      requireWorkspace();
      requireAi();
      const session = researchRequire(String(payload.sessionId ?? ''));
      researchRequireStatus(session, ['composed', 'proposed'], '生成写入提案');
      if (!session.report) fail('INVALID_INPUT', '会话缺少报告。');
      session.proposal = researchBuildProposal(session, session.report);
      session.status = 'proposed';
      researchStep(session, 'proposal', '已生成写入提案（等待用户确认后落库）');
      auditResearch('research_proposal', 'ok', null);
      return researchSave(session);
    }
    case 'research_write': {
      requireWorkspace();
      requireAi();
      const input = (payload.input ?? {}) as ResearchWriteInput;
      const session = researchRequire(String(input.sessionId ?? ''));
      researchRequireStatus(session, ['proposed'], '写入');
      const proposal = session.proposal;
      if (!proposal) fail('INVALID_INPUT', '会话缺少写入提案。');
      const mode = (input.mode ?? 'create') as ResearchWriteMode;
      if (mode === 'abandon') {
        researchStep(session, 'write', '用户选择放弃本次写入：未落库，材料仍保留为草稿');
        auditResearch('research_write', 'ok', 'abandoned');
        researchSave(session);
        return {
          sessionId: session.id,
          outcome: 'abandoned',
          noteId: null,
          noteTitle: null,
          sourceNoteId: null,
          sourceReliability: proposal.sourceReliability,
          validUntil: proposal.validUntil ?? null,
          diffPreview: '',
        } satisfies ResearchWriteResult;
      }
      if (mode !== 'create' && mode !== 'append') {
        fail('INVALID_INPUT', `不支持的写入方式「${mode}」（可用 create / append / abandon）。`);
      }
      const title = (input.title ?? '').trim() || proposal.title;
      let body = proposal.body;
      if (input.includeSpeculation && (session.report?.speculation.length ?? 0) > 0) {
        body += `\n\n## 附：模型推测（未验证）\n\n> 以下内容由模板实现生成（**未接入模型**），不是结论，仅供参考。\n\n${session.report?.speculation.join('\n') ?? ''}`;
      }
      let noteId: string;
      let noteTitle: string;
      let outcome: 'created' | 'appended';
      let diffPreview: string;
      if (mode === 'append') {
        const target = input.targetNoteId ?? proposal.duplicates[0]?.noteId ?? null;
        if (!target) {
          fail(
            'INVALID_INPUT',
            '追加模式需要指定目标笔记（同标题笔记不存在时请改用「新建」）。',
          );
        }
        const note = state.notes.find((item) => item.id === target && item.status !== 'deleted');
        if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
        const existing = state.contents[note.id] ?? '';
        captureHistory(note.id, existing); // 写前留档 → 可回滚
        const merged = `${existing.trimEnd()}\n\n---\n\n${body}`;
        state.contents[note.id] = merged;
        note.wordCount = plainText(merged).length;
        note.updatedAt = now();
        emit('notes://changed', { kind: 'modified', noteId: note.id });
        noteId = note.id;
        noteTitle = note.title;
        outcome = 'appended';
        diffPreview = [
          ...existing
            .split('\n')
            .filter((line) => line.trim().length > 0)
            .slice(-3)
            .map((line) => ` ${line}`),
          ...body.split('\n').map((line) => `+${line}`),
        ].join('\n');
      } else {
        const created = createNote({
          title,
          categoryId: input.categoryId ?? proposal.categoryId ?? null,
          initialContent: body,
          tags: proposal.tags,
          parentId: null,
        });
        noteId = created.note.id;
        noteTitle = created.note.title;
        outcome = 'created';
        diffPreview = body
          .split('\n')
          .map((line) => `+${line}`)
          .join('\n');
      }
      const written = state.notes.find((item) => item.id === noteId);
      if (written) {
        written.sourceType = proposal.sourceType;
        written.sourceRef = proposal.sourceRef;
        written.sourceReliability = proposal.sourceReliability;
        written.validUntil = proposal.validUntil ?? null;
        persist();
      }
      let sourceNoteId: string | null = null;
      if (input.createSourceNote) {
        const lines = session.sources.map(
          (source) =>
            `- [${source.reference}] ${source.title} · ${RESEARCH_LEVEL_LABEL[source.level]} · 获取 ${
              source.fetchedAt
            } · ${source.url ?? (source.noteId ? `note://${source.noteId}` : '本地')} · ${
              source.gradeReason
            }`,
        );
        const child = createNote({
          title: `来源清单：${session.request.topic}`,
          categoryId: null,
          initialContent: [
            '> 本清单由研究助手生成：每行是报告中引用过的一条来源，含分级依据与获取时间。',
            '',
            `> 资料截至 ${session.report?.dataCutoff ?? researchIsoDate()}。`,
            '',
            ...lines,
          ].join('\n'),
          tags: ['来源清单'],
          parentId: noteId,
        });
        sourceNoteId = child.note.id;
      }
      session.writtenNoteId = noteId;
      session.status = 'written';
      researchStep(
        session,
        'write',
        `已${outcome === 'appended' ? '追加' : '新建'}：${noteTitle}（来源等级 ${
          proposal.sourceReliability
        } · 复核 ${proposal.validUntil ?? '未设置'}）`,
      );
      auditResearch('research_write', 'ok', outcome);
      researchSave(session);
      return {
        sessionId: session.id,
        outcome,
        noteId,
        noteTitle,
        sourceNoteId,
        sourceReliability: proposal.sourceReliability,
        validUntil: proposal.validUntil ?? null,
        diffPreview,
      } satisfies ResearchWriteResult;
    }
    case 'research_abort': {
      requireWorkspace();
      requireAi();
      const session = researchRequire(String(payload.sessionId ?? ''));
      if (session.status !== 'aborted') {
        session.status = 'aborted';
        session.abortReason = String(payload.reason ?? '用户中止').trim() || '用户中止';
        researchStep(session, 'abort', '已中止：已有材料保留为草稿');
        auditResearch('research_abort', 'ok', 'aborted');
      }
      return researchSave(session);
    }
    case 'note_clip_url': {
      const raw = String(payload.url ?? '').trim();
      if (!raw) fail('INVALID_INPUT', '请填写网页地址。');
      const url = raw.startsWith('http://') || raw.startsWith('https://') ? raw : `https://${raw}`;
      if (url.includes(' ')) fail('INVALID_INPUT', '网页地址里不能有空格。');
      if (url.includes('fail.example')) {
        fail('INVALID_INPUT', '抓取失败：无法访问该地址（浏览器预览模式为模拟实现）。');
      }
      // 浏览器预览模式不真的联网：返回一份固定结果，便于验证界面流程
      const markdown = [
        '# 剪藏示例：' + url.replace(/^https?:\/\//, '').split('/')[0],
        '',
        '这是浏览器预览模式下的**模拟剪藏结果**：桌面应用会真实抓取网页，',
        '抽取正文（标题 / 段落 / 列表 / 代码块 / 表格）并转成 Markdown。',
        '',
        '## 抽取能力',
        '',
        '- 标题层级 → Markdown 标题',
        '- 列表 → `- ` / `1. `（嵌套缩进）',
        '- 链接与图片 → 转成绝对地址',
        '',
        '```js',
        "console.log('桌面端才会真的抓取');",
        '```',
      ].join('\n');
      return {
        url: raw,
        finalUrl: url,
        title: `剪藏示例 · ${url.replace(/^https?:\/\//, '').split('/')[0]}`,
        site: url.replace(/^https?:\/\//, '').split('/')[0],
        markdown,
        wordCount: markdown.replace(/\s/g, '').length,
        fetchedAt: now(),
        bytes: markdown.length,
      };
    }
    case 'note_history_list': {
      requireWorkspace();
      const noteId = String(payload.noteId);
      return (state.history[noteId] ?? []).map((entry) => ({
        entryId: entry.entryId,
        savedAt: entry.savedAt,
        label: entry.label ?? null,
        bytes: entry.content.length,
        title: entry.content.match(/^#\s+(.+)$/m)?.[1] ?? '',
        preview: entry.content
          .split('\n')
          .map((line) => line.trim())
          .filter((line) => line && !line.startsWith('#'))
          .join(' ')
          .slice(0, 120),
      }));
    }
    case 'note_history_read': {
      requireWorkspace();
      const entries = state.history[String(payload.noteId)] ?? [];
      const entry = entries.find((item) => item.entryId === payload.entryId);
      if (!entry) fail('NOTE_NOT_FOUND', '该历史版本已不存在。');
      return { entryId: entry.entryId, content: entry.content, body: entry.content };
    }
    case 'note_history_restore': {
      requireWorkspace();
      const noteId = String(payload.noteId);
      const entries = state.history[noteId] ?? [];
      const entry = entries.find((item) => item.entryId === payload.entryId);
      if (!entry) fail('NOTE_NOT_FOUND', '该历史版本已不存在。');
      captureHistory(noteId, state.contents[noteId] ?? '');
      state.contents[noteId] = entry.content;
      const note = state.notes.find((item) => item.id === noteId);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      // 与真实后端一致：还原也算一次内容变更，哈希要变，前端才会重新同步编辑器
      note.contentHash = ulid();
      note.updatedAt = now();
      persist();
      return toDetail(note);
    }
    case 'note_history_save_version': {
      requireWorkspace();
      const noteId = String(payload.noteId);
      const label = String(payload.label ?? '').trim();
      if (!label) fail('INVALID_INPUT', '版本名不能为空。');
      const content = state.contents[noteId];
      if (content === undefined) fail('NOTE_NOT_FOUND', '笔记不存在。');
      const entry = {
        entryId: `${Date.now()}-v${Math.random().toString(36).slice(2, 8)}`,
        savedAt: now(),
        content,
        label,
      };
      // 与后端一致：显式保存不去重，且每篇最多保留 50 份
      state.history[noteId] = [entry, ...(state.history[noteId] ?? [])].slice(0, 50);
      persist();
      return {
        entryId: entry.entryId,
        savedAt: entry.savedAt,
        label: entry.label,
        bytes: entry.content.length,
        title: entry.content.match(/^#\s+(.+)$/m)?.[1] ?? '',
        preview: entry.content
          .split('\n')
          .map((line) => line.trim())
          .filter((line) => line && !line.startsWith('#'))
          .join(' ')
          .slice(0, 120),
      };
    }
    case 'note_history_label': {
      requireWorkspace();
      const noteId = String(payload.noteId);
      const raw = String(payload.label ?? '').trim().slice(0, 60);
      const entries = state.history[noteId] ?? [];
      const entry = entries.find((item) => item.entryId === payload.entryId);
      if (!entry) fail('NOTE_NOT_FOUND', '该历史版本已不存在。');
      if (raw) entry.label = raw;
      else delete entry.label;
      persist();
      return entries.map((item) => ({
        entryId: item.entryId,
        savedAt: item.savedAt,
        label: item.label ?? null,
        bytes: item.content.length,
        title: item.content.match(/^#\s+(.+)$/m)?.[1] ?? '',
        preview: item.content
          .split('\n')
          .map((line) => line.trim())
          .filter((line) => line && !line.startsWith('#'))
          .join(' ')
          .slice(0, 120),
      }));
    }
    case 'note_history_remove': {
      requireWorkspace();
      const noteId = String(payload.noteId);
      const entries = (state.history[noteId] ?? []).filter(
        (item) => item.entryId !== payload.entryId,
      );
      state.history[noteId] = entries;
      persist();
      return entries.length;
    }
    case 'template_save':
      fail(
        'INVALID_INPUT',
        '浏览器预览模式无法写入 .knowledge/templates，请在桌面应用中使用模板。',
      );
      break;
    case 'category_list':
      requireWorkspace();
      return state.categories;
    case 'category_tree':
      requireWorkspace();
      return categoryTree();
    case 'category_create': {
      requireWorkspace();
      const input = payload.input as { name: string; parentId?: string | null };
      const siblings = state.categories.filter(
        (category) => (category.parentId ?? null) === (input.parentId ?? null),
      );
      if (siblings.some((category) => category.name.toLowerCase() === input.name.toLowerCase())) {
        fail('CATEGORY_NAME_CONFLICT', '同级下已存在同名分类。');
      }
      const sortOrder = siblings.length + 1;
      const category: Category = {
        id: `cat_${ulid()}`,
        workspaceId: requireWorkspace().id,
        parentId: input.parentId ?? null,
        name: input.name,
        sortOrder,
        dirName: nextCategoryDirectory(input.name, sortOrder),
        createdAt: now(),
        noteCount: 0,
      };
      state.categories.push(category);
      persist();
      return category;
    }
    case 'category_update': {
      const category = state.categories.find((item) => item.id === payload.id);
      if (!category) fail('CATEGORY_NOT_FOUND', '分类不存在。');
      category.name = String(payload.name);
      category.dirName = nextCategoryDirectory(category.name, category.sortOrder);
      persist();
      return category;
    }
    case 'category_move': {
      const category = state.categories.find((item) => item.id === payload.id);
      if (!category) fail('CATEGORY_NOT_FOUND', '分类不存在。');
      category.parentId = (payload.parentId as string | null) ?? null;
      persist();
      return category;
    }
    case 'category_delete': {
      const input = payload.input as { id: string; strategy: string };
      const ids = new Set([input.id]);
      let added = true;
      while (added) {
        added = false;
        state.categories.forEach((category) => {
          if (category.parentId && ids.has(category.parentId) && !ids.has(category.id)) {
            ids.add(category.id);
            added = true;
          }
        });
      }
      let movedNotes = 0;
      state.notes.forEach((note) => {
        if (note.categoryId && ids.has(note.categoryId)) {
          note.categoryId = input.strategy === 'unassign' ? null : null;
          movedNotes += 1;
        }
      });
      state.categories = state.categories.filter((category) => !ids.has(category.id));
      persist();
      return {
        deletedCategories: ids.size,
        movedNotes,
        unassignedNotes: input.strategy === 'unassign' ? movedNotes : 0,
        warnings: [],
      };
    }
    case 'tag_list':
      requireWorkspace();
      return state.tags.map((tag) => ({
        id: tag.id,
        workspaceId: requireWorkspace().id,
        name: tag.name,
        createdAt: tag.createdAt,
        noteCount: Object.values(state.noteTags).filter((names) => names.includes(tag.name)).length,
      }));
    case 'tag_create': {
      requireWorkspace();
      const name = String(payload.name).trim();
      const existing = state.tags.find((tag) => tag.name.toLowerCase() === name.toLowerCase());
      if (existing) return existing;
      const tag = { id: `tag_${ulid()}`, name, createdAt: now() };
      state.tags.push(tag);
      persist();
      return { ...tag, workspaceId: requireWorkspace().id, noteCount: 0 };
    }
    case 'tag_rename': {
      const tag = state.tags.find((item) => item.id === payload.id);
      if (!tag) fail('TAG_NOT_FOUND', '标签不存在。');
      const oldName = tag.name;
      tag.name = String(payload.name);
      Object.keys(state.noteTags).forEach((noteId) => {
        const names = state.noteTags[noteId] ?? [];
        state.noteTags[noteId] = names.map((name) => (name === oldName ? tag.name : name));
      });
      persist();
      return { ...tag, workspaceId: requireWorkspace().id, noteCount: 0 };
    }
    case 'tag_delete': {
      const tag = state.tags.find((item) => item.id === payload.id);
      if (tag) {
        Object.keys(state.noteTags).forEach((noteId) => {
          const names = state.noteTags[noteId] ?? [];
          state.noteTags[noteId] = names.filter((name) => name !== tag.name);
        });
        state.tags = state.tags.filter((item) => item.id !== payload.id);
        persist();
      }
      return null;
    }
    case 'search_notes':
      requireWorkspace();
      return searchNotes(payload.query as SearchQuery);
    case 'search_quick': {
      requireWorkspace();
      return searchNotes({
        keyword: String(payload.keyword ?? ''),
        pageSize: Number(payload.limit ?? 10),
      }).hits.map((hit) => hit.note);
    }
    case 'backup_list':
      return state.backups;
    case 'backup_directory':
      return '（浏览器预览模式不提供备份）';
    case 'backup_create': {
      const info: BackupInfo = {
        fileName: `KnowledgeBackup_${now().slice(0, 19).replace(/[-:T]/g, '')}.zip`,
        path: '(mock)/backups',
        createdAt: now(),
        sizeBytes: 1024,
        noteCount: state.notes.filter((note) => note.status === 'active').length,
        attachmentCount: 0,
        includesDatabase: true,
      };
      state.backups = [info, ...state.backups];
      persist();
      return info;
    }
    case 'backup_validate':
      return {
        valid: true,
        checks: [{ name: '模拟校验', passed: true, detail: '浏览器预览模式' }],
        noteCount: state.notes.length,
        attachmentCount: 0,
        createdAt: now(),
        workspaceName: state.workspace?.name ?? null,
      };
    case 'backup_restore':
    case 'export_markdown':
      fail('BACKUP_FAILED', '浏览器预览模式不支持备份恢复与导出。');
      break;
    default:
      fail('INTERNAL_ERROR', `未实现的预览命令：${command}`);
  }
}
