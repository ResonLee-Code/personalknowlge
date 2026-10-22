'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { runArtifactCheck, runCheck } = require('./file-check');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pkos-file-check-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return root;
}

test('modifiedWithinMinutes chooses the newest matching file recursively', (t) => {
  const root = fixture(t);
  const nested = path.join(root, '00_Inbox', 'nested');
  fs.mkdirSync(nested, { recursive: true });
  const old = path.join(root, '00_Inbox', 'old-ACCEPT.md');
  const fresh = path.join(nested, 'new-ACCEPT.md');
  fs.writeFileSync(old, 'old body');
  fs.writeFileSync(fresh, 'fresh body');
  const oldTime = new Date(Date.now() - 10 * 60_000);
  fs.utimesSync(old, oldTime, oldTime);

  const result = runCheck(root, {
    path: '00_Inbox',
    containsFile: 'ACCEPT',
    modifiedWithinMinutes: 2,
    grep: 'fresh body',
  });
  assert.equal(result.ok, true);
  assert.match(result.evidence, /new-ACCEPT\.md/);
});

test('an old matching body cannot mask the newest file missing that body', (t) => {
  const root = fixture(t);
  fs.mkdirSync(path.join(root, '00_Inbox'), { recursive: true });
  const old = path.join(root, '00_Inbox', 'old-ACCEPT.md');
  const fresh = path.join(root, '00_Inbox', 'new-ACCEPT.md');
  fs.writeFileSync(old, 'required body');
  fs.writeFileSync(fresh, 'missing body');
  const oldTime = new Date(Date.now() - 10 * 60_000);
  fs.utimesSync(old, oldTime, oldTime);

  const result = runCheck(root, {
    path: '00_Inbox',
    containsFile: 'ACCEPT',
    modifiedWithinMinutes: 2,
    grep: 'required body',
  });
  assert.equal(result.ok, false);
  assert.match(result.error, /最新文件.*没有/);
});

test('artifact checks validate text formats and reject paths outside the artifact root', (t) => {
  const root = fixture(t);
  fs.mkdirSync(path.join(root, 'exports'), { recursive: true });
  fs.writeFileSync(
    path.join(root, 'exports', 'note.md'),
    '---\ntitle: 导出验收\n---\n\nROUNDTRIP-42\n',
  );
  fs.writeFileSync(
    path.join(root, 'exports', 'note.html'),
    '<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="default-src \'none\'"></head><body>导出验收 ROUNDTRIP-42</body></html>',
  );

  assert.equal(
    runArtifactCheck(root, {
      path: 'exports/note.md',
      type: 'markdown',
      contains: ['导出验收', 'ROUNDTRIP-42'],
    }).ok,
    true,
  );
  assert.equal(
    runArtifactCheck(root, {
      path: 'exports/note.html',
      type: 'html',
      contains: ['导出验收', 'ROUNDTRIP-42'],
    }).ok,
    true,
  );
  assert.equal(
    runArtifactCheck(root, { path: '../outside.md', type: 'markdown' }).ok,
    false,
  );
});

test('artifact directory checks recursively verify restored files', (t) => {
  const root = fixture(t);
  const restore = path.join(root, 'restore');
  fs.mkdirSync(path.join(restore, '00_Inbox'), { recursive: true });
  fs.writeFileSync(path.join(restore, '00_Inbox', 'ACCEPT-RESTORE.md'), 'body');
  fs.writeFileSync(path.join(restore, '.knowledge.sqlite'), 'db');

  const result = runArtifactCheck(root, {
    path: 'restore',
    type: 'directory',
    minFiles: 2,
    containsFile: 'ACCEPT-RESTORE.md',
  });
  assert.equal(result.ok, true);
  assert.match(result.evidence, /递归含 2 个文件/);
});
