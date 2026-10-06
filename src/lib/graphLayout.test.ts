import { describe, expect, it } from 'vitest';

import {
  createLayout,
  isSettled,
  layoutBounds,
  nodeRadius,
  stepLayout,
} from '@/lib/graphLayout';
import type { GraphEdge, GraphNode } from '@/types';

function node(id: string, degree = 0): GraphNode {
  return {
    id,
    title: id,
    categoryId: null,
    updatedAt: '2026-10-07T00:00:00+08:00',
    degree,
    tags: [],
    orphan: degree === 0,
  };
}

function edge(source: string, target: string, weight = 1): GraphEdge {
  return { source, target, linkType: 'wikilink', weight };
}

const size = { width: 800, height: 600 };

describe('nodeRadius', () => {
  it('随度数次线性增长并保持正数', () => {
    expect(nodeRadius(0)).toBe(3);
    expect(nodeRadius(4)).toBeGreaterThan(nodeRadius(1));
    expect(nodeRadius(100)).toBeLessThan(30);
    expect(nodeRadius(-5)).toBe(3);
  });
});

describe('createLayout', () => {
  it('初始位置落在画布内且不重叠在同一点', () => {
    const nodes = [node('a'), node('b'), node('c', 2)];
    const state = createLayout(nodes, [edge('a', 'b')], size);

    expect(state.nodes).toHaveLength(3);
    const positions = new Set(state.nodes.map((item) => `${item.x.toFixed(2)},${item.y.toFixed(2)}`));
    expect(positions.size).toBe(3);
    for (const item of state.nodes) {
      expect(Number.isFinite(item.x)).toBe(true);
      expect(Number.isFinite(item.y)).toBe(true);
    }
  });

  it('对同样的输入给出同样的初始布局', () => {
    const nodes = [node('a'), node('b')];
    const first = createLayout(nodes, [], size).nodes.map((item) => [item.x, item.y]);
    const second = createLayout(nodes, [], size).nodes.map((item) => [item.x, item.y]);
    expect(first).toEqual(second);
  });
});

describe('stepLayout', () => {
  it('多次迭代后收敛并且坐标保持有限', () => {
    const nodes = [node('a', 3), node('b', 2), node('c', 1), node('d'), node('e')];
    const edges = [edge('a', 'b'), edge('a', 'c'), edge('b', 'd'), edge('c', 'e')];
    const state = createLayout(nodes, edges, size);

    let moving = true;
    let steps = 0;
    while (moving && steps < 600) {
      moving = stepLayout(state);
      steps += 1;
    }

    expect(steps).toBeLessThan(600);
    expect(isSettled(state)).toBe(true);
    for (const item of state.nodes) {
      expect(Number.isFinite(item.x)).toBe(true);
      expect(Number.isFinite(item.y)).toBe(true);
    }
  });

  it('有链接的节点比随机远处节点更靠近', () => {
    const nodes = [node('a'), node('b')];
    const state = createLayout(nodes, [edge('a', 'b')], size);
    const before = Math.hypot(
      state.nodes[0]!.x - state.nodes[1]!.x,
      state.nodes[0]!.y - state.nodes[1]!.y,
    );
    for (let step = 0; step < 200; step += 1) stepLayout(state);
    const after = Math.hypot(
      state.nodes[0]!.x - state.nodes[1]!.x,
      state.nodes[0]!.y - state.nodes[1]!.y,
    );

    expect(after).toBeLessThan(before);
    // 弹簧静止长度附近（96px）± 40%
    expect(after).toBeGreaterThan(50);
    expect(after).toBeLessThan(140);
  });

  it('固定节点在迭代中保持不动', () => {
    const nodes = [node('a'), node('b')];
    const state = createLayout(nodes, [edge('a', 'b')], size);
    state.fixed.add('a');
    const pinned = { x: state.nodes[0]!.x, y: state.nodes[0]!.y };
    for (let step = 0; step < 50; step += 1) stepLayout(state);
    expect(state.nodes[0]!.x).toBe(pinned.x);
    expect(state.nodes[0]!.y).toBe(pinned.y);
  });
});

describe('layoutBounds', () => {
  it('给出包含所有节点的包围盒', () => {
    const nodes = [node('a'), node('b')];
    const state = createLayout(nodes, [], size);
    const bounds = layoutBounds(state);
    for (const item of state.nodes) {
      expect(item.x - item.radius).toBeGreaterThanOrEqual(bounds.minX);
      expect(item.x + item.radius).toBeLessThanOrEqual(bounds.maxX);
    }
    expect(layoutBounds(createLayout([], [], size))).toEqual({
      minX: 0,
      minY: 0,
      maxX: 0,
      maxY: 0,
    });
  });
});
