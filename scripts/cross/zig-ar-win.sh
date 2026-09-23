#!/bin/sh
# 静态库打包（libsqlite3.a 等），使用 Zig 内置的 LLVM ar。
set -eu

ZIG="${ZIG:-zig}"
export ZIG_GLOBAL_CACHE_DIR="${ZIG_GLOBAL_CACHE_DIR:-/tmp/pkos-zig-cache}"
export ZIG_LOCAL_CACHE_DIR="${ZIG_LOCAL_CACHE_DIR:-/tmp/pkos-zig-cache}"

exec "$ZIG" ar "$@"
