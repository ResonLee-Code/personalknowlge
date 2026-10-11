import { beforeEach, describe, expect, it, vi } from 'vitest';

import { noteService } from '@/services/noteService';
import { useNotesStore } from '@/stores/notesStore';
import type { NoteSummary } from '@/types';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

function summary(id: string, title: string): NoteSummary {
  return {
    id,
    title,
    filePath: `00_Inbox/${id}.md`,
    categoryId: null,
    categoryPath: '',
    status: 'active',
    favorite: false,
    wordCount: 0,
    createdAt: '2026-10-11T10:00:00+08:00',
    updatedAt: '2026-10-11T10:00:00+08:00',
    tags: [],
    pinned: false,
    sortOrder: 0,
    parentId: null,
  };
}

/**
 * 回归：**切换知识库后列表必须按新库重新加载**。
 *
 * 现场表现（2026-10-11 用户复现）：从「验收临时库」切到 `knowledge` 后，
 * 侧边栏计数是新的（全部笔记 3），但中栏一直显示「这里还没有笔记」、状态栏还挂着
 * 「笔记不存在或已被删除。」——因为切库只 `reset()` 清空了状态，**没有任何人再去取一次列表**，
 * 得等用户在页面上做点别的（换筛选 / 换路由）才会顺带刷新。
 */
describe('切换知识库后的列表加载', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useNotesStore.setState({
      items: [],
      total: 0,
      scope: { kind: 'all' },
      keyword: '',
      rootOnly: false,
      selectedId: null,
      detail: null,
      error: null,
    });
  });

  it('reloadForWorkspace 清掉旧库数据，并按新库重新取一次列表', async () => {
    const list = vi.spyOn(noteService, 'list');
    list.mockResolvedValueOnce({ items: [summary('note_a', '旧库笔记')], total: 1, page: 1, pageSize: 300 });
    await useNotesStore.getState().refresh();
    expect(useNotesStore.getState().items.map((item) => item.id)).toEqual(['note_a']);

    // 切库：新库有另一篇笔记
    list.mockResolvedValueOnce({ items: [summary('note_b', '新库笔记')], total: 1, page: 1, pageSize: 300 });
    await useNotesStore.getState().reloadForWorkspace();

    expect(list).toHaveBeenCalledTimes(2);
    expect(useNotesStore.getState().items.map((item) => item.id)).toEqual(['note_b']);
    expect(useNotesStore.getState().total).toBe(1);
    // 旧库的「当前笔记」不能留在新库里（否则详情会去请求一个不属于新库的 id）
    expect(useNotesStore.getState().detail).toBeNull();
    expect(useNotesStore.getState().selectedId).toBeNull();
  });

  it('切库时把关键词/多选/「不在目录中」等旧库视图状态一并清掉', async () => {
    vi.spyOn(noteService, 'list').mockResolvedValue({ items: [], total: 0, page: 1, pageSize: 300 });
    useNotesStore.setState({
      keyword: '旧关键词',
      rootOnly: true,
      selection: ['note_a'],
      selectionMode: true,
    });

    await useNotesStore.getState().reloadForWorkspace();

    const state = useNotesStore.getState();
    expect(state.keyword).toBe('');
    expect(state.rootOnly).toBe(false);
    expect(state.selection).toEqual([]);
    expect(state.selectionMode).toBe(false);
  });
});
