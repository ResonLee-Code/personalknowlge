import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { MarkdownPreview } from '@/components/editor/MarkdownPreview';

describe('MarkdownPreview 双向链接', () => {
  it('把已解析的 [[标题]] 渲染成可点击 chip', async () => {
    const onOpenWikilink = vi.fn();
    render(
      <MarkdownPreview
        content="参考 [[索引设计|倒排索引]]。"
        resolveWikilink={() => 'note_target'}
        onOpenWikilink={onOpenWikilink}
      />,
    );

    const chip = screen.getByTestId('wikilink-chip');
    expect(chip).toHaveTextContent('倒排索引');
    expect(chip).toHaveAttribute('title', '打开「索引设计」');

    await userEvent.click(chip);
    expect(onOpenWikilink).toHaveBeenCalledWith('索引设计');
  });

  it('未解析的链接提示可创建', () => {
    render(<MarkdownPreview content="待补充 [[还没写的笔记]]。" resolveWikilink={() => null} />);

    const chip = screen.getByTestId('wikilink-chip');
    expect(chip).toHaveTextContent('还没写的笔记');
    expect(chip.getAttribute('title')).toContain('点击创建');
  });

  it('代码块里的 [[…]] 不渲染成 chip', () => {
    render(<MarkdownPreview content={'```\n[[不算链接]]\n```'} resolveWikilink={() => null} />);
    expect(screen.queryByTestId('wikilink-chip')).not.toBeInTheDocument();
  });

  it('没有正文时给出空状态', () => {
    render(<MarkdownPreview content="   " />);
    expect(screen.getByText('这篇笔记还没有正文')).toBeInTheDocument();
  });
});
