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

  it('批量复制到分类：复制 + 移动副本，原作不动', async () => {
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 300,
    });
    const duplicate = vi
      .spyOn(noteService, 'duplicate')
      .mockResolvedValueOnce({ note: { id: 'copy_1' } } as never)
      .mockResolvedValueOnce({ note: { id: 'copy_2' } } as never);
    const move = vi.spyOn(noteService, 'move').mockResolvedValue({} as never);
    const remove = vi.spyOn(noteService, 'remove').mockResolvedValue(undefined);

    useNotesStore.setState({ selection: ['note_1', 'note_2'], selectionMode: true });
    await useNotesStore.getState().copyManyTo('cat_target');

    expect(duplicate).toHaveBeenCalledTimes(2);
    expect(move).toHaveBeenNthCalledWith(1, 'copy_1', 'cat_target');
    expect(move).toHaveBeenNthCalledWith(2, 'copy_2', 'cat_target');
    // 复制语义：原作必须保留
    expect(remove).not.toHaveBeenCalled();
    expect(useNotesStore.getState().selection).toEqual([]);
  });
});

describe('单篇复制到指定分类（C4）', () => {
  it('复制出副本并移动，原作保留', async () => {
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 300,
    });
    const duplicate = vi
      .spyOn(noteService, 'duplicate')
      .mockResolvedValue({ note: { id: 'copy_1', title: '甲 副本' } } as never);
    const move = vi.spyOn(noteService, 'move').mockResolvedValue({} as never);
    const remove = vi.spyOn(noteService, 'remove').mockResolvedValue(undefined);

    const ok = await useNotesStore.getState().copyTo('note_1', 'cat_target');

    expect(ok).toBe(true);
    expect(duplicate).toHaveBeenCalledWith('note_1');
    expect(move).toHaveBeenCalledWith('copy_1', 'cat_target');
    expect(remove).not.toHaveBeenCalled();
  });

  it('目标为 Inbox（null）时不调用移动', async () => {
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 300,
    });
    vi.spyOn(noteService, 'duplicate').mockResolvedValue({
      note: { id: 'copy_2', title: '甲 副本' },
    } as never);
    const move = vi.spyOn(noteService, 'move').mockResolvedValue({} as never);

    await useNotesStore.getState().copyTo('note_1', null);
    expect(move).not.toHaveBeenCalled();
  });
});
