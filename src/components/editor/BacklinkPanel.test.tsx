import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes, useParams } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { BacklinkPanel } from '@/components/editor/BacklinkPanel';
import { linkService } from '@/services/linkService';
import { noteService } from '@/services/noteService';

vi.mock('@/services/linkService', () => ({
  linkService: {
    backlinks: vi.fn(),
    outgoing: vi.fn(),
    unresolved: vi.fn(),
    counts: vi.fn(),
    rebuild: vi.fn(),
  },
}));

vi.mock('@/services/noteService', () => ({
  noteService: {
    create: vi.fn(),
    list: vi.fn(),
  },
}));

const backlinks = vi.mocked(linkService.backlinks);
const outgoing = vi.mocked(linkService.outgoing);
const create = vi.mocked(noteService.create);

function Harness({ noteId }: { noteId: string }) {
  const params = useParams();
  return (
    <>
      <div data-testid="route-id">{params.id}</div>
      <BacklinkPanel noteId={noteId} />
    </>
  );
}

function renderPanel(noteId = 'note_a') {
  return render(
    <MemoryRouter initialEntries={[`/notes/${noteId}`]}>
      <Routes>
        <Route path="/notes/:id" element={<Harness noteId={noteId} />} />
      </Routes>
    </MemoryRouter>,
  );
}

describe('BacklinkPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    backlinks.mockResolvedValue([]);
    outgoing.mockResolvedValue([]);
  });

  it('没有引用时给出空状态与引用计数', async () => {
    renderPanel();

    expect(await screen.findByText(/还没有笔记引用这篇笔记/)).toBeInTheDocument();
    expect(screen.getByText('被 0 篇引用')).toBeInTheDocument();
  });

  it('展示反向链接的标题、上下文与时间，并可点击跳转到来源笔记', async () => {
    backlinks.mockResolvedValue([
      {
        noteId: 'note_source',
        title: '索引设计笔记',
        filePath: '20_技术/索引设计笔记.md',
        context: '参考 [[目标笔记]] 里的倒排索引章节',
        anchor: null,
        updatedAt: new Date().toISOString(),
      },
    ]);

    renderPanel();

    expect(await screen.findByText('索引设计笔记')).toBeInTheDocument();
    expect(screen.getByText(/参考 \[\[目标笔记\]\] 里的倒排索引章节/)).toBeInTheDocument();
    expect(screen.getByText('被 1 篇引用')).toBeInTheDocument();

    await userEvent.click(screen.getByTestId('backlink-item'));
    await waitFor(() => expect(screen.getByTestId('route-id')).toHaveTextContent('note_source'));
  });

  it('未解析链接提供一键创建按钮，创建后重新拉取链接数据', async () => {
    outgoing.mockResolvedValue([
      {
        targetTitle: '还没有的笔记',
        targetNoteId: null,
        targetExists: false,
        anchor: null,
        linkType: 'wikilink',
        context: '待补充 [[还没有的笔记]]',
      },
    ]);
    create.mockResolvedValue({
      note: {
        id: 'note_new',
        workspaceId: 'ws',
        title: '还没有的笔记',
        filePath: '00_Inbox/还没有的笔记.md',
        categoryId: null,
        status: 'active',
        contentHash: 'hash',
        wordCount: 0,
        favorite: false,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
        lastIndexedAt: null,
      },
      content: '',
      tags: [],
      categoryPath: '',
    });
    vi.mocked(noteService.list).mockResolvedValue({ items: [], total: 0, page: 1, pageSize: 300 });

    renderPanel();

    expect(await screen.findByText('本篇未解析链接')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '创建笔记：还没有的笔记' }));

    await waitFor(() =>
      expect(create).toHaveBeenCalledWith({ title: '还没有的笔记' }),
    );
    // 创建后刷新一次，未解析条目消失
    outgoing.mockResolvedValue([]);
    await waitFor(() => expect(backlinks.mock.calls.length).toBeGreaterThan(1));
  });

  /**
   * 回归：反链读取失败以前被 `.catch` 吞成空列表，面板显示「还没有笔记引用这篇笔记」——
   * 用户会以为自己的笔记真的没被引用。失败必须看得出来，并给重试。
   */
  it('读取失败时明确报错并可重试，不显示成「还没有笔记引用」', async () => {
    backlinks
      .mockRejectedValueOnce({ code: 'IO_ERROR', message: '链接表读不出来' })
      .mockResolvedValue([]);
    outgoing.mockResolvedValue([]);

    renderPanel();

    expect(await screen.findByTestId('backlink-load-error')).toHaveTextContent('链接表读不出来');
    expect(screen.queryByText(/还没有笔记引用这篇笔记/)).not.toBeInTheDocument();
    // 头部计数也不能是「被 0 篇引用」这种假结论
    expect(screen.getByText('读取失败')).toBeInTheDocument();

    await userEvent.click(screen.getByTestId('backlink-load-retry'));
    await waitFor(() =>
      expect(screen.queryByTestId('backlink-load-error')).not.toBeInTheDocument(),
    );
    expect(screen.getByText(/还没有笔记引用这篇笔记/)).toBeInTheDocument();
  });
});
