import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { WINDOW_BOUNDS_KEY, useWindowBounds } from '@/hooks/useWindowBounds';
import { serializeWindowBounds } from '@/lib/windowBounds';

const win = {
  setSize: vi.fn(async () => undefined),
  setPosition: vi.fn(async () => undefined),
  outerPosition: vi.fn(async () => ({ x: 300, y: 120 })),
  outerSize: vi.fn(async () => ({ width: 1200, height: 800 })),
  // 签名写清楚：恢复测试里要手动触发一次回调
  onResized: vi.fn<(callback: () => void) => Promise<() => void>>(async () => () => undefined),
  onMoved: vi.fn<(callback: () => void) => Promise<() => void>>(async () => () => undefined),
};
const availableMonitors = vi.fn(async () => [
  { position: { x: 0, y: 0 }, size: { width: 1512, height: 982 } },
]);

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => win,
  availableMonitors: () => availableMonitors(),
  PhysicalSize: class {
    constructor(public width: number, public height: number) {}
  },
  PhysicalPosition: class {
    constructor(public x: number, public y: number) {}
  },
}));

vi.mock('@/services/api', () => ({ isTauriRuntime: () => true }));

/**
 * Track B4：关掉再开，窗口回到上次的尺寸与位置；位置已经不在屏幕上时只恢复尺寸。
 * 这里钉住的是「恢复动作」，因为它是唯一容易把窗口越恢复越偏的一步。
 */
describe('useWindowBounds', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    localStorage.clear();
    availableMonitors.mockResolvedValue([
      { position: { x: 0, y: 0 }, size: { width: 1512, height: 982 } },
    ]);
  });

  it('有记录且位置仍在屏幕上 → 尺寸与位置都恢复', async () => {
    localStorage.setItem(
      WINDOW_BOUNDS_KEY,
      serializeWindowBounds({ x: 200, y: 100, width: 1200, height: 800 }),
    );

    renderHook(() => useWindowBounds());

    await waitFor(() => expect(win.setSize).toHaveBeenCalled());
    expect(win.setSize).toHaveBeenCalledWith(expect.objectContaining({ width: 1200, height: 800 }));
    expect(win.setPosition).toHaveBeenCalledWith(expect.objectContaining({ x: 200, y: 100 }));
  });

  it('位置已经在屏幕外（例如外接屏拔掉了）→ 只恢复尺寸，不设置位置', async () => {
    localStorage.setItem(
      WINDOW_BOUNDS_KEY,
      serializeWindowBounds({ x: 3000, y: 100, width: 1200, height: 800 }),
    );

    renderHook(() => useWindowBounds());

    await waitFor(() => expect(win.setSize).toHaveBeenCalled());
    expect(win.setPosition).not.toHaveBeenCalled();
  });

  it('没有记录时不去动窗口（首次启动就按默认尺寸）', async () => {
    renderHook(() => useWindowBounds());
    await waitFor(() => expect(win.onResized).toHaveBeenCalled());
    expect(win.setSize).not.toHaveBeenCalled();
    expect(win.setPosition).not.toHaveBeenCalled();
  });

  it('恢复阶段收到 resize/move 事件不回写（否则会把恢复动作当成用户调整）', async () => {
    localStorage.setItem(
      WINDOW_BOUNDS_KEY,
      serializeWindowBounds({ x: 200, y: 100, width: 1200, height: 800 }),
    );
    // 录制回调，手动触发一次“恢复期间的 resize”
    let resizeCallback: (() => void) | null = null;
    win.onResized.mockImplementation(async (cb: () => void) => {
      resizeCallback = cb;
      return () => undefined;
    });

    renderHook(() => useWindowBounds());
    await waitFor(() => expect(resizeCallback).not.toBeNull());
    // 恢复已经结束（setSize/setPosition 都调用过），此后的事件才应该被记录
    resizeCallback!();
    await waitFor(() => expect(localStorage.getItem(WINDOW_BOUNDS_KEY)).not.toBeNull());
  });
});
