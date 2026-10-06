import { describe, expect, it } from 'vitest';

import {
  parseWikilinkHref,
  parseWikilinks,
  preprocessWikilinks,
  wikilinkHref,
} from '@/lib/wikilink';

describe('parseWikilinks', () => {
  it('解析基本形式、别名与锚点，并给出起止位置', () => {
    const content = '见 [[索引设计]]、[[索引设计|倒排索引]] 与 [[索引设计#查询]]。';
    const tokens = parseWikilinks(content);

    expect(tokens).toHaveLength(3);
    expect(tokens[0]).toMatchObject({ target: '索引设计', alias: null, anchor: null });
    expect(tokens[1]).toMatchObject({ target: '索引设计', alias: '倒排索引' });
    expect(tokens[2]).toMatchObject({ target: '索引设计', anchor: '查询' });
    const first = tokens[0]!;
    expect(content.slice(first.start, first.end)).toBe('[[索引设计]]');
    expect(first.context).toContain('见 [[索引设计]]');
  });

  it('忽略围栏代码块与行内代码中的链接', () => {
    const content = ['正文 [[A]]', '```md', '[[B]]', '```', '行内 `[[C]]`'].join('\n');
    expect(parseWikilinks(content).map((token) => token.target)).toEqual(['A']);
  });

  it('不跨行，也不解析未闭合的链接', () => {
    expect(parseWikilinks('[[没有闭合\n第二行]]')).toHaveLength(0);
    expect(parseWikilinks('[[]]')).toHaveLength(0);
  });

  it('支持转义分隔符', () => {
    const tokens = parseWikilinks('[[A\\|B]]');
    expect(tokens[0]).toMatchObject({ target: 'A|B', alias: null });
  });
});

describe('chip 链接的生成与解析', () => {
  it('往返一致', () => {
    const href = wikilinkHref('索引 设计#1', '小节');
    expect(href.startsWith('#wikilink/')).toBe(true);
    expect(parseWikilinkHref(href)).toEqual({ target: '索引 设计#1', anchor: '小节' });
  });

  it('非 chip 链接返回 null', () => {
    expect(parseWikilinkHref('https://example.com')).toBeNull();
    expect(parseWikilinkHref(undefined)).toBeNull();
    expect(parseWikilinkHref('#wikilink/')).toBeNull();
  });
});

describe('preprocessWikilinks', () => {
  it('把 [[…]] 转成标准 Markdown 链接（保留别名）', () => {
    const output = preprocessWikilinks('见 [[索引设计|倒排索引]]。');
    expect(output).toMatch(/^见 \[倒排索引\]\(#wikilink\/.*\)。$/);
  });

  it('代码块中的链接保持不变', () => {
    const content = ['```', '[[不动]]', '```', '', '[[要替换]]'].join('\n');
    const output = preprocessWikilinks(content);
    expect(output).toContain('[[不动]]');
    expect(output).not.toContain('[[要替换]]');
    expect(output).toContain('#wikilink/');
  });

  it('没有链接时原样返回', () => {
    expect(preprocessWikilinks('普通正文')).toBe('普通正文');
  });
});
