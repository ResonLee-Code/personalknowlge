import { renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useAutosave } from '@/hooks/useAutosave';

describe('useAutosave（防抖自动保存 + 立即落盘）', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('签名变化后按延迟保存；重复调用不会叠加保存', async () => {
    const onSave = vi.fn().mockResolvedValue(true);
    const { rerender } = renderHook(
      ({ signature }) => useAutosave({ enabled: true, delay: 1000, onSave, signature }),
      { initialProps: { signature: 'a' } },
    );

    // 首次渲染只建立基线，不保存
    await vi.advanceTimersByTimeAsync(2000);
    expect(onSave).not.toHaveBeenCalled();

    rerender({ signature: 'b' });
    await vi.advanceTimersByTimeAsync(500);
    expect(onSave).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(600);
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it('flush 会取消防抖等待并立即落盘（切换 / 离开编辑器前用）', async () => {
    const onSave = vi.fn().mockResolvedValue(true);
    const { result, rerender } = renderHook(
      ({ signature }) => useAutosave({ enabled: true, delay: 5000, onSave, signature }),
      { initialProps: { signature: 'a' } },
    );

    rerender({ signature: 'b' });
    // 还远没到 5 秒的防抖窗口
    await result.current.flush();
    expect(onSave).toHaveBeenCalledTimes(1);

    // 已经落盘过，原来的防抖定时器不能再补一次
    await vi.advanceTimersByTimeAsync(6000);
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it('关闭自动保存时不会因为签名变化而写盘（但 flush 仍可手动触发）', async () => {
    const onSave = vi.fn().mockResolvedValue(true);
    const { result, rerender } = renderHook(
      ({ signature }) => useAutosave({ enabled: false, delay: 100, onSave, signature }),
      { initialProps: { signature: 'a' } },
    );

    rerender({ signature: 'b' });
    await vi.advanceTimersByTimeAsync(500);
    expect(onSave).not.toHaveBeenCalled();

    await result.current.flush();
    expect(onSave).toHaveBeenCalledTimes(1);
  });
});
