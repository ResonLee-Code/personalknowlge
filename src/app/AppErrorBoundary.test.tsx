import { render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { logAppFailure } from '@/lib/appLog';

import { AppErrorBoundary } from './AppErrorBoundary';

vi.mock('@/lib/appLog', () => ({
  logAppFailure: vi.fn().mockResolvedValue(undefined),
}));

function BrokenView(): never {
  throw new Error('render exploded');
}

describe('AppErrorBoundary', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('shows a recoverable diagnostic instead of a blank window and logs the render error', () => {
    vi.spyOn(console, 'error').mockImplementation(() => undefined);

    render(
      <AppErrorBoundary>
        <BrokenView />
      </AppErrorBoundary>,
    );

    expect(screen.getByRole('alert')).toHaveTextContent('界面加载失败');
    expect(screen.getByRole('alert')).toHaveTextContent('render exploded');
    expect(screen.getByRole('button', { name: '重新加载界面' })).toBeVisible();
    expect(logAppFailure).toHaveBeenCalledWith('render', expect.any(Error));
  });
});
