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

  /**
   * 回归（2026-10-11 真机验收，P0）：**没改动就不许落盘**。
   *
   * 以前 flush 无条件写：每切一次笔记就发一次保存请求。它把「切笔记的中间帧」放大成了
   * 数据风险——那一帧里编辑器状态可能还停在上一篇（甚至为空），把空内容写进别的笔记
   * （真机上四篇笔记的正文被写空）。
   */
  it('没有改动时 flush 不发保存请求（切笔记不再无条件落盘）', async () => {
    const onSave = vi.fn().mockResolvedValue(true);
    const { result } = renderHook(() =>
      useAutosave({ enabled: true, delay: 100, onSave, signature: 'a' }),
    );

    // 首次渲染只是建立基线：不算改动
    await result.current.flush();
    expect(onSave).not.toHaveBeenCalled();

    // 落盘过的内容再 flush 一次也不该重复写
    result.current.resetBaseline('a');
    await result.current.flush();
    expect(onSave).not.toHaveBeenCalled();
  });
});
