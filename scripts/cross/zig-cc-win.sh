#!/bin/sh
# 用 Zig 的 clang + mingw-w64 sysroot 交叉编译 Windows (x86_64-windows-gnu)。
#
# 用途：在没有 mingw-w64 工具链的 macOS/Linux 主机上编译 Windows 版二进制，
# 同时充当 C 编译器（bundled SQLite）与链接器。
#
# 需要环境变量 ZIG 指向 zig 可执行文件；缓存目录写在 /tmp，避免污染用户 HOME。
set -eu

ZIG="${ZIG:-zig}"
export ZIG_GLOBAL_CACHE_DIR="${ZIG_GLOBAL_CACHE_DIR:-/tmp/pkos-zig-cache}"
export ZIG_LOCAL_CACHE_DIR="${ZIG_LOCAL_CACHE_DIR:-/tmp/pkos-zig-cache}"

# cc-rs 会传入 Rust 三元组（--target=x86_64-pc-windows-gnu），
# 而 Zig/clang 需要 x86_64-windows-gnu，这里做一次参数归一化。
has_target=""
filtered=""
count=$#
index=0
while [ "$index" -lt "$count" ]; do
  arg="$1"
  shift
  case "$arg" in
    # Zig 自带 MinGW CRT，不提供 msvcrt/libgcc 等传统 MinGW 运行库文件；
    # 这些参数会让 Zig 驱动直接报错，因此过滤掉（等价符号由 Zig 运行时提供）。
    -lgcc_eh)
      # Zig 用自带的 libunwind 提供 _Unwind_* 符号
      filtered="1"
      set -- "$@" "-lunwind"
      index=$((index + 1))
      continue
      ;;
    -lmsvcrt | -lmingwex | -lmingw32 | -lgcc | -l:libpthread.a | -lpthread | -lc | -lm)
      filtered="1"
      index=$((index + 1))
      continue
      ;;
    --target=*windows-gnu | -target=*windows-gnu)
      arg="${arg%%=*}=x86_64-windows-gnu"
      has_target="1"
      ;;
    --target | -target)
      next="${1:-}"
      case "$next" in
        *windows-gnu)
          shift
          count=$((count - 1))
          arg="-target"
          set -- "$@" "$arg" "x86_64-windows-gnu"
          has_target="1"
          index=$((index + 1))
          continue
          ;;
      esac
      ;;
    *windows-gnu)
      case "$arg" in
        x86_64-pc-windows-gnu | x86_64_pc_windows_gnu) arg="x86_64-windows-gnu" ;;
      esac
      ;;
  esac
  set -- "$@" "$arg"
  index=$((index + 1))
done

if [ -n "$has_target" ]; then
  exec "$ZIG" cc "$@"
fi

exec "$ZIG" cc -target x86_64-windows-gnu "$@"
