import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { VirtualSearchResults } from '@/components/search/VirtualSearchResults';
import type { SearchHit } from '@/types';

vi.mock('@/components/search/SearchResultItem', () => ({
  SearchResultItem: ({ hit }: { hit: SearchHit }) => (
    <div data-testid="virtual-hit">{hit.note.title}</div>
  ),
}));

function hits(count: number): SearchHit[] {
  return Array.from({ length: count }, (_, index) => ({
    note: {
      id: `note-${index}`,
      title: `笔记 ${index}`,
      filePath: `${index}.md`,
      categoryId: null,
      categoryPath: '',
      status: 'active',
      favorite: false,
      wordCount: 10,
      createdAt: '2026-01-01T00:00:00Z',
      updatedAt: '2026-01-01T00:00:00Z',
      tags: [],
    },
    score: 1,
    titleMatched: true,
    snippet: [{ text: '片段', matched: false }],
    matchedTerms: ['片段'],
  }));
}

describe('VirtualSearchResults', () => {
  it('100 条结果只挂载视口窗口内的节点', () => {
    render(<VirtualSearchResults hits={hits(100)} keyword="片段" />);
    const rendered = screen.getAllByTestId('virtual-hit');
    expect(rendered.length).toBeGreaterThan(0);
    expect(rendered.length).toBeLessThan(100);
    expect(Number(screen.getByTestId('search-virtual-list').dataset.renderedCount)).toBe(
      rendered.length,
    );
  });
});
