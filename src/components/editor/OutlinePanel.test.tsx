import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { OutlinePanel } from '@/components/editor/OutlinePanel';

const headings = [
  { level: 1, text: '全文搜索', offset: 0 },
  { level: 2, text: '索引策略', offset: 20 },
  { level: 3, text: '查询策略', offset: 60 },
];

describe('OutlinePanel', () => {
  it('按层级列出标题并显示数量', () => {
    render(<OutlinePanel headings={headings} onJump={() => {}} />);

    expect(screen.getByText('大纲')).toBeInTheDocument();
    expect(screen.getByText('3')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '索引策略' })).toBeInTheDocument();
  });

  it('点击标题回调对应的偏移量', async () => {
    const onJump = vi.fn();
    render(<OutlinePanel headings={headings} onJump={onJump} />);

    await userEvent.click(screen.getByRole('button', { name: '查询策略' }));
    expect(onJump).toHaveBeenCalledWith(60);
  });

  it('没有标题时给出引导文案', () => {
    render(<OutlinePanel headings={[]} onJump={() => {}} />);
    expect(screen.getByText(/正文还没有标题/)).toBeInTheDocument();
  });

  it('钉住态提供「隐藏大纲」，悬浮态提供「钉住大纲」（对标语雀）', async () => {
    const onCollapse = vi.fn();
    const onPin = vi.fn();
    const { rerender } = render(
      <OutlinePanel headings={headings} onJump={() => {}} onCollapse={onCollapse} />,
    );

    await userEvent.click(screen.getByTestId('outline-collapse'));
    expect(onCollapse).toHaveBeenCalledTimes(1);
    expect(screen.queryByTestId('outline-pin')).not.toBeInTheDocument();

    rerender(
      <OutlinePanel headings={headings} onJump={() => {}} pinned={false} onPin={onPin} />,
    );
    await userEvent.click(screen.getByTestId('outline-pin'));
    expect(onPin).toHaveBeenCalledTimes(1);
  });
});
