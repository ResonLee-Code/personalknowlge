import { describe, expect, it } from 'vitest';

import { maskCodeRegions, parseOutline } from '@/lib/markdown';

describe('maskCodeRegions', () => {
  it('标记围栏代码块与行内代码', () => {
    const content = ['正文', '```md', '[[忽略]]', '```', '行内 `[[也忽略]]` 结束'].join('\n');
    const parsable = maskCodeRegions(content);
    const ignored = [...content].map((ch, index) => (parsable[index] ? '' : ch)).join('');

    expect(ignored).toContain('[[忽略]]');
    expect(ignored).toContain('[[也忽略]]');
    // 正文里的普通文字仍然可解析
    expect(parsable[0]).toBe(true);
  });

  it('未闭合的围栏会一直屏蔽到结尾', () => {
    const content = '正文\n```\n[[链接]]';
    const parsable = maskCodeRegions(content);
    expect(parsable[content.indexOf('[[链接]]')]).toBe(false);
  });
});

describe('parseOutline', () => {
  it('解析标题层级与偏移', () => {
    const content = ['# 一级标题', '', '正文', '', '## 二级标题', '### 三级标题'].join('\n');
    const headings = parseOutline(content);

    expect(headings.map((item) => item.level)).toEqual([1, 2, 3]);
    expect(headings.map((item) => item.text)).toEqual(['一级标题', '二级标题', '三级标题']);
    expect(headings[0]?.offset).toBe(0);
    expect(content.slice(headings[4 - 1]?.offset ?? 0)).toContain('### 三级标题');
  });

  it('忽略代码块中的 # 与没有空格的 #', () => {
    const content = ['# 真标题', '', '```sh', '# 注释不是标题', '```', '', '#没有空格'].join('\n');
    const headings = parseOutline(content);
    expect(headings).toHaveLength(1);
    expect(headings[0]?.text).toBe('真标题');
  });

  it('去掉标题末尾的 # 标记', () => {
    const headings = parseOutline('## 标题 ##');
    expect(headings[0]).toMatchObject({ level: 2, text: '标题' });
  });

  it('空正文返回空大纲', () => {
    expect(parseOutline('   \n')).toEqual([]);
  });
});
