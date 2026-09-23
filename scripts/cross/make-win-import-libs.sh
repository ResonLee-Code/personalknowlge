#!/bin/sh
# 为 Windows 交叉编译准备导入库副本。
#
# 背景：交叉编译到 windows-gnu 时，rustc 使用 no_fallback 策略，
# 不会把 -l 交给链接器兜底搜索，必须找到确切文件名的导入库；
# 而 windows-* crate 与 webview2-com-sys 提供的是 MSVC 命名（*.lib）。
#
# 该脚本在输出目录生成 rustc 期望的命名副本：
#   libwindows.<ver>.a、windows.<ver>.dll（内容是 COFF 导入库，链接器按内容识别）
#   libWebView2Loader.dll.a、WebView2Loader.dll.dll
#
# 用法：scripts/cross/make-win-import-libs.sh [输出目录] [项目根目录]
set -eu

OUT_DIR="${1:-/tmp/pkos-win-libs}"
ROOT_DIR="${2:-$(cd "$(dirname "$0")/../.." && pwd)}"

mkdir -p "$OUT_DIR"

# 在全部 cargo registry 源目录中查找文件（可能同时存在多个 registry）
registry_file() {
  for dir in "$HOME"/.cargo/registry/src/*/; do
    if [ -f "$dir$1" ]; then
      printf '%s' "$dir$1"
      return 0
    fi
  done
  return 1
}

copy_as() {
  if src="$(registry_file "$1")"; then
    cp "$src" "$OUT_DIR/$2"
  fi
}

copy_from() {
  if [ -f "$1" ]; then
    cp "$1" "$OUT_DIR/$2"
  fi
}

# windows crate 的 GNU 导入库
for version in 0.52.6 0.53.1; do
  minor="$(printf '%s' "$version" | cut -d. -f1,2)"
  copy_as "windows_x86_64_gnu-$version/lib/libwindows.$minor.0.a" "libwindows.$minor.0.a"
  copy_as "windows_x86_64_gnu-$version/lib/libwindows.$minor.0.a" "windows.$minor.0.dll"
done

# WebView2 加载器（导入库位于 webview2-com-sys 的构建产物中）
WEBVIEW_DIR="$(ls -d "$ROOT_DIR"/src-tauri/target/x86_64-pc-windows-gnu/*/build/webview2-com-sys-*/out/x64 2>/dev/null | head -1 || true)"
if [ -n "$WEBVIEW_DIR" ] && [ -f "$WEBVIEW_DIR/WebView2Loader.dll.lib" ]; then
  copy_from "$WEBVIEW_DIR/WebView2Loader.dll.lib" "libWebView2Loader.dll.a"
  copy_from "$WEBVIEW_DIR/WebView2Loader.dll.lib" "WebView2Loader.dll.dll"
fi

echo "$OUT_DIR"
