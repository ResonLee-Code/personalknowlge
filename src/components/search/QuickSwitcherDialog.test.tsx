import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes, useParams } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { QuickSwitcherDialog } from '@/components/search/QuickSwitcherDialog';
import { noteService } from '@/services/noteService';
import { useUiStore } from '@/stores/uiStore';
import type { NoteSummary } from '@/types';

vi.mock('@/services/noteService', () => ({
  noteService: {
    list: vi.fn(),
    get: vi.fn(),
  },
}));

const list = vi.mocked(noteService.list);

function summary(id: string, title: string): NoteSummary {
  return {
    id,
    title,
    filePath: `00_Inbox/${title}.md`,
    categoryId: null,
    categoryPath: '',
    status: 'active',
    favorite: false,
    wordCount: 10,
    tags: [],
    updatedAt: new Date().toISOString(),
    createdAt: new Date().toISOString(),
  };
}

function Harness() {
  const params = useParams();
  return (
    <>
      <div data-testid="route-id">{params.id ?? 'none'}</div>
      <QuickSwitcherDialog />
    </>
  );
}

describe('QuickSwitcherDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useUiStore.setState({ quickSwitcherOpen: false });
    list.mockResolvedValue({
      items: [summary('note_1', 'SQLite 全文搜索学习笔记'), summary('note_2', '索引设计笔记')],
      total: 2,
      page: 1,
      pageSize: 500,
    });
  });

  it('未打开时不渲染', () => {
    render(
      <MemoryRouter>
        <Harness />
      </MemoryRouter>,
    );
    expect(screen.queryByLabelText('按标题跳转')).not.toBeInTheDocument();
  });

  it('按标题模糊过滤并回车打开', async () => {
    useUiStore.setState({ quickSwitcherOpen: true });
    render(
      <MemoryRouter initialEntries={['/notes']}>
        <Routes>
          <Route path="/notes" element={<Harness />} />
          <Route path="/notes/:id" element={<Harness />} />
        </Routes>
      </MemoryRouter>,
    );

    const input = await screen.findByLabelText('按标题跳转');
    await waitFor(() => expect(screen.getByText('SQLite 全文搜索学习笔记')).toBeInTheDocument());

    await userEvent.type(input, '全文');
    expect(screen.queryByText('索引设计笔记')).not.toBeInTheDocument();

    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(screen.getByTestId('route-id')).toHaveTextContent('note_1'));
    expect(useUiStore.getState().quickSwitcherOpen).toBe(false);
  });

  it('没有匹配项时给出提示', async () => {
    useUiStore.setState({ quickSwitcherOpen: true });
    render(
      <MemoryRouter>
        <Harness />
      </MemoryRouter>,
    );

    const input = await screen.findByLabelText('按标题跳转');
    await userEvent.type(input, '不存在的标题');
    expect(await screen.findByText('没有匹配的标题')).toBeInTheDocument();
  });
});
