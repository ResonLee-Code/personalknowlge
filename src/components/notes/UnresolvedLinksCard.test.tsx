import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { UnresolvedLinksCard } from '@/components/notes/UnresolvedLinksCard';
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
    get: vi.fn(),
  },
}));

const unresolved = vi.mocked(linkService.unresolved);
const create = vi.mocked(noteService.create);

function renderCard() {
  return render(
    <MemoryRouter>
      <UnresolvedLinksCard />
    </MemoryRouter>,
  );
}

describe('UnresolvedLinksCard', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    unresolved.mockResolvedValue([]);
  });

  it('没有未解析链接时给出引导文案', async () => {
    renderCard();
    expect(await screen.findByText(/没有待处理的链接/)).toBeInTheDocument();
  });

  it('列出未解析链接，可一键创建目标笔记', async () => {
    unresolved.mockResolvedValue([
      {
        sourceNoteId: 'note_source',
        sourceTitle: '查询优化',
        targetTitle: '还没有的笔记',
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

    renderCard();

    expect(await screen.findByText('[[还没有的笔记]]')).toBeInTheDocument();
    expect(screen.getByText('查询优化')).toBeInTheDocument();
    expect(screen.getByText('1')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: '创建笔记：还没有的笔记' }));

    await waitFor(() => expect(create).toHaveBeenCalledWith({ title: '还没有的笔记' }));
    await waitFor(() => expect(unresolved.mock.calls.length).toBeGreaterThan(1));
  });

  it('超过上限时提示剩余数量', async () => {
    unresolved.mockResolvedValue(
      Array.from({ length: 7 }, (_, index) => ({
        sourceNoteId: `note_${index}`,
        sourceTitle: `来源 ${index}`,
        targetTitle: `目标 ${index}`,
        context: '',
      })),
    );
    renderCard();

    expect(await screen.findByText('[[目标 0]]')).toBeInTheDocument();
    expect(screen.queryByText('[[目标 6]]')).not.toBeInTheDocument();
    expect(screen.getByText(/还有 2 处未解析链接/)).toBeInTheDocument();
  });
});
