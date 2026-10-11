import { beforeEach, describe, expect, it, vi } from 'vitest';

import { createRequestGuard } from '@/lib/latestRequest';
import { noteService } from '@/services/noteService';
import { searchService } from '@/services/searchService';
import { registerEditorFlush, useNotesStore } from '@/stores/notesStore';
import { useSearchStore } from '@/stores/searchStore';
import type { NoteDetail, SearchResponse } from '@/types';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

function detail(id: string, title: string): NoteDetail {
  return {
    note: {
      id,
      workspaceId: 'ws_1',
      title,
      filePath: `00_Inbox/${title}.md`,
      categoryId: null,
      status: 'active',
      favorite: false,
      wordCount: 1,
      createdAt: '2026-10-11T10:00:00+08:00',
      updatedAt: '2026-10-11T10:00:00+08:00',
      contentHash: 'hash',
    },
    content: `${title} 的正文`,
    categoryPath: '',
    tags: [],
  } as NoteDetail;
}

function searchResponse(keyword: string): SearchResponse {
  return {
    keyword,
    hits: [],
    total: 0,
    page: 1,
    pageSize: 50,
    tookMs: 1,
    indexStale: false,
  } as SearchResponse;
}

describe('异步竞态：最后发起的请求才算数', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('守卫本身：只有最新一次 begin 的判定为真', () => {
    const guard = createRequestGuard();
    const first = guard.begin();
    const second = guard.begin();
    expect(first()).toBe(false);
    expect(second()).toBe(true);
    const third = guard.begin();
    expect(second()).toBe(false);
    expect(third()).toBe(true);
  });

  it('快速切换笔记时，先发的响应后到不会把界面跳回上一篇', async () => {
    let resolveSlow!: (value: NoteDetail) => void;
    const slow = new Promise<NoteDetail>((resolve) => {
      resolveSlow = resolve;
    });
    vi.spyOn(noteService, 'get')
      .mockReturnValueOnce(slow as never) // 先点的 A：慢
      .mockResolvedValueOnce(detail('note_b', '乙') as never); // 后点的 B：快

    const first = useNotesStore.getState().openDetail('note_a');
    const second = useNotesStore.getState().openDetail('note_b');
    await second;
    expect(useNotesStore.getState().detail?.note.id).toBe('note_b');

    // A 的响应此刻才回来——必须被丢弃
    resolveSlow(detail('note_a', '甲'));
    await first;
    expect(useNotesStore.getState().detail?.note.id).toBe('note_b');
    expect(useNotesStore.getState().selectedId).toBe('note_b');
  });

  it('连续检索时，旧关键词的结果不会覆盖新关键词的结果', async () => {
    let resolveSlow!: (value: SearchResponse) => void;
    const slow = new Promise<SearchResponse>((resolve) => {
      resolveSlow = resolve;
    });
    vi.spyOn(searchService, 'search')
      .mockReturnValueOnce(slow as never)
      .mockResolvedValueOnce(searchResponse('新词') as never);

    useSearchStore.setState({ keyword: '旧词' });
    const first = useSearchStore.getState().run();
    useSearchStore.setState({ keyword: '新词' });
    const second = useSearchStore.getState().run();
    await second;
    expect(useSearchStore.getState().response?.keyword).toBe('新词');

    resolveSlow(searchResponse('旧词'));
    await first;
    expect(useSearchStore.getState().response?.keyword).toBe('新词');
  });
  it('切换笔记前会先调用编辑器注册的「立即落盘」', async () => {
    const order: string[] = [];
    registerEditorFlush(async () => {
      order.push('flush');
      return true;
    });
    vi.spyOn(noteService, 'get').mockImplementation(async (id: string) => {
      order.push(`get:${id}`);
      return detail(id, id) as never;
    });

    useNotesStore.setState({ detail: detail('note_a', '甲'), selectedId: 'note_a' });
    await useNotesStore.getState().openDetail('note_b');
    expect(order, '必须先落盘再切走').toEqual(['flush', 'get:note_b']);

    // 同一篇（外部变更后刷新）不重复触发落盘
    order.length = 0;
    await useNotesStore.getState().openDetail('note_b');
    expect(order).toEqual(['get:note_b']);

    // 冲突对话框「以磁盘为准」时明确跳过落盘
    order.length = 0;
    useNotesStore.setState({ detail: detail('note_c', '丙'), selectedId: 'note_c' });
    await useNotesStore.getState().openDetail('note_d', { discardLocalEdits: true });
    expect(order).toEqual(['get:note_d']);

    registerEditorFlush(null);
  });
});
