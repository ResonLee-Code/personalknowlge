import { describe, expect, it } from 'vitest';

import { diffLines, diffStats } from '@/lib/diff';

describe('diffLines', () => {
  it('相同文本没有增删', () => {
    const rows = diffLines('第一行\n第二行', '第一行\n第二行');
    expect(rows.every((row) => row.kind === 'same')).toBe(true);
    expect(diffStats(rows)).toEqual({ added: 0, removed: 0, unchanged: 2 });
  });

  it('识别新增与删除，并给出两侧行号', () => {
    const rows = diffLines('A\nB\nC', 'A\nC\nD');
    expect(diffStats(rows)).toEqual({ added: 1, removed: 1, unchanged: 2 });

    const removed = rows.find((row) => row.kind === 'remove');
    expect(removed).toMatchObject({ text: 'B', leftNo: 2, rightNo: null });
    const added = rows.find((row) => row.kind === 'add');
    expect(added).toMatchObject({ text: 'D', leftNo: null, rightNo: 3 });
    // 相同行两侧都有行号
    expect(rows.find((row) => row.text === 'C')).toMatchObject({ leftNo: 3, rightNo: 2 });
  });

  it('从头写入的文档：整体作为新增', () => {
    const rows = diffLines('', '# 标题\n\n正文');
    expect(diffStats(rows)).toEqual({ added: 3, removed: 0, unchanged: 0 });
  });

  it('忽略行尾差异与 CRLF，末行空串不算一行', () => {
    expect(diffLines('A\nB\n', 'A\r\nB').every((row) => row.kind === 'same')).toBe(true);
  });

  it('超大文本被截断，避免 O(n·m) 卡死', () => {
    const huge = Array.from({ length: 5000 }, (_, index) => `第 ${index} 行`).join('\n');
    const rows = diffLines(huge, huge);
    expect(rows.length).toBe(4000);
  });
});
