import { beforeEach, describe, expect, it } from 'vitest';

import {
  describeSearch,
  type RecentSearch,
  isMeaningfulSearch,
  loadRecent,
  loadSaved,
  newSavedId,
  pushRecent,
  sameSearch,
  saveRecent,
  saveSaved,
} from '@/lib/searchHistory';

const base = { keyword: '索引', categoryId: null, tagId: null, favoriteOnly: false };

describe('搜索条件判定与去重', () => {
  it('空条件不算有意义的搜索', () => {
    expect(isMeaningfulSearch({ keyword: '  ', categoryId: null, tagId: null, favoriteOnly: false })).toBe(false);
    expect(isMeaningfulSearch({ ...base })).toBe(true);
    expect(isMeaningfulSearch({ keyword: '', categoryId: 'cat', tagId: null, favoriteOnly: false })).toBe(true);
  });

  it('比较时忽略关键词首尾空格', () => {
    expect(sameSearch(base, { ...base, keyword: ' 索引 ' })).toBe(true);
    expect(sameSearch(base, { ...base, favoriteOnly: true })).toBe(false);
  });
});

describe('pushRecent', () => {
  it('新记录排在最前、按条件去重、按上限截断', () => {
    let list = pushRecent([], { ...base }, '2026-10-07T10:00:00+08:00');
    list = pushRecent(list, { ...base, keyword: '检索' }, '2026-10-07T10:01:00+08:00');
    list = pushRecent(list, { ...base }, '2026-10-07T10:02:00+08:00');
    expect(list).toHaveLength(2);
    expect(list[0]?.condition.keyword).toBe('索引');
    expect(list[0]?.at).toBe('2026-10-07T10:02:00+08:00');

    let many: RecentSearch[] = [];
    for (let index = 0; index < 12; index += 1) {
      many = pushRecent(many, { ...base, keyword: `词${index}` }, '2026-10-07T10:00:00+08:00', 5);
    }
    expect(many).toHaveLength(5);
  });

  it('空条件不写入历史', () => {
    expect(pushRecent([], { keyword: '', categoryId: null, tagId: null, favoriteOnly: false }, 'x')).toEqual([]);
  });
});

describe('持久化与描述', () => {
  beforeEach(() => localStorage.clear());

  it('按工作区读写历史与收藏', () => {
    saveRecent('ws_1', [{ condition: base, at: 'now' }]);
    saveSaved('ws_1', [{ ...base, id: 's1', name: '我的搜索', createdAt: 'now' }]);

    expect(loadRecent('ws_1')).toHaveLength(1);
    expect(loadSaved('ws_1')).toHaveLength(1);
    // 其它工作区不受影响
    expect(loadRecent('ws_2')).toEqual([]);
  });

  it('损坏数据不会抛错', () => {
    localStorage.setItem('pkos.search.recent.ws_1', '{不是 JSON');
    expect(loadRecent('ws_1')).toEqual([]);
  });

  it('描述包含关键词、分类、标签与收藏', () => {
    const text = describeSearch(
      { keyword: '索引', categoryId: 'cat', tagId: 'tag', favoriteOnly: true },
      { categoryName: '技术', tagName: 'sqlite' },
    );
    expect(text).toBe('“索引” · 分类：技术 · 标签：sqlite · 仅收藏');
    expect(describeSearch({ keyword: '', categoryId: null, tagId: null, favoriteOnly: false }, {})).toBe(
      '全部笔记',
    );
  });

  it('生成的收藏 id 唯一', () => {
    expect(newSavedId()).not.toBe(newSavedId());
  });
});
