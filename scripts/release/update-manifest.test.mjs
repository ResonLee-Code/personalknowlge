import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const script = path.join(path.dirname(fileURLToPath(import.meta.url)), 'update-manifest.mjs');

test('latest.json maps the signed updater artifacts for all shipped targets', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pkos-update-manifest-'));
  const output = path.join(root, 'latest.json');
  try {
    const fixtures = [
      'macos/Personal Knowledge OS.app.tar.gz',
      'windows/Personal Knowledge OS_2.0.0_x64-setup.exe',
      'linux/Personal Knowledge OS_2.0.0_amd64.AppImage.tar.gz',
    ];
    for (const relative of fixtures) {
      const full = path.join(root, relative);
      fs.mkdirSync(path.dirname(full), { recursive: true });
      fs.writeFileSync(full, 'updater artifact');
      fs.writeFileSync(`${full}.sig`, `signature:${path.basename(full)}`);
    }

    const result = spawnSync(process.execPath, [script], {
      cwd: root,
      env: {
        ...process.env,
        PKOS_RELEASE_VERSION: '2.0.0',
        PKOS_RELEASE_TAG: 'v2.0.0',
        PKOS_RELEASE_BASE_URL: 'https://github.com/example/pkos/releases/download/v2.0.0',
        PKOS_UPDATE_ROOT: root,
        PKOS_UPDATE_OUTPUT: output,
      },
      encoding: 'utf8',
    });
    assert.equal(result.status, 0, result.stderr);

    const manifest = JSON.parse(fs.readFileSync(output, 'utf8'));
    assert.equal(manifest.version, '2.0.0');
    assert.deepEqual(Object.keys(manifest.platforms).sort(), [
      'darwin-aarch64',
      'linux-x86_64',
      'windows-x86_64',
    ]);
    assert.match(
      manifest.platforms['linux-x86_64'].url,
      /Personal%20Knowledge%20OS_2\.0\.0_amd64\.AppImage\.tar\.gz$/,
    );
    assert.equal(
      manifest.platforms['linux-x86_64'].signature,
      'signature:Personal Knowledge OS_2.0.0_amd64.AppImage.tar.gz',
    );
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test('latest.json rejects an updater artifact without its signature', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pkos-update-manifest-'));
  try {
    for (const relative of [
      'macos/Personal Knowledge OS.app.tar.gz',
      'windows/Personal Knowledge OS_2.0.0_x64-setup.exe',
      'linux/Personal Knowledge OS_2.0.0_amd64.AppImage.tar.gz',
    ]) {
      const full = path.join(root, relative);
      fs.mkdirSync(path.dirname(full), { recursive: true });
      fs.writeFileSync(full, 'updater artifact');
      if (!relative.includes('linux/')) fs.writeFileSync(`${full}.sig`, 'signature');
    }

    const result = spawnSync(process.execPath, [script], {
      cwd: root,
      env: {
        ...process.env,
        PKOS_RELEASE_VERSION: '2.0.0',
        PKOS_RELEASE_TAG: 'v2.0.0',
        PKOS_RELEASE_BASE_URL: 'https://github.com/example/pkos/releases/download/v2.0.0',
        PKOS_UPDATE_ROOT: root,
        PKOS_UPDATE_OUTPUT: path.join(root, 'latest.json'),
      },
      encoding: 'utf8',
    });
    assert.notEqual(result.status, 0, '缺少签名时不能生成可发布清单');
    assert.match(result.stderr, /找不到签名文件/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});
