import { describe, expect, it } from 'vitest';

import { matchTitle } from '@/lib/fuzzy';

describe('matchTitle', () => {
  it('空查询匹配全部', () => {
    expect(matchTitle('任意标题', '')).toBe(true);
    expect(matchTitle('任意标题', '   ')).toBe(true);
  });

  it('支持子串与按顺序的模糊匹配', () => {
    expect(matchTitle('SQLite 全文搜索学习笔记', '全文')).toBe(true);
    expect(matchTitle('SQLite 全文搜索学习笔记', 'sqlite')).toBe(true);
    expect(matchTitle('索引设计笔记', '索笔')).toBe(true);
  });

  it('顺序不符时不匹配', () => {
    expect(matchTitle('索引设计笔记', '笔索')).toBe(false);
    expect(matchTitle('索引设计笔记', 'xyz')).toBe(false);
  });
});
