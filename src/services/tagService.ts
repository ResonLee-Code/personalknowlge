import type { Tag } from '@/types';

import { call } from './api';

export const tagService = {
  list: () => call<Tag[]>('tag_list'),

  create: (name: string) => call<Tag>('tag_create', { name }),

  rename: (id: string, name: string) => call<Tag>('tag_rename', { id, name }),

  remove: (id: string) => call<void>('tag_delete', { id }),

  setNoteTags: (id: string, tags: string[]) => call<Tag[]>('note_set_tag_names', { id, tags }),
};
