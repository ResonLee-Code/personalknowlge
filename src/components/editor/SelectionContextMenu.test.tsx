import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { SelectionContextMenu } from './SelectionContextMenu';

describe('SelectionContextMenu', () => {
  it('提供 Ask 与 Research 两个上下文动作', async () => {
    const onAsk = vi.fn();
    const onResearch = vi.fn();
    render(
      <SelectionContextMenu
        x={20}
        y={30}
        onAsk={onAsk}
        onResearch={onResearch}
        onClose={() => undefined}
      />,
    );

    await userEvent.click(screen.getByRole('menuitem', { name: /提问/ }));
    await userEvent.click(screen.getByRole('menuitem', { name: /调研/ }));
    expect(onAsk).toHaveBeenCalledTimes(1);
    expect(onResearch).toHaveBeenCalledTimes(1);
  });
});
