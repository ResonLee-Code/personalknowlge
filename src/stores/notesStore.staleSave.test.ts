import { beforeEach, describe, expect, it, vi } from 'vitest';

import { noteService } from '@/services/noteService';
import { useNotesStore } from '@/stores/notesStore';
import type { NoteDetail } from '@/types';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

function detail(id: string, title: string): NoteDetail {
  return {
    note: {
      id,
      title,
      filePath: `00_Inbox/${id}.md`,
      status: 'active',
      favorite: false,
      wordCount: 1,
      createdAt: '2026-10-11T10:00:00+08:00',
      updatedAt: '2026-10-11T10:00:00+08:00',
      contentHash: `hash-${id}`,
      categoryId: null,
      tags: [],
      sourceType: null,
      sourceRef: null,
      sourceReliability: null,
      validUntil: null,
      pinned: false,
      sortOrder: 0,
      parentId: null,
    },
    content: `# ${title}`,
    categoryPath: '',
    tags: [],
  } as unknown as NoteDetail;
}

/**
 * 回归（2026-10-11 真机验收，P0）：**切笔记时「迟到的保存」不能把界面拉回上一篇**。
 *
 * 现场：从目录管理连续进入两篇笔记时，编辑区在 A 与 B 之间频闪；更严重的是
 * 这个来回切换的循环里，每一轮都可能把另一篇的内容写进当前笔记——四篇笔记的正文被写空。
 *
 * 机制：路由要打开 B（detail=B），但编辑器里还留着 A 的内容，于是 flush 保存了 A；
 * 保存完成后 `save()` 无条件 `set({ detail })` 把 detail 又改回 A，`NotesSection` 的
 * effect 见 `detail.note.id !== 路由 id` 又去打开 B……如此循环。
 */
describe('切笔记时迟到的保存', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('保存「已经不在界面上的」笔记，不会把当前 detail 换回去（这条链断了就不会频闪）', async () => {
    vi.spyOn(noteService, 'save').mockResolvedValue({
      note: detail('note_a', 'A').note,
      indexWarning: null,
    } as never);
    vi.spyOn(noteService, 'get').mockResolvedValue(detail('note_a', 'A'));
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 300,
    });

    // 界面此刻显示的是 B
    useNotesStore.setState({ detail: detail('note_b', 'B'), selectedId: 'note_b' });

    await useNotesStore.getState().save({ id: 'note_a', content: 'A 的内容' });

    const state = useNotesStore.getState();
    expect(state.detail?.note.id, '迟到的保存不能抢走界面的 detail').toBe('note_b');
    expect(state.selectedId).toBe('note_b');
  });

  it('保存当前显示的这篇时，照常刷新 detail（正常路径不受影响）', async () => {
    vi.spyOn(noteService, 'save').mockResolvedValue({
      note: detail('note_b', 'B2').note,
      indexWarning: null,
    } as never);
    vi.spyOn(noteService, 'get').mockResolvedValue(detail('note_b', 'B2'));
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 300,
    });
    useNotesStore.setState({ detail: detail('note_b', 'B'), selectedId: 'note_b' });

    await useNotesStore.getState().save({ id: 'note_b', content: 'B 的内容' });

    expect(useNotesStore.getState().detail?.note.title).toBe('B2');
  });
});
