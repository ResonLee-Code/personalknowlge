import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

/**
 * 「版本身份四处一致」的本地守卫（`DEVELOPMENT_RULES.md` §8 明确要求的规则）。
 *
 * 发布工作流在**推 tag 时**会校验这四处（`.github/workflows/release.yml` 的 preflight），
 * 但那意味着改错三处要等到发版才被发现。这里把它提前到 `pnpm test`：
 * 任一处漏改，本地立刻失败。
 *
 * 注意：`docs/CHANGELOG.md` 的版本小节**故意可能领先**（内容先落地、打包时再统一升版），
 * 因此不纳入这条守卫——那属于「发布前统一升版」这一步，见 `PLAN_V1.2.1.md` 阶段 A1。
 */
const repoRoot = resolve(__dirname, '../..');

function read(relative: string): string {
  return readFileSync(resolve(repoRoot, relative), 'utf8');
}

describe('版本身份四处一致', () => {
  it('package.json / tauri.conf.json / Cargo.toml / Cargo.lock 版本号一致', () => {
    const pkg = JSON.parse(read('package.json')) as { version: string };
    const tauri = JSON.parse(read('src-tauri/tauri.conf.json')) as { version: string };

    // Cargo.toml：只取 [package] 段里的 version（依赖的版本不参与）
    const lines = read('src-tauri/Cargo.toml').split('\n');
    const packageStart = lines.findIndex((line) => line.trim() === '[package]');
    expect(packageStart, 'Cargo.toml 里没有 [package] 段？').toBeGreaterThanOrEqual(0);
    const cargo = lines
      .slice(packageStart + 1)
      .find((line) => /^version\s*=/.test(line.trim()))
      ?.match(/"([^"]+)"/)?.[1];

    // Cargo.lock：本包名紧跟的那一行版本
    const lock = read('src-tauri/Cargo.lock');
    const lockEntry = lock.split('[[package]]').find((block) => block.includes('name = "personal-knowledge-os"'));
    const locked = /^version\s*=\s*"([^"]+)"/m.exec(lockEntry ?? '')?.[1];

    const versions = { pkg: pkg.version, tauri: tauri.version, cargo, lock: locked };
    expect(cargo, 'Cargo.toml 里没解析到版本号（解析器坏了？）').toBeTruthy();
    expect(locked, 'Cargo.lock 里没解析到本包版本（解析器坏了？）').toBeTruthy();
    expect(new Set(Object.values(versions)).size, `版本号不一致：${JSON.stringify(versions)}`).toBe(1);
  });
});
