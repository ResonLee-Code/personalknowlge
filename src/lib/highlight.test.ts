import { describe, expect, it } from 'vitest';

import { containsKeyword, markdownPreview, splitHighlight } from './highlight';

describe('splitHighlight', () => {
  it('returns a single unmatched segment when there is no term', () => {
    expect(splitHighlight('普通文本', [])).toEqual([{ text: '普通文本', matched: false }]);
  });

  it('marks matched parts of the text', () => {
    const segments = splitHighlight('SQLite 全文搜索笔记', ['全文搜索']);
    expect(segments).toEqual([
      { text: 'SQLite ', matched: false },
      { text: '全文搜索', matched: true },
      { text: '笔记', matched: false },
    ]);
  });

  it('merges overlapping ranges', () => {
    const segments = splitHighlight('abcdef', ['abc', 'bcd']);
    const matched = segments
      .filter((segment) => segment.matched)
      .map((segment) => segment.text)
      .join('');
    expect(matched).toBe('abcd');
  });

  it('is case insensitive', () => {
    const segments = splitHighlight('SQLite', ['sqlite']);
    expect(segments.some((segment) => segment.matched)).toBe(true);
  });
});

describe('markdownPreview', () => {
  it('strips frontmatter, syntax and code fences', () => {
    const preview = markdownPreview(
      '---\nid: note_1\n---\n\n# 标题\n\n正文 **加粗** 与 `代码`\n\n```js\nconst a = 1;\n```\n',
    );
    expect(preview).not.toContain('note_1');
    expect(preview).toContain('标题');
    expect(preview).toContain('加粗');
    expect(preview).not.toContain('```');
  });

  it('truncates long text', () => {
    const preview = markdownPreview('字'.repeat(200), 20);
    expect(preview.length).toBeLessThanOrEqual(21);
    expect(preview.endsWith('…')).toBe(true);
  });
});

describe('containsKeyword', () => {
  it('ignores empty keywords', () => {
    expect(containsKeyword('abc', '  ')).toBe(false);
    expect(containsKeyword('abc', 'b')).toBe(true);
  });
});
