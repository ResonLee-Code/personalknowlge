/**
 * 浏览器开发模式下的内存后端。
 *
 * 仅在未运行于 Tauri 外壳时启用（`pnpm dev` 直接打开页面即可预览界面）。
 * 数据保存在 localStorage，便于反复调试界面。
 * 该实现对真正的工作区没有副作用。
 */

import { parseWikilinks, type WikiLinkToken } from '@/lib/wikilink';
import type {
  AppError,
  AppSettings,
  Backlink,
  BackupInfo,
  Category,
  CategoryNode,
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
  SaveNoteInput,
  ReorderNotesInput,
  SearchQuery,
  SetParentInput,
  TextSegment,
  UnresolvedLink,
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
}

interface MockHistoryEntry {
  entryId: string;
  savedAt: string;
  content: string;
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

let state: MockState = load();

function load(): MockState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as MockState;
      return { ...parsed, settings: { ...emptySettings, ...parsed.settings } };
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
    case 'note_history_list': {
      requireWorkspace();
      const noteId = String(payload.noteId);
      return (state.history[noteId] ?? []).map((entry) => ({
        entryId: entry.entryId,
        savedAt: entry.savedAt,
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
