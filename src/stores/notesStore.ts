import { toast } from 'sonner';
import { create } from 'zustand';

import { errorMessage } from '@/lib/format';
import { noteService } from '@/services/noteService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type {
  BatchReport,
  CreateNoteInput,
  NoteDetail,
  NoteListQuery,
  NoteSort,
  NoteStatus,
  NoteSummary,
} from '@/types';

/** 当前列表范围（侧边栏导航决定）。 */
export type NoteScope =
  | { kind: 'all' }
  | { kind: 'inbox' }
  | { kind: 'category'; id: string }
  | { kind: 'tag'; id: string }
  | { kind: 'favorites' }
  | { kind: 'trash' }
  | { kind: 'missing' };

export type SaveStatus = 'idle' | 'dirty' | 'saving' | 'saved' | 'error';

interface NotesState {
  items: NoteSummary[];
  total: number;
  loading: boolean;
  error: string | null;
  scope: NoteScope;
  sort: NoteSort;
  keyword: string;
  selectedId: string | null;
  detail: NoteDetail | null;
  detailLoading: boolean;
  saveStatus: SaveStatus;
  saveError: string | null;
  conflict: { noteId: string; currentContent: string } | null;
  setScope: (scope: NoteScope) => Promise<void>;
  setSort: (sort: NoteSort) => Promise<void>;
  setKeyword: (keyword: string) => Promise<void>;
  refresh: () => Promise<void>;
  select: (id: string | null) => Promise<void>;
  openDetail: (id: string) => Promise<void>;
  create: (input: CreateNoteInput) => Promise<NoteDetail | null>;
  save: (input: { id: string; content: string; baseContentHash?: string | null; title?: string; categoryId?: string | null; tags?: string[] }) => Promise<boolean>;
  markDirty: () => void;
  remove: (id: string) => Promise<void>;
  restore: (id: string) => Promise<void>;
  purge: (id: string) => Promise<void>;
  rename: (id: string, title: string, renameFile?: boolean) => Promise<void>;
  move: (id: string, categoryId: string | null) => Promise<void>;
  duplicate: (id: string) => Promise<void>;
  toggleFavorite: (id: string) => Promise<void>;
  setTags: (id: string, tags: string[]) => Promise<void>;
  clearConflict: () => void;
  reset: () => void;
  /** 多选（批量操作） */
  selectionMode: boolean;
  selection: string[];
  setSelectionMode: (enabled: boolean) => void;
  toggleSelected: (id: string) => void;
  setSelection: (ids: string[]) => void;
  clearSelection: () => void;
  moveMany: (categoryId: string | null) => Promise<void>;
  /** 拖拽等场景：直接给定 ids 移动 */
  moveNotes: (ids: string[], categoryId: string | null) => Promise<void>;
  tagMany: (tags: string[], mode?: 'add' | 'remove' | 'replace') => Promise<void>;
  deleteMany: () => Promise<void>;
}

/** 批量操作的统一反馈：成功数 + 失败条数（失败原因取第一条）。 */
function toastBatch(report: BatchReport, requested: number, verb: string) {
  if (report.failures.length === 0) {
    toast.success(`${verb} ${report.succeeded} 篇笔记`);
    return;
  }
  const first = report.failures[0];
  toast.warning(
    `${verb} ${report.succeeded}/${requested} 篇，${report.failures.length} 篇失败：${first?.message ?? ''}`,
  );
}

function queryFromScope(scope: NoteScope, sort: NoteSort, keyword: string): NoteListQuery {
  const base: NoteListQuery = {
    sort,
    pageSize: 300,
    keyword: keyword.trim() ? keyword.trim() : null,
    status: 'active' as NoteStatus,
  };
  switch (scope.kind) {
    case 'inbox':
      return { ...base, inboxOnly: true };
    case 'category':
      return { ...base, categoryId: scope.id };
    case 'tag':
      return { ...base, tagId: scope.id };
    case 'favorites':
      return { ...base, favoriteOnly: true };
    case 'trash':
      return { ...base, status: 'deleted' as NoteStatus, inboxOnly: false };
    case 'missing':
      return { ...base, status: 'missing' as NoteStatus };
    default:
      return base;
  }
}

