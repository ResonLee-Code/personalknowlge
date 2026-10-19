import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { TagEditor } from '@/components/editor/TagEditor';

describe('TagEditor 行内模式', () => {
  it('未编辑时只显示「标签」按钮，点开才出现输入框', async () => {
    const user = userEvent.setup();
    render(<TagEditor inline tags={[]} onChange={() => {}} />);

    expect(screen.queryByRole('textbox', { name: '添加标签' })).toBeNull();
    await user.click(screen.getByRole('button', { name: '添加标签' }));
    expect(screen.getByRole('textbox', { name: '添加标签' })).toHaveFocus();
  });

  it('输入后回车把标签交给 onChange', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<TagEditor inline tags={[]} onChange={onChange} />);

    await user.click(screen.getByRole('button', { name: '添加标签' }));
    await user.type(screen.getByRole('textbox', { name: '添加标签' }), '产品{Enter}');
    expect(onChange).toHaveBeenCalledWith(['产品']);
  });

  it('已有标签时仍可继续添加', async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<TagEditor inline tags={['产品']} onChange={onChange} />);

    expect(screen.getByText('产品')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '添加标签' }));
    await user.type(screen.getByRole('textbox', { name: '添加标签' }), '设计{Enter}');
    expect(onChange).toHaveBeenCalledWith(['产品', '设计']);
  });
});
