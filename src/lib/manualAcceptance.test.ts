import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

const repoRoot = resolve(__dirname, '../..');

function uncheckedCount(relativePath: string): number {
  const content = readFileSync(resolve(repoRoot, relativePath), 'utf8');
  return content.split('\n').filter((line) => /^-\s+\[\s\]/.test(line)).length;
}

describe('V2 统一人工验收清单计数', () => {
  it('执行包声明的未完成数量与两份权威清单一致', () => {
    const plan = readFileSync(
      resolve(repoRoot, 'docs/acceptance/V2_UNIFIED_MANUAL_PLAN.md'),
      'utf8',
    );
    const declared = plan.match(/当前未完成：P0\s+(\d+)\s+项、P1 \/ P2\s+(\d+)\s+项/);
    expect(declared).not.toBeNull();

    const p0 = uncheckedCount('docs/V2_P0_MANUAL_ACCEPTANCE.md');
    const p1p2 = uncheckedCount('docs/V2_P1_P2_MANUAL_ACCEPTANCE.md');
    expect(p0).toBe(Number(declared?.[1]));
    expect(p1p2).toBe(Number(declared?.[2]));
  });
});
