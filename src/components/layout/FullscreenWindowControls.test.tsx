import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { FullscreenWindowControls } from '@/components/layout/FullscreenWindowControls';

const tauri = vi.hoisted(() => ({
  win: {
    isFullscreen: vi.fn(),
    onResized: vi.fn(),
    close: vi.fn(),
    minimize: vi.fn(),
    setFullscreen: vi.fn(),
  },
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => tauri.win,
}));

vi.mock('@/services/api', () => ({
  isTauriRuntime: () => true,
}));

describe('FullscreenWindowControls', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    tauri.win.isFullscreen.mockResolvedValue(true);
    tauri.win.onResized.mockResolvedValue(() => {});
    tauri.win.close.mockResolvedValue(undefined);
    tauri.win.minimize.mockResolvedValue(undefined);
    tauri.win.setFullscreen.mockResolvedValue(undefined);
  });

  it('全屏时默认隐藏，鼠标扫过顶部才显示控制条', async () => {
    render(<FullscreenWindowControls />);
    const controls = await screen.findByTestId('fullscreen-window-controls');
    expect(controls.className).toContain('pointer-events-none');

    fireEvent.mouseMove(window, { clientY: 10 });
    await waitFor(() => expect(controls.className).not.toContain('pointer-events-none'));
    expect(controls).toHaveTextContent('Personal Knowledge OS');
  });

  it('三个按钮分别调用关闭 / 最小化 / 退出全屏', async () => {
    render(<FullscreenWindowControls />);
    await screen.findByTestId('fullscreen-window-controls');
    fireEvent.click(screen.getByRole('button', { name: '关闭窗口' }));
    fireEvent.click(screen.getByRole('button', { name: '最小化窗口' }));
    fireEvent.click(screen.getByRole('button', { name: '退出全屏' }));

    expect(tauri.win.close).toHaveBeenCalledTimes(1);
    expect(tauri.win.minimize).toHaveBeenCalledTimes(1);
    expect(tauri.win.setFullscreen).toHaveBeenCalledWith(false);
  });

  it('非全屏时不渲染任何浮层', async () => {
    tauri.win.isFullscreen.mockResolvedValue(false);
    render(<FullscreenWindowControls />);

    await waitFor(() => expect(tauri.win.isFullscreen).toHaveBeenCalled());
    expect(screen.queryByTestId('fullscreen-window-controls')).toBeNull();
  });
});
