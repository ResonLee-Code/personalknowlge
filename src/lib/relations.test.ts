import { describe, expect, it } from 'vitest';

import { relationBadge, relationHighlights, RELATION_TYPES, RELATION_TYPE_LABELS } from '@/lib/relations';

describe('relationBadge', () => {
  it('覆盖全部关系类型', () => {
    for (const type of RELATION_TYPES) {
      const badge = relationBadge(type);
      expect(badge.label).toBe(RELATION_TYPE_LABELS[type]);
      expect(badge.label.length).toBeGreaterThan(0);
    }
  });

  it('大小写、空格与短横线都能归一', () => {
    expect(relationBadge('supersedes').label).toBe('取代');
    expect(relationBadge('depends-on').label).toBe('依赖');
    expect(relationBadge('part of').label).toBe('属于');
  });

  it('取代用琥珀色、冲突用红色、未知类型原样显示', () => {
    expect(relationBadge('SUPERSEDES').tone).toBe('amber');
    expect(relationBadge('CONTRADICTS').tone).toBe('red');
    expect(relationBadge('CUSTOM_TYPE').label).toBe('CUSTOM_TYPE');
  });
});

describe('relationHighlights', () => {
  it('取出被取代、取代他人与冲突数量', () => {
    const outgoing = [
      { id: '1', relationType: 'SUPERSEDES' },
      { id: '2', relationType: 'CONTRADICTS' },
    ];
    const incoming = [
      { id: '3', relationType: 'SUPERSEDES' },
      { id: '4', relationType: 'CONTRADICTS' },
    ];
    const result = relationHighlights(outgoing, incoming);
    expect(result.supersedes?.id).toBe('1');
    expect(result.supersededBy?.id).toBe('3');
    expect(result.conflictCount).toBe(2);
  });

  it('没有相关关系时返回空', () => {
    const result = relationHighlights([], []);
    expect(result.supersedes).toBeNull();
    expect(result.supersededBy).toBeNull();
    expect(result.conflictCount).toBe(0);
  });
});
