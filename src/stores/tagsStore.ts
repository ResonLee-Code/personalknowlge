import { create } from 'zustand';

import { errorMessage } from '@/lib/format';
import { tagService } from '@/services/tagService';
import type { Tag } from '@/types';

interface TagsState {
  tags: Tag[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  create: (name: string) => Promise<Tag | null>;
  rename: (id: string, name: string) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

export const useTagsStore = create<TagsState>((set, get) => ({
  tags: [],
  loading: false,
  error: null,

  refresh: async () => {
    set({ loading: true });
    try {
      const tags = await tagService.list();
      set({ tags, error: null });
    } catch (error) {
      set({ error: errorMessage(error), tags: [] });
    } finally {
      set({ loading: false });
    }
  },

  create: async (name) => {
    try {
      const tag = await tagService.create(name);
      await get().refresh();
      return tag;
    } catch (error) {
      set({ error: errorMessage(error) });
      return null;
    }
  },

  rename: async (id, name) => {
    try {
      await tagService.rename(id, name);
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  remove: async (id) => {
    try {
      await tagService.remove(id);
      await get().refresh();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },
}));
