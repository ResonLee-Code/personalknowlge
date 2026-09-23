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

  /** 重新加载笔记后重置比对基线。 */
  const resetBaseline = (value: string | null) => {
    lastSignature.current = value;
  };

  return { saveNow, resetBaseline };
}
