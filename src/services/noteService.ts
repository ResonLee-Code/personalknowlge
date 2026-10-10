import type {
  BatchReport,
  CreateNoteInput,
  NoteDetail,
  NoteListQuery,
  NoteListResult,
  NoteSummary,
  SaveNoteInput,
  SetSourceInput,
  SetParentInput,
  ReorderNotesInput,
  SaveNoteResult,
  ImportNotesInput,
  ImportReport,
  ExportNoteInput,
  ExportNoteResult,
  HistoryContent,
  HistoryEntry,
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

  /** 置顶 / 取消置顶（对标语雀「置顶文档」；写回 frontmatter 的 pinned） */
  setPinned: (id: string, pinned: boolean) =>
    call<NoteDetail>('note_set_pinned', { id, pinned }),

  /** 设置父文档（子文档层级）；`parentId` 传 null 表示移到顶层 */
  setParent: (input: SetParentInput) => call<NoteDetail>('note_set_parent', { input }),

  /** 版本历史：列出快照（新的在前） */
  historyList: (noteId: string) => call<HistoryEntry[]>('note_history_list', { noteId }),

  /** 版本历史：读取某个快照 */
  historyRead: (noteId: string, entryId: string) =>
    call<HistoryContent>('note_history_read', { noteId, entryId }),

  /** 版本历史：还原到某个快照（只回滚正文） */
  historyRestore: (noteId: string, entryId: string) =>
    call<NoteDetail>('note_history_restore', { noteId, entryId }),

  /** 版本历史：把当前内容存成命名版本（对标语雀「保存为版本」） */
  historySaveVersion: (noteId: string, label: string) =>
    call<HistoryEntry>('note_history_save_version', { noteId, label }),

  /** 版本历史：命名 / 清除命名（label 传 null 或空串即清除），返回最新列表 */
  historyLabel: (noteId: string, entryId: string, label: string | null) =>
    call<HistoryEntry[]>('note_history_label', { noteId, entryId, label }),

  /** 版本历史：删除某个快照（返回剩余数量） */
  historyRemove: (noteId: string, entryId: string) =>
    call<number>('note_history_remove', { noteId, entryId }),

  /** 手动排序：按传入顺序改写同层笔记的 `order`（返回实际更新的篇数） */
  reorder: (input: ReorderNotesInput) => call<number>('notes_reorder', { input }),

  setTags: (id: string, tags: string[]) => call<NoteDetail>('note_set_tags', { id, tags }),

  /** 更新来源与时效（N1）；会写回 Markdown frontmatter */
  setSource: (input: SetSourceInput) => call<NoteDetail>('note_set_source', { input }),

  count: (status?: string) => call<number>('note_count', { status }),

  /** 批量移动到分类（null 表示 Inbox） */
  moveMany: (ids: string[], categoryId: string | null) =>
    call<BatchReport>('notes_move_batch', { input: { ids, categoryId } }),

  /** 批量打标签（mode: add / remove / replace） */
  tagMany: (ids: string[], tags: string[], mode: 'add' | 'remove' | 'replace' = 'add') =>
    call<BatchReport>('notes_tag_batch', { input: { ids, tags, mode } }),

  /** 批量移入回收站 */
  deleteMany: (ids: string[]) =>
    call<BatchReport>('notes_delete_batch', { input: { ids } }),

  /** 导入外部 Markdown / 文本文件或目录（复制进知识库） */
  importFiles: (input: ImportNotesInput) => call<ImportReport>('note_import', { input }),

  /** 导出单篇笔记（md / html / docx / pdf 打印视图） */
  exportNote: (input: ExportNoteInput) => call<ExportNoteResult>('note_export', { input }),
};
