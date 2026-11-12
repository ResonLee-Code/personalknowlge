import { describe, expect, it } from 'vitest';

import { buildNavItems } from '@/components/layout/navItems';
import type { WorkspaceStats } from '@/types';

const stats: WorkspaceStats = {
  noteCount: 31,
  inboxCount: 2,
  trashCount: 1,
  categoryCount: 4,
  tagCount: 7,
  attachmentCount: 3,
  totalWords: 1200,
  updatedLast7d: 5,
  updatedLast30d: 12,
  pinnedCount: 2,
  activity: [],
};

describe('buildNavItems', () => {
  it('展开态与图标栏共用内容发现入口和统计计数', () => {
    const items = buildNavItems(stats);

    expect(items).toHaveLength(13);
    expect(items.find((item) => item.to === '/discover')).toMatchObject({ label: '内容发现' });
    expect(items.find((item) => item.to === '/notes')?.count).toBe(31);
    expect(items.find((item) => item.to === '/organize')?.label).toBe('整理建议');
  });
});
