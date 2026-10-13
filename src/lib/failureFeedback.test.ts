import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

/**
 * 失败提示的统一口径守卫（Track F2）。
 *
 * 背景：失败原因曾经散落在几十个 `toast.error(...)` 里——有的把整句话塞进标题（长文本被截断）、
 * 有的只挂 tooltip（要悬停才看得见）、有的干脆只报"失败"两个字。B3 让原因**可见**并写进日志，
 * F2 再进一步：所有失败都走 `reportFailure()`（摘要 + 完整原因 + 停留更久 + 可复制），
 * 长原因在界面上另有 `FailureDetail` 可点开。
 *
 * 这条守卫的作用是**别让它退化回去**：新写失败路径时如果直接 `toast.error(errorMessage(error))`，
 * 用户就又只有一句"失败"、看不到原因——测试会拦下来。
 */
const repoRoot = resolve(__dirname, '../..');
const sourceRoots = ['src'];

/** 允许出现「原生 toast.error」的地方：统一入口本身，以及测试。 */
const ALLOWED = new Set([join(repoRoot, 'src/lib/actionFeedback.ts')]);

function sourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) {
      out.push(...sourceFiles(path));
      continue;
    }
    if (!/\.tsx?$/.test(entry)) continue;
    if (/\.test\.tsx?$/.test(entry)) continue;
    out.push(path);
  }
  return out;
}

/**
 * 这些写法说明"失败原因就在手上，却被直接丢给原生 toast"——
 * 正是 F2 要收口的东西。
 */
const FORBIDDEN = [
  /toast\.error\(\s*errorMessage\(/,
  /toast\.error\(\s*use[A-Za-z]+Store\.getState\(\)\.error/,
  /toast\.error\(\s*notesError\(/,
];

describe('失败提示统一走 reportFailure（F2 守卫）', () => {
  it('没有别处再把失败原因直接塞给原生 toast.error', () => {
    const offenders: string[] = [];
    for (const root of sourceRoots) {
      for (const file of sourceFiles(join(repoRoot, root))) {
        if (ALLOWED.has(file)) continue;
        const content = readFileSync(file, 'utf8');
        for (const pattern of FORBIDDEN) {
          if (pattern.test(content)) {
            offenders.push(file.replace(`${repoRoot}/`, ''));
            break;
          }
        }
      }
    }
    expect(
      offenders,
      `以下文件绕过了统一失败提示（原因会被截断或丢失，用户看不到到底为什么失败）。` +
        `改用 reportFailure(摘要, 原因)：\n${offenders.join('\n')}`,
    ).toEqual([]);
  });

  it('reportFailure 自己会写进日志能对上的完整原因（不是只报两个字）', () => {
    const source = readFileSync(join(repoRoot, 'src/lib/actionFeedback.ts'), 'utf8');
    expect(source).toContain('description');
    expect(source).toContain('复制详情');
  });
});
