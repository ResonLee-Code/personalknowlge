import { describe, expect, it } from 'vitest';

import { codepointRangeToUtf16, codepointToUtf16 } from '@/lib/highlightRange';

/**
 * N5：后端给的是**码点**区间，CodeMirror 要的是 **UTF-16** 下标。
 * 这里把两者的换算钉住——含 emoji 时直接相加会错位，是本功能最容易踩的坑。
 */
describe('码点区间 → UTF-16 下标', () => {
  it('纯 BMP 文本（中文）两者一致', () => {
    const text = '倒排索引与分词';
    expect(codepointToUtf16(text, 0)).toBe(0);
    expect(codepointToUtf16(text, 2)).toBe(2);
    expect(codepointToUtf16(text, text.length)).toBe(text.length);
  });

  it('非 BMP 字符（emoji）会占两个码元，下标要跟着变', () => {
    const text = '第一🙂第二';
    // 码点：第0=第 第1=一 第2=🙂 第3=第 第4=二
    expect(codepointToUtf16(text, 2)).toBe(2); // emoji 之前
    expect(codepointToUtf16(text, 3)).toBe(4); // 跳过 emoji 的两个码元
    expect(codepointToUtf16(text, 4)).toBe(5);
  });

  it('区间换算保证 from ≤ to，且越界会夹住', () => {
    const text = 'a🙂b';
    const range = codepointRangeToUtf16(text, 1, 2); // 正好选中 emoji
    expect(range).toEqual({ from: 1, to: 3 });
    expect(text.slice(range.from, range.to)).toBe('🙂');

    expect(codepointRangeToUtf16(text, 99, 3)).toEqual({ from: 4, to: 4 });
    expect(codepointRangeToUtf16(text, -5, 1)).toEqual({ from: 0, to: 1 });
  });
});
