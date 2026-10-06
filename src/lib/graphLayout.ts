/**
 * 力导向布局（V1.1 M3）。
 *
 * 不引入第三方可视化依赖：这里实现一个小而可控的模拟器 —
 * 网格近邻斥力（避免 O(n²)）+ 边弹簧 + 向心力，配合降温系数收敛。
 * 初始位置用黄金角螺旋，保证同样的输入得到同样的布局（便于测试与复现）。
 */

import type { GraphEdge, GraphNode } from '@/types';

/** 节点半径：度数越大越明显，但增长是次线性的，避免枢纽节点吞掉画面。 */
export function nodeRadius(degree: number): number {
  return 3 + Math.sqrt(Math.max(0, degree)) * 2.2;
}

export interface LayoutNode {
  id: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  degree: number;
  radius: number;
  /** 分类 id（决定节点颜色） */
  categoryId: string | null;
  orphan: boolean;
}

export interface LayoutState {
  nodes: LayoutNode[];
  edges: GraphEdge[];
  index: Map<string, number>;
  alpha: number;
  width: number;
  height: number;
  /** 被拖拽固定的节点（用户拖住时保持不动力） */
  fixed: Set<string>;
}

const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));
const CELL_SIZE = 96;
const REST_LENGTH = 96;
/** 低于该值视为已收敛（停止跑动画帧） */
const SETTLED_ALPHA = 0.02;
/** alpha 下限，避免除零式的极端刚性问题 */
const ALPHA_FLOOR = 0.008;
const DECAY = 0.975;

/** 建立初始布局（位置在画布中心附近螺旋展开）。 */
export function createLayout(
  nodes: GraphNode[],
  edges: GraphEdge[],
  size: { width: number; height: number },
): LayoutState {
  const centerX = size.width / 2;
  const centerY = size.height / 2;
  const scale = Math.min(size.width, size.height) * 0.3;
  const count = Math.max(1, nodes.length);

  const positioned: LayoutNode[] = nodes.map((node, index) => {
    const angle = index * GOLDEN_ANGLE;
    const spread = scale * Math.sqrt((index + 1) / count);
    return {
      id: node.id,
      x: centerX + Math.cos(angle) * spread,
      y: centerY + Math.sin(angle) * spread,
      vx: 0,
      vy: 0,
      degree: node.degree,
      radius: nodeRadius(node.degree),
      categoryId: node.categoryId ?? null,
      orphan: node.orphan,
    };
  });

  return {
    nodes: positioned,
    edges,
    index: new Map(positioned.map((node, i) => [node.id, i])),
    alpha: 1,
    width: size.width,
    height: size.height,
    fixed: new Set(),
  };
}

/** 是否仍在收敛（用于决定是否继续跑动画帧）。 */
export function isSettled(state: LayoutState): boolean {
  return state.alpha < SETTLED_ALPHA;
}

/** 用户交互后重新加热。 */
export function reheat(state: LayoutState, alpha = 0.6): void {
  state.alpha = Math.max(state.alpha, alpha);
}

/**
 * 推进一帧模拟。返回是否仍在运动。
 * 复杂度：斥力用网格近邻（邻域 3×3 格），边为 O(E)，整体近似 O(N + E)。
 */
export function stepLayout(state: LayoutState): boolean {
  const { nodes } = state;
  if (nodes.length === 0) return false;
  const alpha = state.alpha;

  // 1) 斥力：按网格分桶，只和同格与相邻格比较
  const grid = new Map<string, number[]>();
  nodes.forEach((node, position) => {
    const key = `${Math.floor(node.x / CELL_SIZE)},${Math.floor(node.y / CELL_SIZE)}`;
    const bucket = grid.get(key);
    if (bucket) bucket.push(position);
    else grid.set(key, [position]);
  });

  const ideal = CELL_SIZE * 1.6;
  for (const [key, bucket] of grid) {
    const [cellX, cellY] = key.split(',').map(Number);
    const baseX = cellX ?? 0;
    const baseY = cellY ?? 0;
    for (let offsetX = -1; offsetX <= 1; offsetX += 1) {
      for (let offsetY = -1; offsetY <= 1; offsetY += 1) {
        const other = grid.get(`${baseX + offsetX},${baseY + offsetY}`);
        if (!other) continue;
        // 只处理每个单元格对一次，避免重复计算
        if (offsetX < 0 || (offsetX === 0 && offsetY < 0)) continue;
        for (const i of bucket) {
          for (const j of other) {
            if (i >= j) continue;
            const left = nodes[i]!;
            const right = nodes[j]!;
            let dx = right.x - left.x;
            let dy = right.y - left.y;
            let distance = Math.hypot(dx, dy);
            if (distance < 0.01) {
              // 完全重合时按索引错开，避免除零
              dx = (j - i) * 0.5 + 0.5;
              dy = 0.5;
              distance = Math.hypot(dx, dy);
            }
            const force = ((ideal * ideal) / (distance * distance)) * 0.6 * alpha;
            const ux = dx / distance;
            const uy = dy / distance;
            left.vx -= ux * force;
            left.vy -= uy * force;
            right.vx += ux * force;
            right.vy += uy * force;
          }
        }
      }
    }
  }

  // 2) 弹簧：让有链接的节点靠近
  for (const edge of state.edges) {
    const sourceIndex = state.index.get(edge.source);
    const targetIndex = state.index.get(edge.target);
    if (sourceIndex === undefined || targetIndex === undefined) continue;
    const source = nodes[sourceIndex]!;
    const target = nodes[targetIndex]!;
    const dx = target.x - source.x;
    const dy = target.y - source.y;
    const distance = Math.hypot(dx, dy) || 0.01;
    const strength = 0.06 * Math.min(3, 1 + (edge.weight - 1) * 0.5) * alpha;
    const force = (distance - REST_LENGTH) * strength;
    const ux = dx / distance;
    const uy = dy / distance;
    source.vx += ux * force;
    source.vy += uy * force;
    target.vx -= ux * force;
    target.vy -= uy * force;
  }

  // 3) 向心 + 阻尼 + 积分
  const centerX = state.width / 2;
  const centerY = state.height / 2;
  for (const node of nodes) {
    if (state.fixed.has(node.id)) {
      node.vx = 0;
      node.vy = 0;
      continue;
    }
    node.vx += (centerX - node.x) * 0.004 * alpha;
    node.vy += (centerY - node.y) * 0.004 * alpha;
    node.vx *= 0.82;
    node.vy *= 0.82;
    // 限速：避免少数节点在低 alpha 时抖动
    const speed = Math.hypot(node.vx, node.vy);
    const limit = 24;
    if (speed > limit) {
      node.vx = (node.vx / speed) * limit;
      node.vy = (node.vy / speed) * limit;
    }
    node.x += node.vx;
    node.y += node.vy;
  }

  state.alpha = Math.max(ALPHA_FLOOR, state.alpha * DECAY);
  return !isSettled(state);
}

/** 布局的包围盒：用于「适应窗口」与偏移动画。 */
export function layoutBounds(state: LayoutState): {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
} {
  if (state.nodes.length === 0) return { minX: 0, minY: 0, maxX: 0, maxY: 0 };
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const node of state.nodes) {
    minX = Math.min(minX, node.x - node.radius);
    minY = Math.min(minY, node.y - node.radius);
    maxX = Math.max(maxX, node.x + node.radius);
    maxY = Math.max(maxY, node.y + node.radius);
  }
  return { minX, minY, maxX, maxY };
}
