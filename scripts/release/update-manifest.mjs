#!/usr/bin/env node
/**
 * 为 Tauri updater 生成 latest.json。
 *
 * 输入：
 * - PKOS_RELEASE_VERSION：不含 v 的版本号
 * - PKOS_RELEASE_TAG：GitHub tag（例如 v2.0.0）
 * - PKOS_RELEASE_BASE_URL：该 Release 的下载基地址
 * - PKOS_RELEASE_NOTES：可选，发布说明
 * - PKOS_UPDATE_ROOT：包含三平台 updater 产物的目录，默认 dist
 */

import fs from 'node:fs';
import path from 'node:path';

const version = process.env.PKOS_RELEASE_VERSION?.trim();
const tag = process.env.PKOS_RELEASE_TAG?.trim();
const baseUrl = process.env.PKOS_RELEASE_BASE_URL?.trim();
const root = path.resolve(process.env.PKOS_UPDATE_ROOT?.trim() || 'dist');
const output = path.resolve(process.env.PKOS_UPDATE_OUTPUT?.trim() || 'dist/latest.json');

if (!version || !tag || !baseUrl) {
  throw new Error('缺少 PKOS_RELEASE_VERSION / PKOS_RELEASE_TAG / PKOS_RELEASE_BASE_URL');
}
if (!fs.existsSync(root)) throw new Error(`更新产物目录不存在：${root}`);

function walk(dir) {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = path.join(dir, entry.name);
    return entry.isDirectory() ? walk(full) : [full];
  });
}

const files = walk(root).sort();
const platforms = [
  {
    key: 'darwin-aarch64',
    match: (file) => file.endsWith('.app.tar.gz'),
  },
  {
    key: 'windows-x86_64',
    match: (file) => file.endsWith('-setup.exe') || /updater[^/]*\.exe$/.test(file),
  },
  {
    key: 'linux-x86_64',
    // Tauri updater 使用 AppImage 外层 gzip 包；裸 AppImage 只用于首次安装。
    match: (file) => file.endsWith('.AppImage.tar.gz'),
  },
];

const result = {};
for (const platform of platforms) {
  const artifact = files.find((file) => platform.match(file) && !file.endsWith('.sig'));
  if (!artifact) throw new Error(`找不到 ${platform.key} 的 updater 产物`);
  const signaturePath = `${artifact}.sig`;
  if (!fs.existsSync(signaturePath)) {
    throw new Error(`找不到签名文件：${signaturePath}`);
  }
  const name = path.basename(artifact);
  // GitHub Releases 会把上传资产名里的空格替换成点；清单 URL 必须跟随这个
  // 实际可下载名称，不能保留本地构建产物的空格（%20 会直接 404）。
  const assetName = name.replaceAll(' ', '.');
  const signature = fs.readFileSync(signaturePath, 'utf8').trim();
  if (!signature) {
    throw new Error(`签名文件为空：${signaturePath}`);
  }
  result[platform.key] = {
    signature,
    url: `${baseUrl.replace(/\/+$/, '')}/${encodeURIComponent(assetName)}`,
  };
}

const manifest = {
  version,
  notes: process.env.PKOS_RELEASE_NOTES?.trim() || `Personal Knowledge OS ${tag}`,
  pub_date: new Date().toISOString(),
  platforms: result,
};

fs.mkdirSync(path.dirname(output), { recursive: true });
fs.writeFileSync(output, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(JSON.stringify(manifest, null, 2));
