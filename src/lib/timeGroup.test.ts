import { describe, expect, it } from 'vitest';

import { groupByRecency, recencyBucket } from '@/lib/timeGroup';

const now = new Date('2026-10-07T12:00:00');

describe('recencyBucket', () => {
  it('按今天 / 昨天 / 近 7 天 / 更早分桶', () => {
    expect(recencyBucket('2026-10-07T08:00:00', now)).toBe('today');
    expect(recencyBucket('2026-10-06T23:30:00', now)).toBe('yesterday');
    expect(recencyBucket('2026-10-03T10:00:00', now)).toBe('last7');
    expect(recencyBucket('2026-09-01T10:00:00', now)).toBe('earlier');
  });

  it('无效时间归入更早', () => {
    expect(recencyBucket(null, now)).toBe('earlier');
    expect(recencyBucket('不是时间', now)).toBe('earlier');
  });
});

describe('groupByRecency', () => {
  it('跳过空桶并保持桶内顺序', () => {
    const items = [
      { id: 'a', at: '2026-10-07T09:00:00' },
      { id: 'b', at: '2026-10-07T08:00:00' },
      { id: 'c', at: '2026-09-01T08:00:00' },
    ];
    const groups = groupByRecency(items, (item) => item.at, now);
    expect(groups.map((group) => group.label)).toEqual(['今天', '更早']);
    expect(groups[0]!.items.map((item) => item.id)).toEqual(['a', 'b']);
  });

  it('空列表返回空数组', () => {
    expect(groupByRecency([], () => null, now)).toEqual([]);
  });
});
