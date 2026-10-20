import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { MarkdownFormatBar } from '@/components/editor/MarkdownFormatBar';

describe('MarkdownFormatBar Ask 入口', () => {
  it('「就选中文字提问」调用编辑器提供的 Ask 动作', async () => {
    const onAskSelection = vi.fn();
    render(
      <MarkdownFormatBar
        onAskSelection={onAskSelection}
        onWrap={vi.fn()}
        onLinePrefix={vi.fn()}
        onHeading={vi.fn()}
        onIndent={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByRole('button', { name: '就选中文字提问' }));
    expect(onAskSelection).toHaveBeenCalledTimes(1);
  });

  it('「就选中文字调研」调用 Research 2.0 动作', async () => {
    const onResearchSelection = vi.fn();
    render(
      <MarkdownFormatBar
        onResearchSelection={onResearchSelection}
        onWrap={vi.fn()}
        onLinePrefix={vi.fn()}
        onHeading={vi.fn()}
        onIndent={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByRole('button', { name: '就选中文字调研' }));
    expect(onResearchSelection).toHaveBeenCalledTimes(1);
  });

  it('没有接入 Ask 动作时按钮置灰', () => {
    render(
      <MarkdownFormatBar
        onWrap={vi.fn()}
        onLinePrefix={vi.fn()}
        onHeading={vi.fn()}
        onIndent={vi.fn()}
      />,
    );
    expect(screen.getByRole('button', { name: '就选中文字提问' })).toBeDisabled();
  });
});
