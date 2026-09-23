import type { Category, CategoryDeleteReport, CategoryNode } from '@/types';

import { call } from './api';

export const categoryService = {
  list: () => call<Category[]>('category_list'),

  tree: () => call<CategoryNode[]>('category_tree'),

  create: (name: string, parentId?: string | null, createDirectory = true) =>
    call<Category>('category_create', {
      input: { name, parentId: parentId ?? null, createDirectory },
    }),

  rename: (id: string, name: string) => call<Category>('category_update', { id, name }),

  move: (id: string, parentId: string | null) =>
    call<Category>('category_move', { id, parentId }),

  remove: (id: string, strategy: 'moveToInbox' | 'moveToParent' | 'unassign') =>
    call<CategoryDeleteReport>('category_delete', { input: { id, strategy } }),
};
