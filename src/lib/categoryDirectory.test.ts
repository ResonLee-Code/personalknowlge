import { describe, expect, it } from 'vitest';

import { flattenCategoriesWithDirectories } from '@/lib/categoryDirectory';
import type { Category, CategoryNode } from '@/types';

function category(id: string, name: string, dirName: string, parentId: string | null = null): Category {
  return {
    id,
    workspaceId: 'ws_1',
    parentId,
    name,
    sortOrder: 1,
    dirName,
    createdAt: '2026-10-11T10:00:00+08:00',
    noteCount: 0,
  };
}

function node(value: Category, children: CategoryNode[] = []): CategoryNode {
  return { category: value, depth: 0, children };
}

describe('flattenCategoriesWithDirectories（分类管理页的目录展示）', () => {
  it('子分类要拼出完整路径，而不是只显示单级目录名', () => {
    const tree = [
      node(category('cat_tech', '技术', '01_技术'), [
        node(category('cat_db', '数据库', '01_数据库', 'cat_tech'), [
          node(category('cat_index', '索引', '01_索引', 'cat_db')),
        ]),
      ]),
    ];

    const rows = flattenCategoriesWithDirectories(tree);
    expect(rows.map((row) => row.directory)).toEqual([
      '01_技术',
      '01_技术/01_数据库',
      '01_技术/01_数据库/01_索引',
    ]);
    // 顺序是深度优先（分类管理页按这个顺序渲染）
    expect(rows.map((row) => row.node.category.name)).toEqual(['技术', '数据库', '索引']);
  });

  it('没有目录的分类沿用父目录（笔记落在父目录里）', () => {
    const tree = [
      node(category('cat_tech', '技术', '01_技术'), [
        node(category('cat_virtual', '虚拟分组', '', 'cat_tech')),
      ]),
      node(category('cat_orphan', '没有目录的根分类', '')),
    ];

    const rows = flattenCategoriesWithDirectories(tree);
    expect(rows.map((row) => row.directory)).toEqual(['01_技术', '01_技术', '']);
  });

  it('空树返回空列表', () => {
    expect(flattenCategoriesWithDirectories([])).toEqual([]);
  });
});
