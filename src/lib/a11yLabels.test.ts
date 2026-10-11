import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

import { describe, expect, it } from 'vitest';

/**
 * 表单控件的程序化标签守卫（0 信任：屏幕阅读器读不出「只有 placeholder」的输入框）。
 *
 * 规则：`<Input>` / `<select>` / `<textarea>` 必须有下列之一——
 * 自己的 `id=`（配合 `<Label htmlFor>`）、`aria-label` / `aria-labelledby`，
 * 或者被 `<label>` / `<Label>` 元素**包裹**（隐式标签）。
 *
 * 为什么要测试：`placeholder` 不是标签——它对语音控制、屏幕阅读器都不可见；
 * 而这类缺失在肉眼与 E2E 里都看不出来（点得动、填得进）。
 */
const CONTROLS = new Set(['Input', 'select', 'textarea']);
const VOID_TAGS = new Set(['input', 'br', 'hr', 'img', 'meta', 'link']);

interface ScannedTag {
  closing: boolean;
  name: string;
  attrs: string;
  selfClosing: boolean;
  index: number;
}

/**
 * 手写标签扫描（比正则稳：能正确处理属性里的 `>` 与引号，也不会把 `a < b` 当标签）。
 */
function scanTags(code: string): ScannedTag[] {
  const tags: ScannedTag[] = [];
  const isNameChar = (ch: string) => /[A-Za-z0-9_.]/.test(ch);
  for (let index = 0; index < code.length; index += 1) {
    if (code[index] !== '<') continue;
    let cursor = index + 1;
    const closing = code[cursor] === '/';
    if (closing) cursor += 1;
    const nameStart = cursor;
    while (cursor < code.length && isNameChar(code[cursor]!)) cursor += 1;
    const name = code.slice(nameStart, cursor);
    if (!name) continue;
    // 标签名后必须是空白、`/` 或 `>`（否则是 `a < b` 这样的比较表达式）
    const next = code[cursor];
    if (next !== undefined && !/[\s/>]/.test(next)) continue;

    let quote: string | null = null;
    const attrsStart = cursor;
    while (cursor < code.length) {
      const ch = code[cursor]!;
      if (quote) {
        if (ch === quote) quote = null;
      } else if (ch === '"' || ch === "'") {
        quote = ch;
      } else if (ch === '>') {
        break;
      }
      cursor += 1;
    }
    const attrs = code.slice(attrsStart, cursor);
    tags.push({
      closing,
      name,
      attrs,
      selfClosing: attrs.trimEnd().endsWith('/'),
      index,
    });
    index = cursor;
  }
  return tags;
}

/** 收集所有 .tsx 源文件（跳过测试与 UI 基元本身——基元的标签由调用方负责）。 */
function tsxSources(root: string): string[] {
  const results: string[] = [];
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir)) {
      const absolute = path.join(dir, entry);
      if (statSync(absolute).isDirectory()) {
        walk(absolute);
        continue;
      }
      if (!entry.endsWith('.tsx') || entry.endsWith('.test.tsx')) continue;
      if (absolute.endsWith(path.join('ui', 'input.tsx'))) continue;
      results.push(absolute);
    }
  };
  walk(root);
  return results;
}

/** 找出缺少程序化标签的控件。 */
function findUnlabeledControls(file: string, source: string): string[] {
  const code = source.replace(/\/\*[\s\S]*?\*\//g, '');
  const problems: string[] = [];
  const stack: string[] = [];
  for (const tag of scanTags(code)) {
    const { closing, name, attrs, selfClosing, index } = tag;
    if (closing) {
      for (let index = stack.length - 1; index >= 0; index -= 1) {
        if (stack[index] === name) {
          stack.length = index;
          break;
        }
      }
      continue;
    }
    if (CONTROLS.has(name)) {
      const hasOwnLabel =
        /\baria-label/.test(attrs) || /\baria-labelledby/.test(attrs) || /\bid=/.test(attrs);
      const insideLabel = stack.some((entry) => entry === 'label' || entry === 'Label');
      if (!hasOwnLabel && !insideLabel) {
        const line = code.slice(0, index).split('\n').length;
        problems.push(
          `${path.relative(process.cwd(), file)}:${line} <${name}> ${attrs.trim().slice(0, 60)}`,
        );
      }
    }
    if (!selfClosing && !VOID_TAGS.has(name.toLowerCase())) {
      stack.push(name);
    }
  }
  return problems;
}

describe('表单控件的程序化标签', () => {
  it('每个 Input / select / textarea 都有 label、id 或 aria-label', () => {
    const root = path.resolve(process.cwd(), 'src');
    const files = tsxSources(root);
    expect(files.length).toBeGreaterThan(30);

    const problems = files.flatMap((file) => findUnlabeledControls(file, readFileSync(file, 'utf8')));
    expect(problems, `以下控件缺少程序化标签（placeholder 不算标签）：\n${problems.join('\n')}`).toEqual(
      [],
    );
  });
});
