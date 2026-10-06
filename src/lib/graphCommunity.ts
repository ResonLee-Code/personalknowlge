/**
 * 社区发现（V1.1.2 图谱增强）。
 *
 * 使用**模块度局部移动**（Louvain 第一阶段）：每个节点依次尝试移动到「让模块度增益最大」的
 * 邻居社区，直到不再有节点移动。比连通分量更贴近「哪几篇笔记抱团」的直觉
 * （两簇之间只有一条桥接链接时会正确分成两个社区）。
 *
 * 确定性：节点按 id 排序处理、增益并列时保留当前社区、最后按规模重新编号；
 * 因此同一张图每次得到同样的社区与配色。复杂度约 O(E) × 轮数。
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
export interface CommunityOptions {
  /** 分辨率：>1 倾向更多更小的社区，<1 倾向合并（默认 1） */
  resolution?: number;
  /** 最大轮数（默认 10） */
  maxPasses?: number;
}

export function detectCommunities(
  nodeIds: readonly string[],
  edges: readonly GraphEdge[],
  options: CommunityOptions = {},
): Map<string, number> {
  const resolution = options.resolution ?? 1;
  const maxPasses = options.maxPasses ?? 10;
  const order = [...nodeIds].sort((left, right) => left.localeCompare(right));

  // 邻接表（无向、带权；同一对节点的多条链接权重相加）
  const weights = new Map<string, Map<string, number>>();
  const degree = new Map<string, number>();
  order.forEach((id) => {
    weights.set(id, new Map());
    degree.set(id, 0);
  });
  let totalWeight = 0;
  for (const edge of edges) {
    if (!weights.has(edge.source) || !weights.has(edge.target)) continue;
    if (edge.source === edge.target) continue;
    const weight = Math.max(0, edge.weight);
    if (weight === 0) continue;
    const forward = weights.get(edge.source)!;
    const backward = weights.get(edge.target)!;
    forward.set(edge.target, (forward.get(edge.target) ?? 0) + weight);
    backward.set(edge.source, (backward.get(edge.source) ?? 0) + weight);
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + weight);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + weight);
    totalWeight += weight;
  }

  const labels = new Map<string, number>();
  order.forEach((id, index) => labels.set(id, index));
  const totals = new Map<number, number>();
  order.forEach((id) => {
    const label = labels.get(id)!;
    totals.set(label, (totals.get(label) ?? 0) + (degree.get(id) ?? 0));
  });

  // 没有边时每个节点自成一社区，直接返回（避免除零）
  if (totalWeight === 0) {
    const result = new Map<string, number>();
    order.forEach((id, index) => result.set(id, index));
    return result;
  }

  for (let pass = 0; pass < maxPasses; pass += 1) {
    let moved = false;
    for (const id of order) {
      const current = labels.get(id)!;
      const nodeDegree = degree.get(id) ?? 0;
      if (nodeDegree === 0) continue;

      // 先把自己从当前社区移除，再评估各候选社区
      totals.set(current, (totals.get(current) ?? 0) - nodeDegree);
      const weightToCommunity = new Map<number, number>();
      for (const [neighbour, weight] of weights.get(id)!) {
        const label = labels.get(neighbour);
        if (label === undefined) continue;
        weightToCommunity.set(label, (weightToCommunity.get(label) ?? 0) + weight);
      }

      let best = current;
      let bestGain = (weightToCommunity.get(current) ?? 0) - (resolution * (totals.get(current) ?? 0) * nodeDegree) / (2 * totalWeight);
      for (const [label, weight] of [...weightToCommunity.entries()].sort((a, b) => a[0] - b[0])) {
        const gain = weight - (resolution * (totals.get(label) ?? 0) * nodeDegree) / (2 * totalWeight);
        // 严格大于才迁移：并列时保留当前社区，保证结果稳定
        if (gain > bestGain + 1e-12) {
          best = label;
          bestGain = gain;
        }
      }

      labels.set(id, best);
      totals.set(best, (totals.get(best) ?? 0) + nodeDegree);
      if (best !== current) moved = true;
      // 清空空社区，避免 totals 里留下 0 影响后续比较
      if ((totals.get(current) ?? 0) <= 0) totals.delete(current);
    }
    if (!moved) break;
  }

  // 按社区规模（大 → 小）重新编号，规模相同时按最小成员 id 排序
  const groups = new Map<number, string[]>();
  for (const id of order) {
    const label = labels.get(id)!;
    if (!groups.has(label)) groups.set(label, []);
    groups.get(label)!.push(id);
  }
  const ordered = [...groups.values()].sort((left, right) => {
    if (right.length !== left.length) return right.length - left.length;
    return (left[0] ?? '').localeCompare(right[0] ?? '');
  });

  const result = new Map<string, number>();
  ordered.forEach((members, index) => {
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
