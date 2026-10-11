import { describe, expect, it } from 'vitest';

import { editorContentBelongsTo } from '@/components/notes/editorContentGuard';

/**
 * 切笔记的中间帧防线：`detail` 已经换成新笔记、编辑器的 content/title 还停在上一篇。
 * 这时任何落盘都会把上一篇的内容写成新笔记的内容（真机上四篇笔记被写空的路径）。
 */
describe('编辑器内容归属守卫', () => {
  it('编辑器装载的就是当前这篇 → 允许保存', () => {
    expect(editorContentBelongsTo('note_a', 'note_a')).toBe(true);
  });

  it('编辑器还停在上一篇（切笔记的中间帧）→ 拒绝保存', () => {
    expect(editorContentBelongsTo('note_a', 'note_b')).toBe(false);
  });

  it('编辑器什么都没装载过 → 拒绝保存', () => {
    expect(editorContentBelongsTo(null, 'note_b')).toBe(false);
    expect(editorContentBelongsTo(null, null)).toBe(false);
  });
});
