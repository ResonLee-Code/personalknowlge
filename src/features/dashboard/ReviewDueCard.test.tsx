import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { ReviewDueCard } from '@/features/dashboard/ReviewDueCard';
import type { ReviewDueNote } from '@/types';

function item(overrides: Partial<ReviewDueNote> = {}): ReviewDueNote {
  return {
    noteId: 'note_1',
    title: 'SQLite 全文搜索学习笔记',
    categoryPath: '技术/数据库',
    validUntil: '2026-09-01',
    overdueDays: 40,
    sourceType: 'official',
    sourceReliability: 4,
    ...overrides,
  };
}

describe('ReviewDueCard（V1.2.1 D4 待复核）', () => {
  it('没有待复核笔记时不占用首页空间', () => {
    const { container } = render(<ReviewDueCard items={[]} onOpen={() => undefined} />);
    expect(container).toBeEmptyDOMElement();
    expect(screen.queryByTestId('review-due-card')).not.toBeInTheDocument();
  });

  it('已过期的排前面，并分别标注过期与到期天数', async () => {
    render(
      <ReviewDueCard
        items={[
          item(),
          item({
            noteId: 'note_2',
            title: '快速变化的手册',
            categoryPath: '',
            validUntil: '2026-10-15',
            overdueDays: -4,
          }),
        ]}
        onOpen={() => undefined}
      />,
    );

    const card = screen.getByTestId('review-due-card');
    expect(card).toHaveTextContent('2 篇（其中已过期 1 篇）');
    expect(card).toHaveTextContent('已过期 40 天');
    expect(card).toHaveTextContent('还有 4 天到期');
    expect(card).toHaveTextContent('技术/数据库');
    // 未分类显示 Inbox，与列表口径一致
    expect(card).toHaveTextContent('Inbox');

    const titles = screen.getAllByRole('button').map((button) => button.textContent);
    expect(titles).toEqual(['SQLite 全文搜索学习笔记', '快速变化的手册']);
  });

  it('点击标题会带着 noteId 回调（用于跳转笔记）', async () => {
    const onOpen = vi.fn();
    render(<ReviewDueCard items={[item()]} onOpen={onOpen} />);

    await userEvent.click(screen.getByRole('button', { name: 'SQLite 全文搜索学习笔记' }));
    expect(onOpen).toHaveBeenCalledWith('note_1');
  });
});
