import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

/**
 * 前端调用的 Tauri 插件命令必须在能力声明（capabilities）里被放行。
 *
 * 起因（2026-10-11 真机验收发现的缺陷）：PDF「打印视图」点下去弹
 * `Command plugin:opener|open_path not allowed by ACL`——`opener:default` 只含
 * `open_url` / `reveal_item_in_dir`，**不含 `open_path`**；同一处问题也让
 * 「设置 → 工作区目录 → 打开」失效。这类错只会在真机点下去时才暴露，
 * 所以在 `pnpm test` 里拦一道：**前端 import 了哪个插件函数，ACL 就必须覆盖它**。
 */
const repoRoot = resolve(__dirname, '../..');

function listSourceFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      out.push(...listSourceFiles(full));
    } else if (/\.tsx?$/.test(entry) && !/\.test\.tsx?$/.test(entry)) {
      out.push(full);
    }
  }
  return out;
}

/** 插件函数 → 需要的权限标识；`coveredByDefaultSet` 表示该插件的 default 权限集已含它。 */
const OPENEr_REQUIREMENTS: Record<string, { permission: string; coveredByDefault: boolean }> = {
  openPath: { permission: 'opener:allow-open-path', coveredByDefault: false },
  openUrl: { permission: 'opener:allow-open-url', coveredByDefault: true },
  revealItemInDir: { permission: 'opener:allow-reveal-item-in-dir', coveredByDefault: true },
};

type Capability = {
  permissions: Array<string | { identifier: string; allow?: Array<{ path?: string }> }>;
};

function capability(): Capability {
  return JSON.parse(readFileSync(resolve(repoRoot, 'src-tauri/capabilities/default.json'), 'utf8'));
}

function usedOpenerFunctions(): string[] {
  const used = new Set<string>();
  for (const file of listSourceFiles(resolve(repoRoot, 'src'))) {
    const code = readFileSync(file, 'utf8');
    const match = /import\s*\{([^}]+)\}\s*from\s*'@tauri-apps\/plugin-opener'/g;
    for (const found of code.matchAll(match)) {
      (found[1] ?? '')
        .split(',')
        .map((name) => name.trim().split(/\s+as\s+/)[0]!.trim())
        .filter(Boolean)
        .forEach((name) => used.add(name));
    }
  }
  return [...used];
}

describe('Tauri 插件权限与前端调用对齐', () => {
  it('前端用到的 opener 函数都有对应权限（open_path 不在 default 里，必须显式放行）', () => {
    const permissions = capability().permissions;
    const identifiers = permissions.map((entry) =>
      typeof entry === 'string' ? entry : entry.identifier,
    );
    const used = usedOpenerFunctions();
    expect(used.length, '没扫到任何 opener 调用？扫描逻辑坏了').toBeGreaterThan(0);

    const missing: string[] = [];
    for (const name of used) {
      const requirement = OPENEr_REQUIREMENTS[name];
      expect(requirement, `新增了 opener 调用 ${name}，但测试里的权限映射没跟上`).toBeTruthy();
      if (!requirement!.coveredByDefault && !identifiers.includes(requirement!.permission)) {
        missing.push(`${name} → 需要 ${requirement!.permission}`);
      }
    }
    expect(missing, `capabilities/default.json 缺少权限：\n${missing.join('\n')}`).toEqual([]);
    // `opener:default` 是 openUrl / revealItemInDir 的前提
    expect(identifiers).toContain('opener:default');
  });

  /**
   * 光有标识还不够：`allow-open-path` 如果不带 scope，插件会认为「没有任何允许的路径」，
   * 按钮依旧报错。这里要求它至少给出一条 path 规则。
   */
  it('open_path 的放行必须带非空 scope（否则等于没放行）', () => {
    const entry = capability().permissions.find(
      (item) => typeof item !== 'string' && item.identifier === 'opener:allow-open-path',
    );
    expect(entry, '缺少 opener:allow-open-path').toBeTruthy();
    const allow = (entry as { allow?: Array<{ path?: string }> }).allow ?? [];
    expect(allow.length, 'allow 列表为空：插件会拒绝所有路径').toBeGreaterThan(0);
    expect(allow.every((rule) => typeof rule.path === 'string' && rule.path.length > 0)).toBe(true);
  });
});
