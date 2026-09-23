import { useEffect } from 'react';

interface ShortcutHandlers {
  onNewNote?: () => void;
  onSearch?: () => void;
  onSave?: () => void;
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

      if (meta && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        handlers.onSearch?.();
        return;
      }
      if (meta && event.key.toLowerCase() === 'n') {
        event.preventDefault();
        handlers.onNewNote?.();
        return;
      }
      if (meta && event.key.toLowerCase() === 's') {
        event.preventDefault();
        handlers.onSave?.();
        return;
      }
      if (meta && event.key.toLowerCase() === 'e') {
        event.preventDefault();
        handlers.onFocusEditor?.();
        return;
      }
      if (meta && event.key.toLowerCase() === 'w') {
        event.preventDefault();
        handlers.onCloseNote?.();
        return;
      }
      if (event.key === 'Delete' && !editable && !meta) {
        handlers.onDelete?.();
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [handlers]);
}
