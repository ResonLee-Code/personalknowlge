import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it } from 'vitest';

import { OutlineToggleButton } from '@/components/editor/OutlineToggleButton';
import { useUiStore } from '@/stores/uiStore';

describe('OutlineToggleButton（列表栏头部的大纲开关）', () => {
  beforeEach(() => {
    useUiStore.setState({
      outlinePlacement: 'list',
      listPaneMode: 'list',
      outlineMode: 'pinned',
    });
  });

  it('没有选中笔记时置灰：那会儿没有大纲可看，点上去不该什么都不发生', async () => {
    render(<OutlineToggleButton hasNote={false} />);
    const button = screen.getByTestId('outline-toggle');
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute('title', '显示大纲（先选中一篇笔记）');

    await userEvent.click(button);
    expect(useUiStore.getState().listPaneMode).toBe('list');
  });

  it('位置在列表栏时，开关就是「列表 ⇄ 大纲」', async () => {
    render(<OutlineToggleButton hasNote />);
    await userEvent.click(screen.getByTestId('outline-toggle'));
    expect(useUiStore.getState().listPaneMode).toBe('outline');
    expect(screen.getByTestId('outline-toggle')).toHaveAttribute('aria-pressed', 'true');

    await userEvent.click(screen.getByTestId('outline-toggle'));
    expect(useUiStore.getState().listPaneMode).toBe('list');
  });

  it('位置在正文右侧时，开关是显示 / 隐藏右边那一列', async () => {
    useUiStore.setState({ outlinePlacement: 'side' });
    render(<OutlineToggleButton hasNote />);
    await userEvent.click(screen.getByTestId('outline-toggle'));
    expect(useUiStore.getState().outlineMode).toBe('hidden');

    await userEvent.click(screen.getByTestId('outline-toggle'));
    expect(useUiStore.getState().outlineMode).toBe('pinned');
  });
});
