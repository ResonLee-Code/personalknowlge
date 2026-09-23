import { create } from 'zustand';

import { errorMessage } from '@/lib/format';
import { searchService } from '@/services/searchService';
import type { IndexStatus, SearchQuery, SearchResponse, SearchSort } from '@/types';

interface SearchState {
  keyword: string;
  categoryId: string | null;
  tagId: string | null;
  favoriteOnly: boolean;
  updatedAfter: string | null;
  sort: SearchSort;
  page: number;
  pageSize: number;
  response: SearchResponse | null;
  status: IndexStatus | null;
  loading: boolean;
  error: string | null;
  setKeyword: (keyword: string) => void;
  setFilters: (filters: Partial<Pick<SearchState, 'categoryId' | 'tagId' | 'favoriteOnly' | 'updatedAfter' | 'sort'>>) => void;
  setPage: (page: number) => void;
  resetFilters: () => void;
  run: () => Promise<void>;
  refreshStatus: () => Promise<void>;
  rebuildIndex: () => Promise<void>;
}

export const useSearchStore = create<SearchState>((set, get) => ({
  keyword: '',
  categoryId: null,
  tagId: null,
  favoriteOnly: false,
  updatedAfter: null,
  sort: 'relevance',
  page: 1,
  pageSize: 50,
  response: null,
  status: null,
  loading: false,
  error: null,

  setKeyword: (keyword) => set({ keyword, page: 1 }),

  setFilters: (filters) => set({ ...filters, page: 1 }),

  setPage: (page) => {
    set({ page });
    void get().run();
  },

  resetFilters: () =>
    set({
      categoryId: null,
      tagId: null,
      favoriteOnly: false,
      updatedAfter: null,
      sort: 'relevance',
      page: 1,
    }),

  run: async () => {
    const { keyword, categoryId, tagId, favoriteOnly, updatedAfter, sort, page, pageSize } = get();
    set({ loading: true, error: null });
    const query: SearchQuery = {
      keyword,
      categoryId,
      tagId,
      favoriteOnly,
      updatedAfter,
      sort,
      page,
      pageSize,
    };
    try {
      const response = await searchService.search(query);
      set({ response });
    } catch (error) {
      set({ error: errorMessage(error), response: null });
    } finally {
      set({ loading: false });
    }
  },

  refreshStatus: async () => {
    try {
      const status = await searchService.status();
      set({ status });
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  rebuildIndex: async () => {
    set({ loading: true });
    try {
      const status = await searchService.rebuildIndex();
      set({ status });
      await get().run();
    } catch (error) {
      set({ error: errorMessage(error) });
    } finally {
      set({ loading: false });
    }
  },
}));
