import type {
  CreateNoteInput,
  NoteDetail,
  NoteListQuery,
  NoteListResult,
  NoteSummary,
  SaveNoteInput,
  SaveNoteResult,
} from '@/types';

import { call } from './api';

export const noteService = {
  create: (input: CreateNoteInput) => call<NoteDetail>('note_create', { input }),

  get: (id: string) => call<NoteDetail>('note_get', { id }),

  save: (input: SaveNoteInput) => call<SaveNoteResult>('note_save', { input }),

  list: (query: NoteListQuery) => call<NoteListResult>('note_list', { query }),

  recent: (limit = 10) => call<NoteSummary[]>('note_recent', { limit }),

  listDeleted: () => call<NoteSummary[]>('note_list_deleted'),

  listMissing: () => call<NoteSummary[]>('note_list_missing'),

  remove: (id: string) => call<void>('note_delete', { id }),

  restore: (id: string) => call<NoteDetail>('note_restore', { id }),

  purge: (id: string) => call<void>('note_purge', { id }),

  rename: (id: string, title: string, renameFile = true) =>
    call<NoteDetail>('note_rename', { id, title, renameFile }),

  move: (id: string, categoryId: string | null) =>
    call<NoteDetail>('note_move', { id, categoryId }),

  duplicate: (id: string) => call<NoteDetail>('note_duplicate', { id }),

  setFavorite: (id: string, favorite: boolean) =>
    call<NoteDetail>('note_set_favorite', { id, favorite }),

  setTags: (id: string, tags: string[]) => call<NoteDetail>('note_set_tags', { id, tags }),

  count: (status?: string) => call<number>('note_count', { status }),
};