export const useNotesStore = create<NotesState>((set, get) => ({
  items: [],
  total: 0,
  loading: false,
  error: null,
  scope: { kind: 'all' },
  sort: 'updatedDesc',
  keyword: '',
  selectedId: null,
  detail: null,
  detailLoading: false,
  saveStatus: 'idle',
  saveError: null,
  conflict: null,
  selectionMode: false,
  selection: [],

  setScope: async (scope) => {
    set({ scope, selectedId: null, detail: null });
    await get().refresh();
  },

  setSort: async (sort) => {
    set({ sort });
    await get().refresh();
  },

  setKeyword: async (keyword) => {
    set({ keyword });
    await get().refresh();
  },

  refresh: async () => {
    const { scope, sort, keyword } = get();
    set({ loading: true, error: null });
    try {
      const result = await noteService.list(queryFromScope(scope, sort, keyword));
      set({ items: result.items, total: result.total });
    } catch (error) {
      set({ error: errorMessage(error), items: [], total: 0 });
    } finally {
      set({ loading: false });
    }
  },

  select: async (id) => {
    set({ selectedId: id });
    if (id) await get().openDetail(id);
    else set({ detail: null });
  },

  openDetail: async (id) => {
    set({ detailLoading: true });
    try {
      const detail = await noteService.get(id);
      set({ detail, selectedId: id, saveStatus: 'idle', saveError: null, conflict: null });
    } catch (error) {
      set({ error: errorMessage(error), detail: null });
    } finally {
      set({ detailLoading: false });
    }
  },

  create: async (input) => {
    try {
      const detail = await noteService.create(input);
      await get().refresh();
      set({ detail, selectedId: detail.note.id, saveStatus: 'idle' });
      return detail;
    } catch (error) {
      set({ error: errorMessage(error) });
      return null;
    }
  },

  save: async (input) => {
    set({ saveStatus: 'saving', saveError: null });
    try {
      const result = await noteService.save({
        id: input.id,
        content: input.content,
        baseContentHash: input.baseContentHash ?? null,
        title: input.title ?? null,
        categoryId: input.categoryId ?? null,
        tags: input.tags ?? null,
      });
      const detail = await noteService.get(input.id);
      set({
        detail,
        saveStatus: 'saved',
        saveError: result.indexWarning ?? null,
        conflict: null,
      });
      await get().refresh();
      return true;
    } catch (error) {
      const code = (error as { code?: string }).code;
      if (code === 'NOTE_CONFLICT') {
        const details = (error as { details?: { currentContent?: string } }).details;
        set({
          saveStatus: 'error',
          saveError: errorMessage(error),
          conflict: { noteId: input.id, currentContent: details?.currentContent ?? '' },
        });
      } else {
        set({ saveStatus: 'error', saveError: errorMessage(error) });
      }
      return false;
    }
  },

  markDirty: () => set((state) => ({ saveStatus: state.saveStatus === 'saving' ? 'saving' : 'dirty' })),

  remove: async (id) => {
    try {
      await noteService.remove(id);
      if (get().selectedId === id) set({ selectedId: null, detail: null });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  restore: async (id) => {
    try {
      await noteService.restore(id);
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  purge: async (id) => {
    try {
      await noteService.purge(id);
      if (get().selectedId === id) set({ selectedId: null, detail: null });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  rename: async (id, title, renameFile = true) => {
    try {
      const detail = await noteService.rename(id, title, renameFile);
      set({ detail, saveStatus: 'saved' });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  move: async (id, categoryId) => {
    try {
      const detail = await noteService.move(id, categoryId);
      set({ detail });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  duplicate: async (id) => {
    try {
      const detail = await noteService.duplicate(id);
      await get().refresh();
      set({ detail, selectedId: detail.note.id });
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  toggleFavorite: async (id) => {
    const current = get().items.find((item) => item.id === id);
    try {
      const detail = await noteService.setFavorite(id, !(current?.favorite ?? false));
      set({ detail: get().selectedId === id ? detail : get().detail });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  setTags: async (id, tags) => {
    try {
      const detail = await noteService.setTags(id, tags);
      set({ detail, saveStatus: 'saved' });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  clearConflict: () => set({ conflict: null, saveStatus: 'idle', saveError: null }),

  setSelectionMode: (enabled) => set({ selectionMode: enabled, selection: [] }),

  toggleSelected: (id) =>
    set((state) => ({
      selection: state.selection.includes(id)
        ? state.selection.filter((item) => item !== id)
        : [...state.selection, id],
    })),

  setSelection: (ids) => set({ selection: ids }),

  clearSelection: () => set({ selection: [] }),

  moveMany: async (categoryId) => {
    const ids = get().selection;
    if (ids.length === 0) return;
    await get().moveNotes(ids, categoryId);
    set({ selection: [], selectionMode: false });
  },

  moveNotes: async (ids, categoryId) => {
    if (ids.length === 0) return;
    try {
      const report = await noteService.moveMany(ids, categoryId);
      toastBatch(report, ids.length, '已移动');
      await get().refresh();
      await useCategoriesStore.getState().refresh();
      await useWorkspaceStore.getState().refreshStats();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  tagMany: async (tags, mode = 'add') => {
    const ids = get().selection;
    if (ids.length === 0) return;
    try {
      const report = await noteService.tagMany(ids, tags, mode);
      toastBatch(report, ids.length, mode === 'remove' ? '已移除标签' : '已打标签');
      set({ selection: [], selectionMode: false });
      await get().refresh();
      await useTagsStore.getState().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  deleteMany: async () => {
    const ids = get().selection;
    if (ids.length === 0) return;
    try {
      const report = await noteService.deleteMany(ids);
      toastBatch(report, ids.length, '已移入回收站');
      set({ selection: [], selectionMode: false, detail: null, selectedId: null });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  reset: () =>
    set({
      items: [],
      total: 0,
      scope: { kind: 'all' },
      selectedId: null,
      detail: null,
      saveStatus: 'idle',
      saveError: null,
      conflict: null,
      keyword: '',
    }),
}));
