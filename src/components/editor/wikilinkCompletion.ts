import type { CompletionContext, CompletionResult } from '@codemirror/autocomplete';

/**
 * `[[` 触发的标题补全（V1.1 M2-2）。
 *
 * 命中规则：光标前有未闭合的 `[[`（同一行内），候选为传入的笔记标题；
 * 选择后补上 `]]`，因此输入 `[[索` 回车即可得到 `[[索引设计笔记]]`。
 */
export function wikilinkCompletionSource(getTitles: () => readonly string[]) {
  return (context: CompletionContext): CompletionResult | null => {
    const before = context.matchBefore(/\[\[[^\]\n]*/);
    if (!before) return null;

    const query = before.text.slice(2).trim().toLowerCase();
    const titles = getTitles();
    const matches = titles
      .filter((title) => !query || title.toLowerCase().includes(query))
      // 前缀命中优先，其余按标题排序，保证「输入两三个字」即可命中目标
      .sort((left, right) => {
        const leftPrefix = left.toLowerCase().startsWith(query) ? 0 : 1;
        const rightPrefix = right.toLowerCase().startsWith(query) ? 0 : 1;
        return leftPrefix - rightPrefix || left.localeCompare(right, 'zh-CN');
      })
      .slice(0, 20);
    if (matches.length === 0) return null;

    return {
      from: before.from + 2,
      options: matches.map((title) => ({
        label: title,
        apply: `${title}]]`,
        detail: '双向链接',
      })),
      // 继续输入时无需重新触发补全，直到遇到 ] 或换行
      validFor: /^[^\]\n]*$/,
    };
  };
}
