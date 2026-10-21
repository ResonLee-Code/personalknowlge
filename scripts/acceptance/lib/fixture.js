'use strict';

const path = require('node:path');

/** Resolve a fixture path without allowing writes outside the verified workspace root. */
function resolveWorkspaceFile(root, relative) {
  const rootPath = path.resolve(root);
  const trimmed = String(relative || '').trim();
  if (!trimmed || path.isAbsolute(trimmed) || trimmed.includes('\0')) {
    throw new Error(`夹具路径无效：${relative}`);
  }
  const target = path.resolve(rootPath, trimmed);
  if (target === rootPath || !target.startsWith(`${rootPath}${path.sep}`)) {
    throw new Error(`夹具路径无效：${relative}`);
  }
  return target;
}

module.exports = { resolveWorkspaceFile };
