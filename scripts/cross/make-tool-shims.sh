#!/bin/sh
# 生成 mingw-w64 工具链软链接，让 Rust/cc/embed-resource 在 macOS/Linux 上
# 也能找到 `x86_64-w64-mingw32-*` 工具（实际由 Zig 提供）。
#
# 用法：scripts/cross/make-tool-shims.sh [输出目录]
# 默认输出到 /tmp/pkos-win-tools/bin，可把该目录加入 PATH 使用。
set -eu

CROSS_DIR="$(cd "$(dirname "$0")" && pwd)"
OUT_DIR="${1:-/tmp/pkos-win-tools/bin}"

mkdir -p "$OUT_DIR"

for tool in dlltool ar ranlib windres gcc cc; do
  target="$OUT_DIR/x86_64-w64-mingw32-$tool"
  rm -f "$target"
  ln -s "$CROSS_DIR/zig-forward.sh" "$target"
done

echo "$OUT_DIR"
