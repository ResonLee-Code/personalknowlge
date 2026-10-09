import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ManagePage } from '@/features/manage/ManagePage';
import { noteService } from '@/services/noteService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import type { NoteSummary } from '@/types';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

function note(id: string, title: string, patch: Partial<NoteSummary> = {}): NoteSummary {
  return {
    id,
    title,
    filePath: `${id}.md`,
    categoryId: 'cat_1',
    categoryPath: '技术',
    status: 'active',
    favorite: false,
    wordCount: 10,
    createdAt: '2026-10-01T00:00:00+08:00',
    updatedAt: '2026-10-01T00:00:00+08:00',
    tags: ['sqlite'],
    ...patch,
  };
}

describe('ManagePage', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useCategoriesStore.setState({
      flat: [
        {
          id: 'cat_1',
          workspaceId: 'ws',
          parentId: null,
          name: '技术',
          sortOrder: 1,
          dirName: '01_技术',
          createdAt: '2026-10-01T00:00:00+08:00',
          noteCount: 2,
        },
      ],
    });
  });

  it('列出文档并展示分类、标签与层级', async () => {
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [note('n1', '顶层文档'), note('n2', '子文档', { parentId: 'n1', pinned: true })],
      total: 2,
      page: 1,
      pageSize: 500,
    });

    render(
      <MemoryRouter>
        <ManagePage />
      </MemoryRouter>,
    );

    expect(await screen.findAllByTestId('manage-row')).toHaveLength(2);
    expect(screen.getByText('顶层文档')).toBeInTheDocument();
    expect(screen.getByText('顶层')).toBeInTheDocument();
    // 子文档行显示父文档标题（顶层文档在结果里，所以能解析出名字）
    expect(screen.getByText(/子文档 · 顶层文档/)).toBeInTheDocument();
    expect(screen.getByText(/共 2 篇（最多显示 500 篇）/)).toBeInTheDocument();
  });

  it('筛选条件会带进查询（状态 / 不在目录中 / 关键词）', async () => {
    const list = vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 500,
    });

    render(
      <MemoryRouter>
        <ManagePage />
      </MemoryRouter>,
    );
    await screen.findByText('当前筛选下没有文档');

    await userEvent.selectOptions(screen.getByLabelText('按状态筛选'), 'deleted');
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith(expect.objectContaining({ status: 'deleted' })),
    );

    await userEvent.click(screen.getByTestId('manage-root-only'));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith(expect.objectContaining({ rootOnly: true })),
    );

    await userEvent.type(screen.getByTestId('manage-search'), '索引');
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith(expect.objectContaining({ keyword: '索引' })),
    );
  });

  it('全选后批量工具栏出现，可打开移动/打标签/复制对话框', async () => {
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [note('n1', '甲'), note('n2', '乙')],
      total: 2,
      page: 1,
      pageSize: 500,
    });

    render(
      <MemoryRouter>
        <ManagePage />
      </MemoryRouter>,
    );
    await screen.findAllByTestId('manage-row');

    await userEvent.click(screen.getByTestId('manage-select-all'));
    expect(await screen.findByTestId('manage-batch-toolbar')).toHaveTextContent('已选 2');

    await userEvent.click(screen.getByTestId('manage-move'));
    expect(await screen.findByText('批量移动分类')).toBeInTheDocument();
  });
});
