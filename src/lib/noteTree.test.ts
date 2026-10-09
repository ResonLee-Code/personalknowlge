import { describe, expect, it } from 'vitest';

import { buildNoteTree } from '@/lib/noteTree';
import type { NoteSummary } from '@/types';

function note(id: string, parentId: string | null = null, title = id): NoteSummary {
  return {
    id,
    title,
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

describe('buildNoteTree', () => {
  it('把子文档排在父文档之后并标记深度与子文档数量', () => {
    const rows = buildNoteTree([
      note('parent'),
      note('child', 'parent'),
      note('grandchild', 'child'),
      note('other'),
    ]);

    expect(rows.map((row) => [row.note.id, row.depth])).toEqual([
      ['parent', 0],
      ['child', 1],
      ['grandchild', 2],
      ['other', 0],
    ]);
    expect(rows[0]!.childCount).toBe(1);
    expect(rows[1]!.childCount).toBe(1);
    expect(rows[2]!.childCount).toBe(0);
  });

  it('父文档不在当前筛选结果里时，子文档按顶层展示', () => {
    const rows = buildNoteTree([note('child', 'parent-missing')]);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({ depth: 0, childCount: 0 });
  });

  it('折叠的父文档会隐藏其后代，但不隐藏自身', () => {
    const rows = buildNoteTree(
      [note('parent'), note('child', 'parent'), note('grandchild', 'child')],
      new Set(['parent']),
    );
    expect(rows.map((row) => row.note.id)).toEqual(['parent']);
    expect(rows[0]!.childCount).toBe(1);
  });

  it('frontmatter 手写出环时不死循环，且笔记不会丢失', () => {
    const rows = buildNoteTree([note('a', 'b'), note('b', 'a')]);
    expect(rows.map((row) => row.note.id).sort()).toEqual(['a', 'b']);
  });

  it('自引用的 parent 当作顶层处理', () => {
    const rows = buildNoteTree([note('self', 'self')]);
    expect(rows).toHaveLength(1);
    expect(rows[0]!.depth).toBe(0);
  });
});
