'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');

const { resolveWorkspaceFile } = require('./fixture');

test('workspace fixture paths stay inside the verified root', () => {
  const root = path.resolve('/private/tmp/pkos-workspace');

  assert.equal(
    resolveWorkspaceFile(root, '00_Inbox/fixture.md'),
    path.join(root, '00_Inbox/fixture.md'),
  );
  for (const invalid of ['', '../outside.md', '/tmp/outside.md', '00_Inbox/../../outside.md']) {
    assert.throws(() => resolveWorkspaceFile(root, invalid), /夹具路径无效/);
  }
});
