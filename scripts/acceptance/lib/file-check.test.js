'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const { runCheck } = require('./file-check');

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
