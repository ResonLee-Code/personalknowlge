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
