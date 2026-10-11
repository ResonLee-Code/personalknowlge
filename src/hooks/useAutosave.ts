import { useCallback, useEffect, useRef } from 'react';

interface AutosaveOptions {
  enabled: boolean;
  delay: number;
  onSave: () => Promise<boolean>;
  /** 变化标识：内容或哈希变化时触发保存 */
  signature: string | null;
}

// 自动保存（设计文档 FR-N04）：
// 防抖、串行、保存失败交由界面提示，不静默重试。
export function useAutosave({ enabled, delay, onSave, signature }: AutosaveOptions) {
  const timer = useRef<number | null>(null);
  const saving = useRef(false);
  const pending = useRef(false);
  const lastSignature = useRef<string | null>(null);
  /**
   * 「有改动还没落盘」。
   *
   * 只看签名差不够：签名一变，下面的 effect 就把 `lastSignature` 更新成新值并排好定时器，
   * 等到 `flush()` 时两者已经相等了。用独立的标记才能既保证「切走前一定落盘」，
   * 又做到「没改动就不发保存请求」。
   */
  const dirty = useRef(false);

  const trigger = useCallback(async () => {
    if (saving.current) {
      pending.current = true;
      return;
    }
    saving.current = true;
    try {
      await onSave();
      dirty.current = false;
    } finally {
      saving.current = false;
      if (pending.current) {
        pending.current = false;
        void trigger();
      }
    }
  }, [onSave]);

  useEffect(() => {
    if (signature === null) return;
    if (lastSignature.current === null) {
      lastSignature.current = signature;
      return;
    }
    if (lastSignature.current === signature) return;
    lastSignature.current = signature;
    // 「有改动」与「是否开启自动保存」无关：关掉自动保存时改动仍要能在
    // 切笔记 / 离开编辑器 / ⌘S 时落盘（flush 只认这个标记）。
    dirty.current = true;
    if (!enabled) return;

    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      void trigger();
    }, delay);

    return () => {
      if (timer.current) window.clearTimeout(timer.current);
    };
  }, [signature, enabled, delay, trigger]);

  const saveNow = () => trigger();

  /**
   * 立即落盘（取消防抖等待）。
   *
   * 用途：切换笔记 / 离开编辑器之前——否则「刚敲的字」还停在防抖窗口里，
   * 组件一旦卸载或内容被下一篇覆盖，这些改动就**静默丢失**了。
   * 无改动时调用也安全（`onSave` 自身会判断是否真的需要写）。
   */
  const flush = useCallback(async () => {
    if (timer.current !== null) {
      window.clearTimeout(timer.current);
      timer.current = null;
    }
    // **没有改动就不写**（2026-10-11 真机验收）。
    //
    // 以前 flush 是无条件落盘的：每次切笔记 / 离开编辑器都会发一次保存请求。
    // 这本身无害，但它把「切笔记的中间帧」放大了——那一帧里编辑器的 content/title 可能还停在
    // 上一篇（极端情况下为空），无条件保存就可能把空内容写进别的笔记（真机上四篇笔记被写空）。
    if (!dirty.current) return;
    await trigger();
  }, [trigger]);

  /** 重新加载笔记后重置比对基线。 */
  const resetBaseline = (value: string | null) => {
    lastSignature.current = value;
    dirty.current = false;
  };

  return { saveNow, flush, resetBaseline };
}
