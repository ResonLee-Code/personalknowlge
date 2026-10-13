import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { SearchResultItem } from '@/components/search/SearchResultItem';
import { useUiStore } from '@/stores/uiStore';
import type { SearchHit } from '@/types';

const navigate = vi.fn();
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual<typeof import('react-router-dom')>('react-router-dom');
  return { ...actual, useNavigate: () => navigate };
});

function hit(overrides: Partial<SearchHit> = {}): SearchHit {
  return {
    note: {
      id: 'note_1',
      title: '索引设计',
      categoryId: null,
      categoryPath: '',
      tags: [],
      favorite: false,
      pinned: false,
      status: 'active',
      wordCount: 10,
      createdAt: '2026-10-12T10:00:00+08:00',
      updatedAt: '2026-10-12T10:00:00+08:00',
      sourceType: null,
      sourceRef: null,
      sourceReliability: null,
      validUntil: null,
      parentId: null,
      sortOrder: 0,
      filePath: '00_Inbox/索引设计.md',
    },
    score: -0.5,
    titleMatched: false,
    snippet: [],
    matchedTerms: ['查询优化'],
    ...overrides,
  } as unknown as SearchHit;
}

/**
 * N5：命中带了原文精确区间时，点它要先登记「待高亮区间」再跳转——
 * 编辑器据此滚到那一段并选中。没带区间时只跳转，不做多余的事。
 */
describe('SearchResultItem（N5 精确定位）', () => {
  beforeEach(() => {
    navigate.mockClear();
    useUiStore.setState({ pendingHighlight: null });
  });

  it('有精确区间时，点击会登记待高亮区间并跳转到该笔记', async () => {
    render(
      <SearchResultItem
        hit={hit({
          location: { heading: '索引设计 > 查询策略', chunkIndex: 1, charStart: 42, charEnd: 46 },
        })}
        keyword="查询优化"
      />,
    );

    expect(screen.getByText('§ 索引设计 > 查询策略')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button'));

    expect(useUiStore.getState().pendingHighlight).toEqual({
      noteId: 'note_1',
      charStart: 42,
      charEnd: 46,
    });
    expect(navigate).toHaveBeenCalledWith('/notes/note_1');
  });

  it('没有精确区间时只跳转，不登记高亮（不猜位置）', async () => {
    render(<SearchResultItem hit={hit()} keyword="查询优化" />);

    await userEvent.click(screen.getByRole('button'));

    expect(useUiStore.getState().pendingHighlight).toBeNull();
    expect(navigate).toHaveBeenCalledWith('/notes/note_1');
  });
});
