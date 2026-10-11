import { create } from 'zustand';

import { errorMessage } from '@/lib/format';
import { createRequestGuard } from '@/lib/latestRequest';
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
  rebuildIndex: () => Promise<boolean>;
}

/** 「最后发起的检索才算数」守卫：连续输入时旧结果不会覆盖新结果 */
const searchGuard = createRequestGuard();

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
    const isLatest = searchGuard.begin();
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
      if (!isLatest()) return; // 又发起了新检索（继续输入 / 改筛选），丢弃这次结果
      set({ response });
    } catch (error) {
      if (!isLatest()) return;
      set({ error: errorMessage(error), response: null });
    } finally {
      if (isLatest()) set({ loading: false });
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
      set({ status, error: null });
      await get().run();
    } catch (error) {
      set({ error: errorMessage(error), loading: false });
      return false;
    } finally {
      set({ loading: false });
    }
    return true;
  },
}));
