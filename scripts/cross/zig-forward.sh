#!/bin/sh
# 通用转发器：按被调用的名字（argv[0]）把 mingw-w64 工具链命令转发给 Zig。
#
# 需要环境变量 ZIG 指向 zig 可执行文件。由 scripts/cross/make-tool-shims.sh
# 以 `x86_64-w64-mingw32-*` 名字创建软链接后使用。
set -eu

if [ -z "${ZIG:-}" ]; then
  echo "错误：请设置 ZIG 环境变量指向 zig 可执行文件" >&2
  exit 1
fi

export ZIG_GLOBAL_CACHE_DIR="${ZIG_GLOBAL_CACHE_DIR:-/tmp/pkos-zig-cache}"
export ZIG_LOCAL_CACHE_DIR="${ZIG_LOCAL_CACHE_DIR:-/tmp/pkos-zig-cache}"

name="$(basename "$0")"
case "$name" in
  *windres | *rc)
    exec sh "$(dirname "$0")/zig-rc-win.sh" "$@"
    ;;
  *dlltool)
    exec "$ZIG" dlltool "$@"
    ;;
  *ranlib)
    exec "$ZIG" ranlib "$@"
    ;;
  *-ar)
    exec "$ZIG" ar "$@"
    ;;
  *cc | *gcc | *clang)
    exec "$ZIG" cc -target x86_64-windows-gnu "$@"
    ;;
  *)
    echo "未知的转发目标：$name" >&2
    exit 1
    ;;
esac
