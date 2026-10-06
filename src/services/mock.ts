/**
 * 浏览器开发模式下的内存后端。
 *
 * 仅在未运行于 Tauri 外壳时启用（`pnpm dev` 直接打开页面即可预览界面）。
 * 数据保存在 localStorage，便于反复调试界面。
 * 该实现对真正的工作区没有副作用。
 */

import type {
  AppError,
  AppSettings,
  BackupInfo,
  Category,
  CategoryNode,
  CreateNoteInput,
  IndexStatus,
  Note,
  NoteDetail,
  NoteListQuery,
  NoteSummary,
  SaveNoteInput,
  SearchQuery,
  TextSegment,
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
    scan: { added: 0, updated: 0, missing: 0, unchanged: state.notes.length, durationMs: 1, warnings: [] },
  };
}

function computeStats() {
  const active = state.notes.filter((note) => note.status === 'active');
  return {
    noteCount: active.length,
    inboxCount: active.filter((note) => !note.categoryId).length,
    trashCount: state.notes.filter((note) => note.status === 'deleted').length,
    categoryCount: state.categories.length,
    tagCount: state.tags.length,
    attachmentCount: 0,
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
  if (query.favoriteOnly) items = items.filter((note) => note.favorite);
  if (query.tagId) items = items.filter((note) => (state.noteTags[note.id] ?? []).includes(query.tagId!));
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
    if (sort === 'titleAsc') return a.title.localeCompare(b.title, 'zh-CN');
    if (sort === 'createdDesc') return b.createdAt.localeCompare(a.createdAt);
    if (sort === 'updatedAsc') return a.updatedAt.localeCompare(b.updatedAt);
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
  const createdAt = now();
  const id = `note_${ulid()}`;
  const title = input.title.trim() || '未命名笔记';
  const filePath = `00_Inbox/${createdAt.slice(0, 19).replace(/[-:T]/g, '')}-${title.replace(/\s+/g, '-')}.md`;
  const note: Note = {
    id,
    workspaceId: workspace.id,
    title,
    filePath,
    categoryId: input.categoryId ?? null,
    status: 'active',
    contentHash: ulid(),
    wordCount: plainText(input.initialContent ?? '').length,
    favorite: false,
    createdAt,
    updatedAt: createdAt,
    lastIndexedAt: createdAt,
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

function saveNote(input: SaveNoteInput) {
  const note = state.notes.find((item) => item.id === input.id);
  if (!note) fail('NOTE_NOT_FOUND', '笔记不存在或已被删除。');
  if (input.baseContentHash && input.baseContentHash !== note.contentHash) {
    fail('NOTE_CONFLICT', '文件已被其他程序修改，请选择重新加载或覆盖保存。', {
      currentContent: state.contents[note.id] ?? '',
    });
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
  if (query.tagId) notes = notes.filter((note) => (state.noteTags[note.id] ?? []).includes(query.tagId!));
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
      return openWorkspace(String(payload.name ?? '预览知识库'), String(payload.root ?? '~/MockKnowledge'));
    case 'workspace_switch':
      return openWorkspace(state.workspace?.name ?? '预览知识库', state.workspace?.rootPath ?? '~/MockKnowledge');
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
    case 'note_duplicate': {
      const note = state.notes.find((item) => item.id === payload.id);
      if (!note) fail('NOTE_NOT_FOUND', '笔记不存在。');
      return createNote({
        title: `${note.title} 副本`,
        categoryId: note.categoryId,
        initialContent: state.contents[note.id] ?? '',
        tags: state.noteTags[note.id] ?? [],
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
    case 'note_import':
      fail(
        'INVALID_INPUT',
        '浏览器预览模式无法读取本地文件，请在桌面应用中使用「导入外部文件」。',
      );
      break;
    case 'note_export':
      fail(
        'INVALID_INPUT',
        '浏览器预览模式不支持导出到本地文件，请在桌面应用中使用「导出」。',
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
