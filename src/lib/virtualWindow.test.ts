import { describe, expect, it } from 'vitest';

import { computeVirtualWindow } from '@/lib/virtualWindow';

describe('computeVirtualWindow', () => {
  it('只渲染视口与 overscan 覆盖的行', () => {
    const window = computeVirtualWindow(100, 1_260, 630, 126, 2);
    expect(window.start).toBe(8);
    expect(window.end).toBe(18);
    expect(window.end - window.start).toBeLessThan(20);
    expect(window.totalHeight).toBe(12_600);
    expect(window.offsetTop).toBe(1_008);
  });

  it('滚动越界与空结果会夹取到合法范围', () => {
    expect(computeVirtualWindow(3, 99_999, 500, 120, 3).start).toBe(0);
    expect(computeVirtualWindow(3, 99_999, 500, 120, 3).end).toBe(3);
    expect(computeVirtualWindow(0, 100, 500, 120, 3)).toEqual({
      start: 0,
      end: 0,
      totalHeight: 0,
      offsetTop: 0,
    });
  });
});
