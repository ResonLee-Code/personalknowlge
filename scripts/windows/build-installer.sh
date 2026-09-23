#!/bin/sh
# 在 macOS / Linux 上用 NSIS 生成 Windows 安装包。
#
# 前置条件：
#   1. 已交叉编译出 Windows 可执行文件：
#        ZIG=/path/to/zig scripts/cross/build-windows.sh
#   2. 环境中有 makensis（NSIS 3.x）。macOS 可用 Homebrew：
#        brew install makensis
#      或参见 docs/WINDOWS_INSTALL_GUIDE.md 中「用 Zig 交叉编译」一节的说明。
#
# 用法：scripts/windows/build-installer.sh [版本号]
set -eu

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
VERSION="${1:-$(grep -m1 '^version' "$ROOT_DIR/src-tauri/Cargo.toml" | cut -d'"' -f2)}"
SOURCE_DIR="${SOURCE_DIR:-$ROOT_DIR/src-tauri/target/x86_64-pc-windows-gnu/release}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/dist-windows}"

if ! command -v makensis >/dev/null 2>&1 && [ ! -x "$NSIS_PATH/makensis.exe" ]; then
  echo "错误：未找到 makensis。请先安装 NSIS 3.x（brew install makensis 或使用 NSIS_PATH 指向自建版本）。" >&2
  exit 1
fi

MAKENSIS="$(command -v makensis || echo "$NSIS_PATH/makensis.exe")"

# 自建 makensis 的共享数据（Stubs/Include/Plugins）与可执行文件同目录，
# 而 POSIX 版 makensis 默认会向上找一级；这里按需显式指定 NSISDIR。
if [ -z "${NSISDIR:-}" ] && [ -d "$(dirname "$MAKENSIS")/Stubs" ]; then
  export NSISDIR="$(cd "$(dirname "$MAKENSIS")" && pwd)"
fi

if [ ! -f "$SOURCE_DIR/personal-knowledge-os.exe" ]; then
  echo "错误：未找到 $SOURCE_DIR/personal-knowledge-os.exe" >&2
  echo "请先执行：ZIG=/path/to/zig scripts/cross/build-windows.sh" >&2
  exit 1
fi

# 若存在 WebView2Loader.dll（GNU 目标动态加载需要），一并打包
WEBVIEW_DLL="$(ls -d "$ROOT_DIR"/src-tauri/target/x86_64-pc-windows-gnu/*/build/webview2-com-sys-*/out/x64/WebView2Loader.dll 2>/dev/null | head -1 || true)"
if [ -n "$WEBVIEW_DLL" ] && [ ! -f "$SOURCE_DIR/WebView2Loader.dll" ]; then
  cp "$WEBVIEW_DLL" "$SOURCE_DIR/WebView2Loader.dll"
  echo "==> 已附带 WebView2Loader.dll"
fi

mkdir -p "$OUTPUT_DIR"

echo "==> 生成安装包（版本 ${VERSION}）"
"$MAKENSIS" -V2 \
  -DAPP_VERSION="$VERSION" \
  -DSOURCE_DIR="$SOURCE_DIR" \
  -DOUTPUT_DIR="$OUTPUT_DIR" \
  "$ROOT_DIR/scripts/windows/installer.nsi"

ls -la "$OUTPUT_DIR"/*.exe

# 附带生成便携版（免安装）：exe + WebView2Loader.dll + 使用说明
PORTABLE_DIR="$OUTPUT_DIR/portable"
PORTABLE_ZIP="$OUTPUT_DIR/Personal Knowledge OS_${VERSION}_x64-portable.zip"
rm -rf "$PORTABLE_DIR"
mkdir -p "$PORTABLE_DIR"
cp "$SOURCE_DIR/personal-knowledge-os.exe" "$PORTABLE_DIR/"
[ -f "$SOURCE_DIR/WebView2Loader.dll" ] && cp "$SOURCE_DIR/WebView2Loader.dll" "$PORTABLE_DIR/"
# 使用 ASCII 文件名，避免 ZIP 在 Windows 上出现中文乱码
cp "$ROOT_DIR/docs/WINDOWS_INSTALL_GUIDE.md" "$PORTABLE_DIR/README-Windows.md"

rm -f "$PORTABLE_ZIP"
( cd "$OUTPUT_DIR" && zip -qr "$(basename "$PORTABLE_ZIP")" portable )
rm -rf "$PORTABLE_DIR"

echo "==> 便携版：$PORTABLE_ZIP"
ls -la "$OUTPUT_DIR"
