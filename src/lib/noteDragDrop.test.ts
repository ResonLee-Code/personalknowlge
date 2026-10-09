import { describe, expect, it } from 'vitest';

import { noteDropPosition, planNoteDrop } from '@/lib/noteDragDrop';
import type { NoteSummary } from '@/types';

function note(id: string, parentId: string | null = null): NoteSummary {
  return {
    id,
    title: id,
    parentId,
    filePath: `${id}.md`,
    categoryPath: 'Inbox',
    status: 'active',
    favorite: false,
    wordCount: 1,
    createdAt: '2026-10-01T00:00:00+08:00',
    updatedAt: '2026-10-01T00:00:00+08:00',
    tags: [],
  };
}

describe('noteDropPosition', () => {
  it('上 30% 插到前面、下 30% 插到后面、中间成为子文档', () => {
    expect(noteDropPosition(2, 40)).toBe('before');
    expect(noteDropPosition(20, 40)).toBe('inside');
    expect(noteDropPosition(38, 40)).toBe('after');
  });

  it('高度非法时退化为「成为子文档」', () => {
    expect(noteDropPosition(1, 0)).toBe('inside');
  });
});

describe('planNoteDrop', () => {
  const items = [note('a'), note('b'), note('c'), note('child', 'a')];

  it('拖到条目中间 = 成为它的子文档，并追加到子文档末尾', () => {
    const plan = planNoteDrop({ items, targetId: 'a', draggedIds: ['b'], position: 'inside' });
    expect(plan).toMatchObject({ parentId: 'a', changesParent: true, fromParentId: null });
    // a 的原有子文档 child 在前，新来的 b 追加在后
    expect(plan!.siblingIds).toEqual(['child', 'b']);
  });

  it('拖到条目上/下边缘 = 同层插入，顺序写入 order', () => {
    const before = planNoteDrop({ items, targetId: 'c', draggedIds: ['a'], position: 'before' });
    expect(before!.parentId).toBeNull();
    // 把 a 拖到 c 前面：从原位置摘掉 a 后是 [b, c]，再插到 c 前 → [b, a, c]
    expect(before!.siblingIds).toEqual(['b', 'a', 'c']);
    expect(before!.changesParent).toBe(false);

    const after = planNoteDrop({ items, targetId: 'a', draggedIds: ['b'], position: 'after' });
    expect(after!.siblingIds).toEqual(['a', 'b', 'c']);
  });

  it('多选拖拽保持相对顺序', () => {
    const plan = planNoteDrop({ items, targetId: 'c', draggedIds: ['a', 'b'], position: 'after' });
    expect(plan!.siblingIds).toEqual(['c', 'a', 'b']);
  });

  it('不允许拖进自己或自己的子树', () => {
    expect(planNoteDrop({ items, targetId: 'a', draggedIds: ['a'], position: 'inside' })).toBeNull();
    expect(planNoteDrop({ items, targetId: 'child', draggedIds: ['a'], position: 'inside' })).toBeNull();
    expect(planNoteDrop({ items, targetId: 'child', draggedIds: ['a'], position: 'before' })).toBeNull();
  });

  it('目标不存在或没有有效拖拽项时返回 null', () => {
    expect(planNoteDrop({ items, targetId: 'ghost', draggedIds: ['a'], position: 'inside' })).toBeNull();
    expect(planNoteDrop({ items, targetId: 'a', draggedIds: [], position: 'inside' })).toBeNull();
  });
});
