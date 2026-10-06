import type { AddRelationInput, RelationView } from '@/types';

import { call } from './api';

/** 类型化关系（N2）：事实来源是来源笔记 frontmatter 的 relations: 字段。 */
export const relationService = {
  list: (noteId: string) => call<RelationView>('relations_list', { noteId }),

  add: (input: AddRelationInput) => call<RelationView>('relations_add', { input }),

  remove: (id: string) => call<RelationView>('relations_remove', { id }),

  /** 从正文重建派生关系表 */
  rebuild: () => call<number>('relations_rebuild'),
};

export type { AddRelationInput, RelationView };
