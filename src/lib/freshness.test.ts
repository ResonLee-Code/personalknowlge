import { describe, expect, it } from 'vitest';

import {
  freshnessState,
  parseValidUntil,
  reliabilityStars,
  sourceTypeLabel,
} from '@/lib/freshness';

const now = new Date('2026-10-07T12:00:00');

describe('parseValidUntil', () => {
  it('支持日期与完整 ISO', () => {
    expect(parseValidUntil('2026-12-31')?.getFullYear()).toBe(2026);
    expect(parseValidUntil('2026-12-31T08:00:00Z')).not.toBeNull();
    expect(parseValidUntil('')).toBeNull();
    expect(parseValidUntil('不是日期')).toBeNull();
  });

  it('纯日期按当天结束计算', () => {
    const until = parseValidUntil('2026-10-07')!;
    expect(until.getHours()).toBe(23);
    expect(until.getMinutes()).toBe(59);
  });
});

describe('freshnessState', () => {
  it('识别过期 / 即将到期 / 有效 / 未设置', () => {
    expect(freshnessState('2026-09-30', now)).toBe('expired');
    expect(freshnessState('2026-10-20', now)).toBe('expiring');
    expect(freshnessState('2027-01-01', now)).toBe('valid');
    expect(freshnessState(null, now)).toBe('unknown');
  });

  it('当天到期仍算有效（按 23:59 计算）', () => {
    expect(freshnessState('2026-10-07', now)).toBe('expiring');
  });
});

describe('reliabilityStars / sourceTypeLabel', () => {
  it('星标数量与等级一致并夹在 1–5', () => {
    expect(reliabilityStars(4)).toBe('★★★★☆');
    expect(reliabilityStars(9)).toBe('★★★★★');
    expect(reliabilityStars(0)).toBe('');
    expect(reliabilityStars(null)).toBe('');
  });

  it('来源类型展示中文名，未知类型原样显示', () => {
    expect(sourceTypeLabel('official')).toBe('官方文档');
    expect(sourceTypeLabel('OFFICIAL')).toBe('官方文档');
    expect(sourceTypeLabel('自定义来源')).toBe('自定义来源');
    expect(sourceTypeLabel('')).toBe('');
  });
});
