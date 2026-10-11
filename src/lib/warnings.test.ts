import { describe, expect, it } from 'vitest';

import { formatWarningsToast } from '@/lib/warnings';

describe('formatWarningsToast', () => {
  it('没有提醒时返回 null（调用方据此不发提示，避免噪声）', () => {
    expect(formatWarningsToast([])).toBeNull();
    expect(formatWarningsToast(undefined)).toBeNull();
    expect(formatWarningsToast(null)).toBeNull();
    expect(formatWarningsToast(['   ', ''])).toBeNull();
  });

  it('少量提醒直接列出', () => {
    expect(formatWarningsToast(['跳过无法读取的文件 a.md'])).toBe(
      '注意：跳过无法读取的文件 a.md',
    );
    expect(formatWarningsToast(['a', 'b'])).toBe('注意：a；b');
  });

  it('超过上限时用数量带过，避免提示刷屏', () => {
    expect(formatWarningsToast(['a', 'b', 'c', 'd'])).toBe('注意（共 4 条）：a；b 等');
  });
});
