import type { CategoryNode } from '@/types';

/** 一个分类在磁盘上的位置：分类节点 + **完整**相对目录路径。 */
export interface CategoryRow {
  node: CategoryNode;
  /** 相对工作区根目录的完整路径；空串表示「没有对应目录」（笔记直接放父目录） */
  directory: string;
}

/**
 * 把分类树压平成带**完整目录路径**的行（用于分类管理页展示与后续操作）。
 *
 * 为什么需要它：后端 `dirName` 只存**单级目录名**（完整路径由祖先链拼接，见
 * `CategoryLookup::directory_of`），所以「子分类显示为 `01_数据库`」会让人误以为它在根目录；
 * 这里沿树拼出 `01_技术/01_数据库` 才是它在磁盘上的真实位置。
 */
export function flattenCategoriesWithDirectories(tree: CategoryNode[]): CategoryRow[] {
  const rows: CategoryRow[] = [];
  const walk = (nodes: CategoryNode[], parentDirectory: string) => {
    nodes.forEach((node) => {
      const leaf = node.category.dirName?.trim() ?? '';
      const directory = leaf
        ? parentDirectory
          ? `${parentDirectory}/${leaf}`
          : leaf
        : parentDirectory; // 未创建目录的分类：笔记落在父目录里
      rows.push({ node, directory });
      walk(node.children, directory);
    });
  };
  walk(tree, '');
  return rows;
}
