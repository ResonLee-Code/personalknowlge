'use strict';

const assert = require('node:assert/strict');
const test = require('node:test');

const { parseWorkspaceRoots } = require('./mac');

test('workspace root is derived from the open database path', () => {
  const output = [
    'p123',
    'fcwd',
    'n/',
    'f7u',
    'n/private/tmp/pkos-acceptance/.knowledge/database.sqlite',
    'f8u',
    'n/private/tmp/pkos-acceptance/.knowledge/database.sqlite-wal',
  ].join('\n');
  assert.deepEqual(parseWorkspaceRoots(output), ['/private/tmp/pkos-acceptance']);
});

test('unrelated and temporary database names are ignored', () => {
  const output = [
    'n/tmp/database.sqlite',
    'n/tmp/.knowledge/database.sqlite-shm',
    'n/Library/Other/file',
  ].join('\n');
  assert.deepEqual(parseWorkspaceRoots(output), []);
});
