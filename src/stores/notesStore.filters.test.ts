import { beforeEach, describe, expect, it, vi } from 'vitest';

import { noteService } from '@/services/noteService';
import { useNotesStore } from '@/stores/notesStore';

vi.mock('sonner', () => ({
  toast: {
    success: vi.fn(),
    warning: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
  },
}));

import { toast } from 'sonner';

describe('笔记列表筛选与批量撤销', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('「不在目录中」筛选会带进列表查询', async () => {
    const list = vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 300,
    });
    useNotesStore.setState({ rootOnly: false });

    await useNotesStore.getState().setRootOnly(true);
    expect(list).toHaveBeenLastCalledWith(expect.objectContaining({ rootOnly: true }));

    await useNotesStore.getState().setRootOnly(false);
    expect(list).toHaveBeenLastCalledWith(expect.objectContaining({ rootOnly: false }));
  });

  it('批量删除后提供「撤销」，撤销会恢复成功删除的笔记', async () => {
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 300,
    });
    vi.spyOn(noteService, 'deleteMany').mockResolvedValue({
      succeeded: 1,
      failures: [{ noteId: 'note_2', message: '笔记不存在' }],
    });
    const restore = vi.spyOn(noteService, 'restore').mockResolvedValue({} as never);

    useNotesStore.setState({ selection: ['note_1', 'note_2'], selectionMode: true });
    await useNotesStore.getState().deleteMany();

    // 有失败项时走 warning 分支，同样要带「撤销」
    const [, options] = vi.mocked(toast.warning).mock.calls.at(-1) as [
      string,
      { action?: { label: string; onClick: () => void } },
    ];
    expect(options?.action?.label).toBe('撤销');

    options!.action!.onClick();
    await vi.waitFor(() => expect(restore).toHaveBeenCalledWith('note_1'));
    // 删除失败的那篇不应被恢复
    expect(restore).not.toHaveBeenCalledWith('note_2');
  });

  it('全部删除成功时同样提供「撤销」', async () => {
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 300,
    });
    vi.spyOn(noteService, 'deleteMany').mockResolvedValue({ succeeded: 2, failures: [] });
    const restore = vi.spyOn(noteService, 'restore').mockResolvedValue({} as never);

    useNotesStore.setState({ selection: ['note_1', 'note_2'], selectionMode: true });
    await useNotesStore.getState().deleteMany();

    const [, options] = vi.mocked(toast.success).mock.calls.at(-1) as [
      string,
      { action?: { label: string; onClick: () => void } },
    ];
    expect(options?.action?.label).toBe('撤销');
    options!.action!.onClick();
    await vi.waitFor(() => expect(restore).toHaveBeenCalledWith('note_2'));
  });

  it('置顶会调用 note_set_pinned 并刷新列表', async () => {
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 300,
    });
    const setPinned = vi
      .spyOn(noteService, 'setPinned')
      .mockResolvedValue({ note: { id: 'note_1', pinned: true } } as never);
    const list = vi.mocked(noteService.list);

    await useNotesStore.getState().setPinned('note_1', true);
    expect(setPinned).toHaveBeenCalledWith('note_1', true);
    expect(list).toHaveBeenCalled();
  });
});
