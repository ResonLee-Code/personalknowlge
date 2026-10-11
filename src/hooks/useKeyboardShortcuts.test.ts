import { renderHook } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useKeyboardShortcuts } from '@/hooks/useKeyboardShortcuts';

function press(key: string, options: KeyboardEventInit = {}) {
  const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...options });
  window.dispatchEvent(event);
  return event;
}

describe('useKeyboardShortcuts（全局快捷键）', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('⌘/Ctrl + K 与 ⌘/Ctrl + N 会调用对应处理器并阻止默认行为', () => {
    const onSearch = vi.fn();
    const onNewNote = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onSearch, onNewNote }));

    const search = press('k', { metaKey: true });
    expect(onSearch).toHaveBeenCalledTimes(1);
    expect(search.defaultPrevented).toBe(true);

    const created = press('n', { metaKey: true });
    expect(onNewNote).toHaveBeenCalledTimes(1);
    expect(created.defaultPrevented).toBe(true);

    // Ctrl 同样有效（Windows / Linux）
    press('k', { ctrlKey: true });
    expect(onSearch).toHaveBeenCalledTimes(2);
  });

  it('没有处理器的快捷键**不会**被吞掉（避免「按了没反应」）', () => {
    renderHook(() => useKeyboardShortcuts({ onSearch: vi.fn() }));

    // ⌘S / ⌘E 由编辑器页面自己实现：这一层没有处理器，就不该 preventDefault
    expect(press('s', { metaKey: true }).defaultPrevented).toBe(false);
    expect(press('e', { metaKey: true }).defaultPrevented).toBe(false);
    expect(press('w', { metaKey: true }).defaultPrevented).toBe(false);
  });

  it('提供了处理器时 ⌘S / ⌘E 正常工作', () => {
    const onSave = vi.fn();
    const onFocusEditor = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onSave, onFocusEditor }));

    expect(press('s', { metaKey: true }).defaultPrevented).toBe(true);
    expect(onSave).toHaveBeenCalledTimes(1);
    expect(press('e', { ctrlKey: true }).defaultPrevented).toBe(true);
    expect(onFocusEditor).toHaveBeenCalledTimes(1);
  });

  it('Delete 只在非输入状态下触发删除（避免在编辑器里误删笔记）', () => {
    const onDelete = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onDelete }));

    // 焦点在输入框里 → 不触发
    const input = document.createElement('input');
    document.body.appendChild(input);
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true }));
    expect(onDelete).not.toHaveBeenCalled();

    // 焦点在普通区域 → 触发
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete', bubbles: true }));
    expect(onDelete).toHaveBeenCalledTimes(1);
  });
});
