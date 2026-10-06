import type {
  Backlink,
  LinkCounts,
  LinkRebuildReport,
  OutgoingLink,
  UnresolvedLink,
} from '@/types';

import { call } from './api';

export const linkService = {
  /** 谁引用了这篇笔记 */
  backlinks: (noteId: string) => call<Backlink[]>('links_get_backlinks', { noteId }),

  /** 这篇笔记引用了谁（含未解析与外链） */
  outgoing: (noteId: string) => call<OutgoingLink[]>('links_get_outgoing', { noteId }),

  /** 全库未解析链接 */
  unresolved: () => call<UnresolvedLink[]>('links_get_unresolved'),

  counts: (noteId: string) => call<LinkCounts>('links_get_counts', { noteId }),

  rebuild: () => call<LinkRebuildReport>('links_rebuild'),
};
