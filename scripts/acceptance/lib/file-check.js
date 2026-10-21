'use strict';

const fs = require('node:fs');
const path = require('node:path');

/** 递归找文件名包含 needle 的文件（最多 depth 层）。 */
function findFilesRecursive(root, needle, depth = 3) {
  const found = [];
  const walk = (dir, prefix, level) => {
    let entries;
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        if (level < depth) walk(path.join(dir, entry.name), relative, level + 1);
      } else if (entry.name.includes(needle)) {
        found.push({ name: relative, absolute: path.join(dir, entry.name) });
      }
    }
  };
  walk(root, '', 0);
  return found;
}

/**
 * 文件断言：`{path, exists, containsFile, grep}`（相对于工作区）。
 *
 * 当同时指定 modifiedWithinMinutes 与 grep 时，两项必须落在**同一个最新文件**上：
 * 旧文件命中正文、新文件没写进去，仍然是失败，不能因为旧文件而假通过。
 */
function runCheck(workspace, step) {
  const target = path.join(workspace, step.path);
  if (step.exists) {
    if (!fs.existsSync(target)) return { ok: false, error: `不存在：${step.path}` };
    return { ok: true, evidence: `✓ ${step.path} 存在` };
  }
  if (step.notExists) {
    if (fs.existsSync(target)) return { ok: false, error: `本不该存在，却在：${step.path}` };
    return { ok: true, evidence: `✓ ${step.path} 不存在（符合预期）` };
  }
  if (step.containsFile) {
    if (!fs.existsSync(target)) return { ok: false, error: `不存在：${step.path}` };
    const entries = findFilesRecursive(target, step.containsFile);
    if (entries.length === 0) {
      return {
        ok: false,
        error: `${step.path}（含子目录）下没有匹配「${step.containsFile}」的文件`,
      };
    }
    if (step.modifiedWithinMinutes) {
      const newest = entries
        .map((entry) => ({ ...entry, mtime: fs.statSync(entry.absolute).mtimeMs }))
        .sort((left, right) => right.mtime - left.mtime)[0];
      if (!newest || Date.now() - newest.mtime > step.modifiedWithinMinutes * 60_000) {
        return {
          ok: false,
          error: `${step.path} 下的「${step.containsFile}」都不是最近 ${step.modifiedWithinMinutes} 分钟内生成的（可能是旧文件）`,
        };
      }
      if (step.grep) {
        const body = fs.readFileSync(newest.absolute, 'utf8');
        if (!body.includes(step.grep)) {
          return {
            ok: false,
            error: `最新文件 ${step.path}/${newest.name} 里没有「${step.grep}」`,
          };
        }
      }
      return { ok: true, evidence: `✓ ${step.path}/${newest.name}（刚生成${step.grep ? '且正文匹配' : ''}）` };
    }
    if (step.grep) {
      const hit = entries.some((entry) => {
        const body = fs.readFileSync(entry.absolute, 'utf8');
        return body.includes(step.grep);
      });
      if (!hit) return { ok: false, error: `匹配到的文件里没有「${step.grep}」` };
    }
    return { ok: true, evidence: `✓ ${step.path}/${entries[0].name}` };
  }
  return { ok: false, error: '未知的断言类型' };
}

module.exports = { findFilesRecursive, runCheck };
