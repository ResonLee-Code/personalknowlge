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
  /** 只看顶层文档（不在任何父文档下） */
  rootOnly: boolean;
  selectedId: string | null;
  detail: NoteDetail | null;
  detailLoading: boolean;
  saveStatus: SaveStatus;
  saveError: string | null;
  conflict: { noteId: string; currentContent: string } | null;
  setScope: (scope: NoteScope) => Promise<void>;
  setSort: (sort: NoteSort) => Promise<void>;
  /** 从工作区设置恢复排序（不写回设置，避免自我触发） */
  hydrateSort: (sort: NoteSort) => void;
  setKeyword: (keyword: string) => Promise<void>;
  setRootOnly: (rootOnly: boolean) => Promise<void>;
  refresh: () => Promise<void>;
  select: (id: string | null) => Promise<void>;
  openDetail: (id: string) => Promise<void>;
  create: (input: CreateNoteInput) => Promise<NoteDetail | null>;
  save: (input: { id: string; content: string; baseContentHash?: string | null; title?: string; categoryId?: string | null; tags?: string[] }) => Promise<boolean>;
  markDirty: () => void;
  remove: (id: string) => Promise<boolean>;
  restore: (id: string) => Promise<boolean>;
  purge: (id: string) => Promise<boolean>;
  rename: (id: string, title: string, renameFile?: boolean) => Promise<void>;
  move: (id: string, categoryId: string | null) => Promise<void>;
  duplicate: (id: string) => Promise<boolean>;
  /** 设置父文档（子文档层级）；`parentId = null` 移到顶层 */
  setParent: (id: string, parentId: string | null) => Promise<boolean>;
  /** 手动排序：按传入顺序改写同层笔记的顺序（配合「自定义顺序」排序） */
  reorder: (ids: string[]) => Promise<boolean>;
  toggleFavorite: (id: string) => Promise<boolean>;
  /** 置顶 / 取消置顶 */
  setPinned: (id: string, pinned: boolean) => Promise<boolean>;
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
  /** 批量复制到分类（原作保留） */
  copyManyTo: (categoryId: string | null) => Promise<void>;
  /** 单篇复制到指定分类（原作保留；标题带「副本」后缀） */
  copyTo: (id: string, categoryId: string | null) => Promise<boolean>;
  /** 拖拽等场景：直接给定 ids 移动 */
  moveNotes: (ids: string[], categoryId: string | null) => Promise<void>;
  tagMany: (tags: string[], mode?: 'add' | 'remove' | 'replace') => Promise<void>;
  deleteMany: () => Promise<void>;
}

/** 批量操作的统一反馈：成功数 + 失败条数（失败原因取第一条）。 */
function toastBatch(
  report: BatchReport,
  requested: number,
  verb: string,
  undo?: { label: string; run: () => void },
) {
  if (report.failures.length === 0) {
    toast.success(`${verb} ${report.succeeded} 篇笔记`, {
      action: undo ? { label: undo.label, onClick: undo.run } : undefined,
    });
    return;
  }
  const first = report.failures[0];
  toast.warning(
    `${verb} ${report.succeeded}/${requested} 篇，${report.failures.length} 篇失败：${first?.message ?? ''}`,
    { action: undo ? { label: undo.label, onClick: undo.run } : undefined },
  );
}

