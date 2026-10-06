import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { GraphPage } from '@/features/graph/GraphPage';
import { graphService } from '@/services/graphService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import type { GraphData } from '@/types';

vi.mock('@/services/graphService', () => ({
  graphService: { get: vi.fn() },
}));

const getGraph = vi.mocked(graphService.get);

function payload(overrides: Partial<GraphData> = {}): GraphData {
  return {
    nodes: [
      {
        id: 'note_a',
        title: '索引设计',
        categoryId: 'cat_tech',
        updatedAt: '2026-10-07T09:00:00+08:00',
        degree: 1,
        tags: ['搜索'],
        orphan: false,
      },
      {
        id: 'note_b',
        title: '孤立笔记',
        categoryId: null,
        updatedAt: '2026-10-07T08:00:00+08:00',
        degree: 0,
        tags: [],
        orphan: true,
      },
    ],
    edges: [{ source: 'note_a', target: 'note_b', linkType: 'wikilink', weight: 1 }],
    truncated: false,
    totalNodes: 2,
    totalEdges: 1,
    durationMs: 7,
    ...overrides,
  };
}

function renderPage() {
  return render(
    <MemoryRouter>
      <GraphPage />
    </MemoryRouter>,
  );
}

describe('GraphPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    getGraph.mockResolvedValue(payload());
    useCategoriesStore.setState({
      flat: [
        {
          id: 'cat_tech',
          workspaceId: 'ws',
          parentId: null,
          name: '技术',
          sortOrder: 1,
          dirName: '01_技术',
          createdAt: '2026-10-01T00:00:00+08:00',
          noteCount: 1,
        },
      ],
    });
  });

  it('展示节点/连接统计与图例', async () => {
    renderPage();

    expect(await screen.findByText(/2 个节点/)).toBeInTheDocument();
    expect(screen.getByText(/1 条连接/)).toBeInTheDocument();
    expect(screen.getByText('未分类')).toBeInTheDocument();
    expect(screen.getByTestId('graph-canvas')).toBeInTheDocument();
  });

  it('按分类与标签筛选时带上查询参数', async () => {
    renderPage();
    await screen.findByTestId('graph-canvas');

    await userEvent.selectOptions(screen.getByLabelText('按分类筛选图谱'), 'cat_tech');
    await waitFor(() =>
      expect(getGraph).toHaveBeenLastCalledWith(
        expect.objectContaining({ categoryId: 'cat_tech' }),
      ),
    );

    await userEvent.click(screen.getByRole('checkbox', { name: '孤立节点' }));
    await waitFor(() =>
      expect(getGraph).toHaveBeenLastCalledWith(
        expect.objectContaining({ includeOrphans: false }),
      ),
    );
  });

  it('可切换为按社区着色并更新图例', async () => {
    renderPage();
    await screen.findByTestId('graph-canvas');

    await userEvent.selectOptions(screen.getByLabelText('图谱着色方式'), 'community');
    expect(await screen.findByText('社区 1')).toBeInTheDocument();
    expect(screen.queryByText('未分类')).not.toBeInTheDocument();
  });

  it('截断时给出提示', async () => {
    getGraph.mockResolvedValue(payload({ truncated: true, totalNodes: 50 }));
    renderPage();

    expect(await screen.findByText(/节点或连接过多/)).toBeInTheDocument();
  });

  it('没有节点时给出空状态', async () => {
    getGraph.mockResolvedValue(payload({ nodes: [], edges: [], totalNodes: 0, totalEdges: 0 }));
    renderPage();

    expect(await screen.findByText('当前筛选下没有节点')).toBeInTheDocument();
  });

  it('查询失败时展示错误与重试', async () => {
    getGraph.mockRejectedValue({ code: 'INTERNAL_ERROR', message: '查询失败' });
    renderPage();

    expect(await screen.findByText('图谱加载失败')).toBeInTheDocument();
    expect(screen.getByText('查询失败')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '重试' }));
    await waitFor(() => expect(getGraph.mock.calls.length).toBeGreaterThan(1));
  });
});
