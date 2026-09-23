import type { IndexStatus, NoteSummary, SearchQuery, SearchResponse } from '@/types';

import { call } from './api';

export const searchService = {
  search: (query: SearchQuery) => call<SearchResponse>('search_notes', { query }),

  quick: (keyword: string, limit = 10) =>
    call<NoteSummary[]>('search_quick', { keyword, limit }),

  rebuildIndex: () => call<IndexStatus>('search_rebuild_index'),

  status: () => call<IndexStatus>('search_get_status'),
};