function queryFromScope(
  scope: NoteScope,
  sort: NoteSort,
  keyword: string,
  rootOnly: boolean,
): NoteListQuery {
  const base: NoteListQuery = {
    sort,
    pageSize: 300,
    keyword: keyword.trim() ? keyword.trim() : null,
    status: 'active' as NoteStatus,
    rootOnly,
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
  rootOnly: false,
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
    // 排序是视图偏好，记到工作区设置里：重开应用后仍是上次的选择
    // （「自定义顺序」尤其需要，否则拖出来的顺序下次打开就看不到了）
    void useWorkspaceStore.getState().updateSettings({ listSort: sort });
    await get().refresh();
  },

  hydrateSort: (sort) => {
    if (get().sort === sort) return;
    set({ sort });
    void get().refresh();
  },

  setKeyword: async (keyword) => {
    set({ keyword });
    await get().refresh();
  },

  setRootOnly: async (rootOnly) => {
    set({ rootOnly });
    await get().refresh();
  },

  refresh: async () => {
    const { scope, sort, keyword, rootOnly } = get();
    set({ loading: true, error: null });
    try {
      const result = await noteService.list(queryFromScope(scope, sort, keyword, rootOnly));
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
      return false;
    }
    return true;
  },

  restore: async (id) => {
    try {
      await noteService.restore(id);
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    }
    return true;
  },

  purge: async (id) => {
    try {
      await noteService.purge(id);
      if (get().selectedId === id) set({ selectedId: null, detail: null });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    }
    return true;
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
      return false;
    }
    return true;
  },

  setParent: async (id, parentId) => {
    try {
      const detail = await noteService.setParent({ id, parentId });
      set({ detail: get().selectedId === id ? detail : get().detail, error: null });
      await get().refresh();
      return true;
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    }
  },

  reorder: async (ids) => {
    if (ids.length === 0) return true;
    try {
      await noteService.reorder({ ids });
      set({ error: null });
      await get().refresh();
      return true;
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    }
  },

  setPinned: async (id, pinned) => {
    try {
      const detail = await noteService.setPinned(id, pinned);
      set({ detail: get().selectedId === id ? detail : get().detail });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    }
    return true;
  },

  toggleFavorite: async (id) => {
    const current = get().items.find((item) => item.id === id);
    try {
      const detail = await noteService.setFavorite(id, !(current?.favorite ?? false));
      set({ detail: get().selectedId === id ? detail : get().detail });
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    }
    return true;
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

  copyTo: async (id, categoryId) => {
    try {
      const detail = await noteService.duplicate(id);
      if (categoryId) await noteService.move(detail.note.id, categoryId);
      set({ error: null });
      await get().refresh();
      await useCategoriesStore.getState().refresh();
      return true;
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    }
  },

  copyManyTo: async (categoryId) => {
    const ids = get().selection;
    if (ids.length === 0) return;
    let copied = 0;
    const failures: string[] = [];
    for (const id of ids) {
      try {
        // 复制语义：先 duplicating 出一份（保留正文/标签/父文档），再移动到目标分类
        const detail = await noteService.duplicate(id);
        if (categoryId) await noteService.move(detail.note.id, categoryId);
        copied += 1;
      } catch (error) {
        failures.push(errorMessage(error));
      }
    }
    set({ selection: [], selectionMode: false });
    await get().refresh();
    if (failures.length === 0) {
      toast.success(`已复制 ${copied} 篇笔记（原作保留）`);
    } else {
      toast.warning(`复制 ${copied}/${ids.length} 篇，${failures.length} 篇失败：${failures[0] ?? ''}`);
    }
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
      const failed = new Set(report.failures.map((item) => item.noteId));
      const removed = ids.filter((id) => !failed.has(id));
      // 对标语雀：批量删除后可随即撤销（逐篇恢复，回收站本身就是软删除）
      toastBatch(report, ids.length, '已移入回收站', {
        label: '撤销',
        run: () =>
          void (async () => {
            let restored = 0;
            let firstError: string | null = null;
            for (const id of removed) {
              try {
                await noteService.restore(id);
                restored += 1;
              } catch (error) {
                firstError ??= errorMessage(error);
              }
            }
            await get().refresh();
            // 失败必须如实说：不能把「恢复了一半」报成全部成功
            if (restored === removed.length) {
              toast.success(`已恢复 ${restored} 篇笔记`);
            } else {
              toast.error(
                `已恢复 ${restored}/${removed.length} 篇，${
                  removed.length - restored
                } 篇失败${firstError ? `：${firstError}` : ''}`,
              );
            }
          })(),
      });
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
      rootOnly: false,
    }),
}));
