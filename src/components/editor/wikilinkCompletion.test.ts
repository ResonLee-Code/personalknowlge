import type { CompletionContext, CompletionResult } from '@codemirror/autocomplete';
import { describe, expect, it } from 'vitest';

import { wikilinkCompletionSource } from '@/components/editor/wikilinkCompletion';

/** 用最小的 matchBefore 替身模拟光标前的文本。 */
function contextBefore(text: string): CompletionContext {
  return {
    matchBefore(regex: RegExp) {
      const match = text.match(new RegExp(`${regex.source}$`));
      if (!match) return null;
      return { from: text.length - match[0].length, to: text.length, text: match[0] };
    },
  } as unknown as CompletionContext;
}

const titles = ['索引设计笔记', 'SQLite 全文搜索学习笔记', '模板与占位符'];

describe('wikilinkCompletionSource', () => {
  it('在 [[ 之后给出标题候选并补上 ]]', () => {
    const source = wikilinkCompletionSource(() => titles);
    const result = source(contextBefore('见 [[索引设')) as CompletionResult | null;

    expect(result).not.toBeNull();
    expect(result?.options.map((option) => option.label)).toEqual(['索引设计笔记']);
    expect(result?.options[0]?.apply).toBe('索引设计笔记]]');
  });

  it('输入 [[ 但没有关键词时列出全部标题', () => {
    const source = wikilinkCompletionSource(() => titles);
    const result = source(contextBefore('[[ ')) as CompletionResult | null;
    expect(result?.options).toHaveLength(titles.length);
  });

  it('不在 [[ 语境下不触发', () => {
    const source = wikilinkCompletionSource(() => titles);
    expect(source(contextBefore('普通文字 '))).toBeNull();
  });

  it('没有匹配项时不弹出', () => {
    const source = wikilinkCompletionSource(() => titles);
    expect(source(contextBefore('[[不存在的标题'))).toBeNull();
  });
});
