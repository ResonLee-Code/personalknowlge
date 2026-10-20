import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { describe, expect, it, vi } from 'vitest';

import { handleSelectionContextMenu } from './selectionContext';

describe('MarkdownEditor selection context menu', () => {
  it('选中文字时阻止原生菜单并交出当前选区文本', () => {
    const view = new EditorView({
      state: EditorState.create({
        doc: '选中的原文',
        selection: { anchor: 0, head: 5 },
      }),
    });
    const callback = vi.fn();
    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    try {
      expect(handleSelectionContextMenu(view, event, callback)).toBe(true);
      expect(event.defaultPrevented).toBe(true);
      expect(callback).toHaveBeenCalledWith(event, '选中的原文');
    } finally {
      view.destroy();
    }
  });

  it('没有选区时保留编辑器原生右键菜单', () => {
    const view = new EditorView({
      state: EditorState.create({ doc: '没有选区', selection: { anchor: 0 } }),
    });
    const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
    try {
      expect(handleSelectionContextMenu(view, event, vi.fn())).toBe(false);
      expect(event.defaultPrevented).toBe(false);
    } finally {
      view.destroy();
    }
  });
});
