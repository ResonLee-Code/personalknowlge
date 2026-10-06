/** 图谱分类配色（V1.1 M3）：按 categoryId 哈希分配，同一分类恒定同色。 */

const PALETTE = [
  '#6366f1',
  '#0ea5e9',
  '#10b981',
  '#f59e0b',
  '#ef4444',
  '#8b5cf6',
  '#14b8a6',
  '#ec4899',
];

/** 未分类节点使用中性灰，避免与分类颜色混淆。 */
export const UNCATEGORIZED_COLOR = '#94a3b8';

export function categoryColor(categoryId?: string | null): string {
  if (!categoryId) return UNCATEGORIZED_COLOR;
  let hash = 0;
  for (let index = 0; index < categoryId.length; index += 1) {
    hash = (hash * 31 + categoryId.charCodeAt(index)) % 100000;
  }
  return PALETTE[hash % PALETTE.length]!;
}
