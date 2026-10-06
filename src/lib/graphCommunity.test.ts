import { describe, expect, it } from 'vitest';

import { communityColor, communitySizes, detectCommunities } from '@/lib/graphCommunity';
import type { GraphEdge } from '@/types';

function edge(source: string, target: string, weight = 1): GraphEdge {
  return { source, target, linkType: 'wikilink', weight };
}

describe('detectCommunities', () => {
  it('两簇之间只有一条桥接链接时，仍然分成两个社区（模块度）', () => {
    const nodes = ['a1', 'a2', 'a3', 'b1', 'b2', 'b3'];
    const edges = [
      edge('a1', 'a2'),
      edge('a2', 'a3'),
      edge('a1', 'a3'),
      edge('b1', 'b2'),
      edge('b2', 'b3'),
      edge('b1', 'b3'),
      edge('a3', 'b1'),
    ];
    const communities = detectCommunities(nodes, edges);

    // 连通分量会把 6 个节点并成一簇，模块度不会：a3—b1 只是一条弱桥
    expect(communities.get('a1')).toBe(communities.get('a2'));
    expect(communities.get('b1')).toBe(communities.get('b2'));
    expect(communities.get('a1')).not.toBe(communities.get('b1'));
    expect(new Set(communities.values()).size).toBe(2);
  });

  it('孤立节点各自成社区，且结果稳定可复现', () => {
    const nodes = ['x', 'y', 'z'];
    const first = detectCommunities(nodes, [edge('x', 'y')]);
    const second = detectCommunities([...nodes].reverse(), [edge('y', 'x')]);

    expect(first.get('z')).not.toBe(first.get('x'));
    expect([...first.entries()].sort()).toEqual([...second.entries()].sort());
  });

  it('分辨率调高会把社区切得更细', () => {
    const nodes = ['a1', 'a2', 'a3', 'b1', 'b2', 'b3'];
    const edges = [
      edge('a1', 'a2'),
      edge('a2', 'a3'),
      edge('a1', 'a3'),
      edge('b1', 'b2'),
      edge('b2', 'b3'),
      edge('b1', 'b3'),
      edge('a3', 'b1'),
    ];
    const coarse = new Set(detectCommunities(nodes, edges, { resolution: 0.5 }).values()).size;
    const fine = new Set(detectCommunities(nodes, edges, { resolution: 2 }).values()).size;
    expect(fine).toBeGreaterThanOrEqual(coarse);
  });

  it('空图返回空映射，忽略不存在的端点', () => {
    expect(detectCommunities([], []).size).toBe(0);
    const communities = detectCommunities(['a'], [edge('a', 'ghost')]);
    expect(communities.get('a')).toBe(0);
  });
});

describe('communityColor / communitySizes', () => {
  it('按序号配色，未分配返回中性灰', () => {
    expect(communityColor(0)).toBe('#6366f1');
    expect(communityColor(undefined)).toBe('#94a3b8');
    // 超过色板长度时回绕，不会返回 undefined
    expect(communityColor(99)).toBeTruthy();
  });

  it('统计社区规模并按大小排序', () => {
    const communities = new Map([
      ['a', 0],
      ['b', 0],
      ['c', 1],
    ]);
    expect(communitySizes(communities)).toEqual([
      { id: 0, count: 2 },
      { id: 1, count: 1 },
    ]);
  });
});
