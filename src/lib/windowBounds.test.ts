import { describe, expect, it } from 'vitest';

import {
  isOnSomeMonitor,
  parseWindowBounds,
  planWindowRestore,
  serializeWindowBounds,
  type MonitorArea,
} from '@/lib/windowBounds';

const laptop: MonitorArea = { x: 0, y: 0, width: 1512, height: 982 };
const external: MonitorArea = { x: 1512, y: -400, width: 1920, height: 1080 };

describe('窗口尺寸/位置记忆（Track B4）', () => {
  it('合法值可以往返', () => {
    const raw = serializeWindowBounds({ x: 100.4, y: 33.6, width: 1380.2, height: 859.8 });
    const parsed = parseWindowBounds(raw);
    expect(parsed).toEqual({ x: 100, y: 34, width: 1380, height: 860 });
  });

  it('坏数据一律当作没有（宁可系统摆放，也不要恢复出一个坏窗口）', () => {
    expect(parseWindowBounds(null)).toBeNull();
    expect(parseWindowBounds('not json')).toBeNull();
    expect(parseWindowBounds('{}')).toBeNull();
    expect(parseWindowBounds('{"x":0,"y":0,"width":null,"height":800}')).toBeNull();
    expect(parseWindowBounds('{"x":0,"y":0,"width":400,"height":300}')).toBeNull(); // 太小
    expect(parseWindowBounds('{"x":999999,"y":0,"width":1200,"height":800}')).toBeNull(); // 离谱坐标
  });

  it('位置还在屏幕上 → 尺寸与位置都恢复', () => {
    const plan = planWindowRestore({ x: 200, y: 100, width: 1200, height: 800 }, [laptop]);
    expect(plan).toEqual({ size: [1200, 800], position: [200, 100] });
  });

  it('外接屏位置的窗口，在拔掉外接屏之后只恢复尺寸（不能把窗口扔到屏幕外）', () => {
    // 上一次在扩展屏上（x=1600），现在只有笔记本屏
    const plan = planWindowRestore({ x: 1600, y: 100, width: 1200, height: 800 }, [laptop]);
    expect(plan).toEqual({ size: [1200, 800], position: null });
    expect(isOnSomeMonitor({ x: 1600, y: 100, width: 1200, height: 800 }, [laptop])).toBe(false);
  });

  it('跨屏（有一部分在笔记本屏上）仍算可见', () => {
    const bounds = { x: 1400, y: 0, width: 1200, height: 800 };
    expect(isOnSomeMonitor(bounds, [laptop, external])).toBe(true);
    expect(planWindowRestore(bounds, [laptop, external])?.position).toEqual([1400, 0]);
  });

  it('拿不到显示器信息时不赌位置', () => {
    expect(planWindowRestore({ x: 10, y: 10, width: 1200, height: 800 }, [])).toEqual({
      size: [1200, 800],
      position: null,
    });
  });

  it('没有记录时什么都不恢复', () => {
    expect(planWindowRestore(null, [laptop])).toBeNull();
  });
});
