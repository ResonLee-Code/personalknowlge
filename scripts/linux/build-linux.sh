#!/usr/bin/env bash
#
# 在 Linux 上构建 Personal Knowledge OS 的安装包（deb / rpm / AppImage）。
#
# 用法：
#   ./scripts/linux/build-linux.sh              # 构建全部 Linux 包
#   ./scripts/linux/build-linux.sh deb appimage # 只构建指定格式
#
# 说明：
# - 必须在 Linux 上运行（Tauri 不支持从 macOS/Windows 交叉编译到 Linux）；
# - 首次运行会用 apt 安装系统依赖（需要 sudo）；
# - 产物在 src-tauri/target/release/bundle/ 下，脚本最后打印 SHA-256。

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

BUNDLES=("$@")
if [ ${#BUNDLES[@]} -eq 0 ]; then
  BUNDLES=(deb rpm appimage)
fi

echo "==> 检查系统依赖"
MISSING=()
# libfuse2：AppImage 打包走 linuxdeploy，它需要 FUSE 2（Ubuntu 22.04 默认只带 FUSE 3）
for pkg in libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf libfuse2 build-essential pkg-config libssl-dev; do
  if ! dpkg -s "${pkg}" >/dev/null 2>&1; then
    MISSING+=("${pkg}")
  fi
done
if [ ${#MISSING[@]} -gt 0 ]; then
  echo "缺少依赖：${MISSING[*]}"
  echo "将执行：sudo apt-get update && sudo apt-get install -y ${MISSING[*]}"
  sudo apt-get update
  sudo apt-get install -y "${MISSING[@]}"
fi

echo "==> 构建前端"
if command -v pnpm >/dev/null 2>&1; then
  pnpm install --frozen-lockfile
  pnpm build
else
  echo "未找到 pnpm，请先安装：corepack enable && corepack prepare pnpm@11 --activate" >&2
  exit 1
fi

echo "==> 构建 Tauri 安装包：${BUNDLES[*]}"
ARGS=()
for bundle in "${BUNDLES[@]}"; do
  ARGS+=(--bundles "${bundle}")
done
pnpm tauri build "${ARGS[@]}"

echo
echo "==> 产物与校验值"
find src-tauri/target/release/bundle -maxdepth 2 -type f \
  \( -name '*.deb' -o -name '*.rpm' -o -name '*.AppImage' \) -print0 |
  while IFS= read -r -d '' file; do
    size=$(stat -c '%s' "${file}")
    hash=$(sha256sum "${file}" | awk '{print $1}')
    printf '%s\n  size   %s bytes\n  sha256 %s\n' "${file}" "${size}" "${hash}"
  done

echo
echo "提示：安装方式见 docs/LINUX_INSTALL_GUIDE.md"
