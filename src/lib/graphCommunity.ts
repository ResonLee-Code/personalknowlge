/**
 * 社区发现（V1.1.1 图谱增强）。
 *
 * 定义：**连通分量** —— 互相可达的一组笔记算一个社区，孤立笔记各自成社区。
 * 用并查集实现，确定性（与输入顺序无关）、O(N+E)，对用户可直接解释
 * （「这一簇是互相引用的笔记，那几个是孤岛」）。
 * 更细的社区划分（模块度优化等）留待后续版本，函数签名不变。
 */

import type { GraphEdge } from '@/types';

/** 与分类配色同一套色板，避免两种着色模式视觉风格割裂。 */
const COMMUNITY_PALETTE = [
  '#6366f1',
  '#0ea5e9',
  '#10b981',
  '#f59e0b',
  '#ef4444',
  '#8b5cf6',
  '#14b8a6',
  '#ec4899',
  '#84cc16',
  '#f97316',
];

export const COMMUNITY_UNASSIGNED_COLOR = '#94a3b8';

export function communityColor(community: number | undefined): string {
  if (community === undefined || community < 0) return COMMUNITY_UNASSIGNED_COLOR;
  return COMMUNITY_PALETTE[community % COMMUNITY_PALETTE.length]!;
}

/**
 * 返回 `节点 id → 社区序号`（0 起，按社区规模从大到小编号）。
 * 孤立节点各自成为一个社区。
 */
export function detectCommunities(
  nodeIds: readonly string[],
  edges: readonly GraphEdge[],
): Map<string, number> {
  // 连通分量 + 并查集：确定性、O(N+E)，而且「互相可达的一组笔记」对用户是可直接解释的社区。
  // 更细的划分（模块度优化 / 标签传播）留待后续版本，接口保持不变。
  const parent = new Map<string, string>();
  const order = [...nodeIds].sort((left, right) => left.localeCompare(right));
  order.forEach((id) => parent.set(id, id));

  const find = (id: string): string => {
    let root = id;
    while (parent.get(root) !== root) {
      const next = parent.get(root);
      if (next === undefined) break;
      root = next;
    }
    // 路径压缩
    let cursor = id;
    while (parent.get(cursor) !== root) {
      const next = parent.get(cursor);
      if (next === undefined) break;
      parent.set(cursor, root);
      cursor = next;
    }
    return root;
  };

  for (const edge of edges) {
    if (!parent.has(edge.source) || !parent.has(edge.target)) continue;
    const left = find(edge.source);
    const right = find(edge.target);
    if (left !== right) {
      // 固定把较大 id 的根挂到较小 id 上，保证结果与输入顺序无关
      const [keep, merge] = left < right ? [left, right] : [right, left];
      parent.set(merge, keep);
    }
  }

  // 按分量规模（大 → 小）重新编号，规模相同时按最小成员 id 排序
  const groups = new Map<string, string[]>();
  for (const id of order) {
    const root = find(id);
    if (!groups.has(root)) groups.set(root, []);
    groups.get(root)!.push(id);
  }
  const ordered = [...groups.entries()].sort((left, right) => {
    if (right[1].length !== left[1].length) return right[1].length - left[1].length;
    return left[0].localeCompare(right[0]);
  });

  const result = new Map<string, number>();
  ordered.forEach(([, members], index) => {
    members.forEach((id) => result.set(id, index));
  });
  return result;
}

/** 统计各社区的节点数（用于图例）。 */
export function communitySizes(communities: Map<string, number>): Array<{ id: number; count: number }> {
  const sizes = new Map<number, number>();
  for (const community of communities.values()) {
    sizes.set(community, (sizes.get(community) ?? 0) + 1);
  }
  return [...sizes.entries()]
    .map(([id, count]) => ({ id, count }))
    .sort((left, right) => right.count - left.count || left.id - right.id);
}
