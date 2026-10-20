import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { GlobalSearchDialog } from '@/components/search/GlobalSearchDialog';
import { searchService } from '@/services/searchService';
import { useUiStore } from '@/stores/uiStore';

vi.mock('@/services/searchService', () => ({
  searchService: { quick: vi.fn() },
}));

describe('GlobalSearchDialog · Ask 命令入口', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(searchService.quick).mockResolvedValue([]);
    useUiStore.setState({
      commandPaletteOpen: true,
      askPanelOpen: false,
      askSeed: null,
      researchSeed: null,
    });
  });

  it('把搜索框里的问题带到 Ask 右侧面板', async () => {
    render(
      <MemoryRouter>
        <GlobalSearchDialog />
      </MemoryRouter>,
    );

    await userEvent.type(screen.getByLabelText('搜索关键词'), 'SQLite FTS5 有什么限制？');
    await userEvent.click(screen.getByRole('button', { name: /AI: 就当前问题提问/ }));

    await waitFor(() => {
      expect(useUiStore.getState().askPanelOpen).toBe(true);
    });
    expect(useUiStore.getState().askSeed).toEqual({
      question: 'SQLite FTS5 有什么限制？',
    });
    expect(useUiStore.getState().commandPaletteOpen).toBe(false);
  });

  it('把搜索框里的文字带入 Research 2.0 调研入口', async () => {
    render(
      <MemoryRouter>
        <GlobalSearchDialog />
      </MemoryRouter>,
    );

    await userEvent.type(screen.getByLabelText('搜索关键词'), 'Rust 异步调度限制与生产风险');
    await userEvent.click(screen.getByRole('button', { name: /AI: 深入调研/ }));

    expect(useUiStore.getState().researchSeed).toEqual({
      topic: 'Rust 异步调度限制与生产风险',
    });
    expect(useUiStore.getState().commandPaletteOpen).toBe(false);
  });
});
