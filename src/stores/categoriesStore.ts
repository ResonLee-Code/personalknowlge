import { create } from 'zustand';

import { errorMessage } from '@/lib/format';
import { categoryService } from '@/services/categoryService';
import type { Category, CategoryDeleteReport, CategoryNode, CategoryDeleteStrategy } from '@/types';

import { useWorkspaceStore } from './workspaceStore';
import { useNotesStore } from './notesStore';

interface CategoriesState {
  tree: CategoryNode[];
  flat: Category[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  create: (name: string, parentId?: string | null, createDirectory?: boolean) => Promise<Category | null>;
  rename: (id: string, name: string) => Promise<boolean>;
  move: (id: string, parentId: string | null) => Promise<void>;
  remove: (id: string, strategy: CategoryDeleteStrategy) => Promise<CategoryDeleteReport | null>;
  pathOf: (id?: string | null) => string;
}

export const useCategoriesStore = create<CategoriesState>((set, get) => ({
  tree: [],
  flat: [],
  loading: false,
  error: null,

  refresh: async () => {
    set({ loading: true });
    try {
      const [tree, flat] = await Promise.all([
        categoryService.tree(),
        categoryService.list(),
      ]);
      set({ tree, flat, error: null });
    } catch (error) {
      set({ error: errorMessage(error), tree: [], flat: [] });
    } finally {
      set({ loading: false });
    }
  },

  create: async (name, parentId = null, createDirectory = true) => {
    try {
      const category = await categoryService.create(name, parentId, createDirectory);
      await get().refresh();
      await useWorkspaceStore.getState().refreshStats();
      return category;
    } catch (error) {
      set({ error: errorMessage(error) });
      return null;
    }
  },

  rename: async (id, name) => {
    try {
      await categoryService.rename(id, name);
      await get().refresh();
      await useWorkspaceStore.getState().refreshStats();
      return true;
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    }
  },

  move: async (id, parentId) => {
    try {
      await categoryService.move(id, parentId);
      await get().refresh();
      await useWorkspaceStore.getState().refreshStats();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  remove: async (id, strategy) => {
    try {
      const report = await categoryService.remove(id, strategy);
      await get().refresh();
      await useWorkspaceStore.getState().refreshStats();
      await useNotesStore.getState().refresh();
      return report;
    } catch (error) {
      set({ error: errorMessage(error) });
      return null;
    }
  },

  pathOf: (id) => {
    if (!id) return '';
    const parts: string[] = [];
    let current = get().flat.find((category) => category.id === id);
    let guard = 0;
    while (current && guard < 20) {
      parts.unshift(current.name);
      current = current.parentId
        ? get().flat.find((category) => category.id === current?.parentId)
        : undefined;
      guard += 1;
    }
    return parts.join('/');
  },
}));
