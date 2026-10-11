import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { SHORTCUTS } from '@/lib/constants';

/**
 * 快捷键「实现 ↔ 用户可见列表」的契约。
 *
 * 应用内「关于」页展示的是 `SHORTCUTS`，而真正生效的是 `useKeyboardShortcuts` 里的分支。
 * 两者漂移的后果很直接：**实现有、列表没有 → 用户不知道这条能用**
 * （⌘P 快速切换就漏过一次）；列表有、实现没有 → 用户按了没反应。
 *
 * 这里直接解析 hook 源码里的按键分支，与 `SHORTCUTS` 做集合比较。
 */
const hookPath = resolve(__dirname, '../hooks/useKeyboardShortcuts.ts');

function handledCombos(): string[] {
  const source = readFileSync(hookPath, 'utf8');
  const combos: string[] = [];
  // `if (meta && key === 'k')` 这类分支
  for (const match of source.matchAll(/if\s*\(meta && key === '([a-z])'\)/g)) {
    combos.push(`⌘/Ctrl + ${match[1]!.toUpperCase()}`);
  }
  // `event.key === 'Delete'`（不带修饰键）
  if (/event\.key === 'Delete'/.test(source)) combos.push('Delete');
  return combos;
}

describe('快捷键契约（关于页列表 ↔ 实现）', () => {
  it('解析出的实现分支不能为空（防止解析器坏掉造成假绿）', () => {
    expect(handledCombos().length).toBeGreaterThanOrEqual(5);
  });

  it('列表与实现完全一致：实现的每条都要能查到，列表里的每条也都真的生效', () => {
    const handled = new Set(handledCombos());
    const listed = new Set<string>(SHORTCUTS.map((shortcut) => shortcut.keys as string));

    const missingInHelp = [...handled].filter((combo) => !listed.has(combo));
    const notImplemented = [...listed].filter((combo) => !handled.has(combo));

    expect(
      missingInHelp,
      `这些快捷键已经实现，但「关于」页的列表没写（用户发现不了）：${missingInHelp.join('、')}`,
    ).toEqual([]);
    expect(
      notImplemented,
      `这些快捷键列在「关于」页，但实现里没有对应分支（按了没反应）：${notImplemented.join('、')}`,
    ).toEqual([]);
  });
});
