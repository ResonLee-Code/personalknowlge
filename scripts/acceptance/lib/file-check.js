'use strict';

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const { resolveWorkspaceFile } = require('./fixture');

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

function readText(file) {
  return fs.readFileSync(file, 'utf8');
}

function assertContains(file, values) {
  const body = readText(file);
  for (const value of values || []) {
    if (!body.includes(value)) return `文件里没有「${value}」`;
  }
  return null;
}

/**
 * 校验导出 / 恢复产物。路径只允许落在本次验收器创建的临时产物根目录内。
 *
 * `type` 的可选值：
 * - `markdown` / `html`：UTF-8 文本，并检查 `contains`；
 * - `docx`：必须是 ZIP 容器，正文 `word/document.xml` 含 `contains`；
 * - `sqlite`：运行只读 `PRAGMA integrity_check`，可再执行给定的只读查询并核对输出；
 * - `directory`：递归检查至少 `minFiles` 个文件，并可检查 `containsFile`。
 */
function runArtifactCheck(root, step) {
  let target;
  try {
    target = resolveWorkspaceFile(root, step.path);
  } catch (error) {
    return { ok: false, error: error.message };
  }
  if (!fs.existsSync(target)) return { ok: false, error: `产物不存在：${step.path}` };
  const stat = fs.statSync(target);
  const type = step.type || 'file';

  if (type === 'directory') {
    if (!stat.isDirectory()) return { ok: false, error: `不是目录：${step.path}` };
    const files = findFilesRecursive(target, '', 20).filter((entry) => entry.name !== '');
    const minFiles = Number(step.minFiles) || 1;
    if (files.length < minFiles) {
      return { ok: false, error: `目录内只有 ${files.length} 个文件，少于 ${minFiles}` };
    }
    if (step.containsFile) {
      const matches = files.filter((entry) => entry.name.includes(step.containsFile));
      if (matches.length === 0) {
        return { ok: false, error: `目录内没有文件名含「${step.containsFile}」的文件` };
      }
    }
    return {
      ok: true,
      evidence: `✓ ${step.path} 是目录，递归含 ${files.length} 个文件`,
    };
  }

  if (!stat.isFile() || stat.size === 0) {
    return { ok: false, error: `不是非空文件：${step.path}` };
  }

  if (type === 'markdown' || type === 'html') {
    const problem = assertContains(target, step.contains);
    if (problem) return { ok: false, error: problem };
    const body = readText(target);
    if (type === 'html') {
      if (!/<html[\s>]/i.test(body)) return { ok: false, error: 'HTML 缺少 html 根元素' };
      if (!/Content-Security-Policy/i.test(body)) {
        return { ok: false, error: 'HTML 缺少 Content-Security-Policy' };
      }
    }
    return {
      ok: true,
      evidence: `✓ ${step.path}（${type}，${stat.size} B，正文匹配）`,
    };
  }

  if (type === 'docx') {
    const signature = fs.readFileSync(target).subarray(0, 4);
    if (signature.toString('hex') !== '504b0304') {
      return { ok: false, error: `DOCX 不是 ZIP 容器：${step.path}` };
    }
    let documentXml;
    try {
      documentXml = execFileSync('unzip', ['-p', target, 'word/document.xml'], {
        encoding: 'utf8',
        maxBuffer: 8 * 1024 * 1024,
      });
    } catch (error) {
      return { ok: false, error: `读取 DOCX 正文失败：${error.message}` };
    }
    for (const value of step.contains || []) {
      if (!documentXml.includes(value)) {
        return { ok: false, error: `DOCX 正文里没有「${value}」` };
      }
    }
    return {
      ok: true,
      evidence: `✓ ${step.path}（docx，${stat.size} B，OOXML 正文匹配）`,
    };
  }

  if (type === 'sqlite') {
    try {
      const integrity = execFileSync('sqlite3', ['-readonly', target, 'PRAGMA integrity_check;'], {
        encoding: 'utf8',
        timeout: 10_000,
      }).trim();
      if (integrity !== 'ok') {
        return { ok: false, error: `SQLite 完整性检查失败：${integrity}` };
      }
      if (step.sql) {
        const result = execFileSync('sqlite3', ['-readonly', target, step.sql], {
          encoding: 'utf8',
          timeout: 10_000,
        }).trim();
        if (result !== String(step.expect)) {
          return {
            ok: false,
            error: `SQLite 查询结果不符：期望「${step.expect}」，实际「${result}」`,
          };
        }
      }
    } catch (error) {
      return { ok: false, error: `SQLite 校验失败：${error.message}` };
    }
    return {
      ok: true,
      evidence: `✓ ${step.path}（SQLite integrity_check=ok${step.sql ? '，恢复数据可查询' : ''}）`,
    };
  }

  return { ok: false, error: `未知的产物类型：${type}` };
}

module.exports = { findFilesRecursive, runArtifactCheck, runCheck };
