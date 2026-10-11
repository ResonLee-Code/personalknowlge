import { useEffect } from 'react';

interface ShortcutHandlers {
  onNewNote?: () => void;
  onSearch?: () => void;
  onSave?: () => void;
  onQuickSwitch?: () => void;
  onFocusEditor?: () => void;
  onCloseNote?: () => void;
  onDelete?: () => void;
}

function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toLowerCase();
  return tag === 'input' || tag === 'textarea' || target.isContentEditable;
}

/** 全局快捷键（设计文档 17.1）。 */
export function useKeyboardShortcuts(handlers: ShortcutHandlers) {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const meta = event.metaKey || event.ctrlKey;
      const editable = isEditableTarget(event.target);

      /**
       * 只有**确实有处理器**时才 `preventDefault`。
       *
       * 否则会出现「键被吞掉但什么也没发生」：⌘S / ⌘E 由编辑器页面自己实现
       * （`NoteEditorPage` 里另有一个监听器），AppShell 这一层没有处理器——
       * 若照样拦截，一旦编辑器换实现，用户按下去就变成彻底静默失效。
       */
      const handle = (handler: (() => void) | undefined) => {
        if (!handler) return false;
        event.preventDefault();
        handler();
        return true;
      };

      const key = event.key.toLowerCase();
      if (meta && key === 'k') {
        handle(handlers.onSearch);
        return;
      }
      if (meta && key === 'p') {
        handle(handlers.onQuickSwitch);
        return;
      }
      if (meta && key === 'n') {
        handle(handlers.onNewNote);
        return;
      }
      if (meta && key === 's') {
        handle(handlers.onSave);
        return;
      }
      if (meta && key === 'e') {
        handle(handlers.onFocusEditor);
        return;
      }
      if (meta && key === 'w') {
        handle(handlers.onCloseNote);
        return;
      }
      if (event.key === 'Delete' && !editable && !meta) {
        // Delete 没有默认行为要拦，只在有处理器时响应
        handlers.onDelete?.();
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [handlers]);
}
