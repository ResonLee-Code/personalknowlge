import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const workflow = fs.readFileSync(path.join(root, '.github/workflows/release.yml'), 'utf8');

test('release write permission is scoped to the release job', () => {
  assert.match(
    workflow,
    /^permissions:\s*\n\s+contents:\s*read\s*$/m,
    'preflight 与构建 job 只应获得只读仓库权限',
  );

  const releaseJob = workflow.split(/\n  release:\s*\n/, 2)[1];
  assert.ok(releaseJob, '必须存在 release job');
  assert.match(
    releaseJob,
    /^ {4}permissions:\s*\n {6}contents:\s*write\s*$/m,
    '只有 release job 应获得 contents: write',
  );
});

test('every external action is pinned to an immutable commit SHA', () => {
  const actionRefs = Array.from(
    workflow.matchAll(/^\s*(?:-\s*)?uses:\s*(\S+)/gm),
    (match) => match[1],
  );
  assert.ok(actionRefs.length > 0, '工作流应包含 Action');
  const mutable = actionRefs.filter((reference) => !/@[0-9a-f]{40}$/.test(reference));
  assert.deepEqual(mutable, [], `Action 必须锁定完整 commit SHA：${mutable.join(', ')}`);
});

test('updater signature verification can bootstrap on a fresh runner', () => {
  const script = fs.readFileSync(
    path.join(root, 'scripts/release/verify-updater-signature.sh'),
    'utf8',
  );
  assert.doesNotMatch(
    script,
    /cargo test --offline/,
    '签名验签脚本不能假设 runner 已有 Rust 依赖缓存',
  );
  assert.match(script, /cargo test --locked/, '签名验签脚本必须使用锁文件，同时允许首次下载依赖');
});

test('preflight enforces the same build, lint, acceptance, and E2E gates as local release checks', () => {
  const preflight = workflow.split(/\n  macos:\s*\n/, 2)[0];
  const required = [
    'pnpm lint',
    'pnpm build',
    'pnpm test:acceptance',
    'pnpm exec playwright install --with-deps chromium',
    'pnpm test:e2e',
  ];
  for (const command of required) {
    assert.ok(preflight.includes(command), `preflight 缺少发布门禁：${command}`);
  }
  assert.ok(
    preflight.indexOf('pnpm build') < preflight.indexOf('pnpm test:e2e'),
    'E2E 必须使用 production build 产物',
  );
  assert.ok(
    preflight.indexOf('pnpm exec playwright install --with-deps chromium') <
      preflight.indexOf('pnpm test:e2e'),
    '必须先安装 Playwright Chromium',
  );
});

test('release job directly needs preflight when consuming its version output', () => {
  const releaseJob = workflow.split(/\n  release:\s*\n/, 2)[1];
  assert.ok(releaseJob, '必须存在 release job');
  assert.match(
    releaseJob,
    /needs:\s*\[preflight, macos, windows, linux\]/,
    '读取 needs.preflight.outputs 时 preflight 必须是 release 的直接依赖',
  );
  assert.match(releaseJob, /needs\.preflight\.outputs\.version/);
});

test('preflight blocks Rust dependency vulnerabilities with a pinned cargo-audit', () => {
  const preflight = workflow.split(/\n  macos:\s*\n/, 2)[0];
  assert.ok(
    preflight.includes('cargo install cargo-audit --locked --version 0.22.2'),
    'cargo-audit 必须固定版本安装',
  );
  assert.ok(
    preflight.includes('cargo audit --file src-tauri/Cargo.lock'),
    'preflight 必须审计 Rust 依赖漏洞',
  );
});

test('Linux release job installs the xdg-open runtime required by AppImage bundling', () => {
  const linuxJob = workflow.split(/\n  linux:\s*\n/, 2)[1];
  assert.ok(linuxJob, '必须存在 linux job');
  assert.match(
    linuxJob,
    /apt-get install[\s\S]*xdg-utils/,
    '缺少 xdg-utils 时 linuxdeploy 会在 AppImage 打包阶段失败',
  );
});
