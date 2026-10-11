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

  const trigger = useCallback(async () => {
    if (saving.current) {
      pending.current = true;
      return;
    }
    saving.current = true;
    try {
      await onSave();
    } finally {
      saving.current = false;
      if (pending.current) {
        pending.current = false;
        void trigger();
      }
    }
  }, [onSave]);

  useEffect(() => {
    if (!enabled || signature === null) return;
    if (lastSignature.current === null) {
      lastSignature.current = signature;
      return;
    }
    if (lastSignature.current === signature) return;
    lastSignature.current = signature;

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
    await trigger();
  }, [trigger]);

  /** 重新加载笔记后重置比对基线。 */
  const resetBaseline = (value: string | null) => {
    lastSignature.current = value;
  };

  return { saveNow, flush, resetBaseline };
}
