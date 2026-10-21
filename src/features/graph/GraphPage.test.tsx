import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { GraphPage } from '@/features/graph/GraphPage';
import { toast } from 'sonner';
import { graphService } from '@/services/graphService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { GraphData, GraphInsights } from '@/types';

vi.mock('@/services/graphService', () => ({
  graphService: { get: vi.fn(), insights: vi.fn() },
}));

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

const getGraph = vi.mocked(graphService.get);
const getInsights = vi.mocked(graphService.insights);

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

function insightsPayload(overrides: Partial<GraphInsights> = {}): GraphInsights {
  return {
    mode: 'local',
    neighbors: [
      {
        noteId: 'note_b',
        title: '孤立笔记',
        categoryId: null,
        score: 0.71,
        evidence: [
          { kind: 'sharedTopic', label: '共同主题：搜索、索引', noteIds: ['note_b'] },
          { kind: 'similarContent', label: '正文词项相似度 71%', noteIds: ['note_b'] },
        ],
      },
    ],
    duplicateClusters: [
      {
        id: 'duplicate-note_a',
        similarity: 0.9,
        reason: '正文词项相似度至少 90%，合并前请核对差异。',
        members: [
          {
            noteId: 'note_a',
            title: '索引设计',
            excerpt: '倒排索引与分块。',
            updatedAt: '2026-10-07T09:00:00+08:00',
          },
          {
            noteId: 'note_b',
            title: '索引设计旧版',
            excerpt: '倒排索引与分块检索。',
            updatedAt: '2026-10-06T09:00:00+08:00',
          },
        ],
        requiresConfirmation: true,
      },
    ],
    orphanNotes: [],
    bridgeNotes: [],
    islandCategories: [],
    sampledNodes: 2,
    totalNodes: 2,
    truncated: false,
    warnings: ['本轮使用本地词项降级通道。'],
    durationMs: 4,
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
    getInsights.mockResolvedValue(insightsPayload());
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
      expect(getGraph).toHaveBeenLastCalledWith(expect.objectContaining({ includeOrphans: false })),
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

  it('洞察展示可解释近邻，重复只打开只读预览', async () => {
    useWorkspaceStore.setState({ workspace: { id: 'ws_a' } as never });
    useNotesStore.setState({ selectedId: 'note_a' });
    renderPage();
    await screen.findByTestId('graph-canvas');
    await userEvent.click(screen.getByRole('button', { name: '洞察' }));

    expect(await screen.findByText('共同主题：搜索、索引')).toBeInTheDocument();
    expect(screen.getByText(/本轮使用本地词项降级通道/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /索引设计.*2 篇/ }));
    expect(await screen.findByTestId('duplicate-preview')).toBeInTheDocument();
    expect(screen.getByText(/不会自动改写任何笔记/)).toBeInTheDocument();
  });

  it('切换知识库后自动重新查询（不再需要手动刷新）', async () => {
    useWorkspaceStore.setState({ workspace: { id: 'ws_a' } as never });
    renderPage();
    await screen.findByTestId('graph-canvas');
    const initialCalls = getGraph.mock.calls.length;

    getGraph.mockResolvedValue(payload({ nodes: [], edges: [], totalNodes: 0, totalEdges: 0 }));
    useWorkspaceStore.setState({ workspace: { id: 'ws_b' } as never });

    await waitFor(() => expect(getGraph.mock.calls.length).toBeGreaterThan(initialCalls));
    expect(await screen.findByText('当前筛选下没有节点')).toBeInTheDocument();
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

  // 回归：无 Canvas 的环境（测试、无头）里点「导出 PNG」以前是**静默无事发生**，
  // 现在必须把失败原因交回页面提示用户，而不是让人以为已经导出成功。
  it('导出 PNG 失败时给出明确提示，而不是静默', async () => {
    renderPage();
    await screen.findByTestId('graph-canvas');
    await screen.findByRole('button', { name: '导出 PNG' });

    await userEvent.click(screen.getByRole('button', { name: '导出 PNG' }));

    await waitFor(() =>
      expect(vi.mocked(toast.error)).toHaveBeenCalledWith(expect.stringContaining('不支持 Canvas')),
    );
  });
});
