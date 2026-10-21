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
