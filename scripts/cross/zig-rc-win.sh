#!/bin/sh
# 资源编译器（图标、版本信息、清单）。
#
# embed-resource 会先用 `rc -V /?` 探测实现；这里伪装成 LLVM Resource Converter，
# 然后把请求转发给 `zig rc`（与 Microsoft RC.EXE 参数兼容）。
set -eu

ZIG="${ZIG:-zig}"
export ZIG_GLOBAL_CACHE_DIR="${ZIG_GLOBAL_CACHE_DIR:-/tmp/pkos-zig-cache}"
export ZIG_LOCAL_CACHE_DIR="${ZIG_LOCAL_CACHE_DIR:-/tmp/pkos-zig-cache}"

if [ "${1:-}" = "-V" ] && [ "${2:-}" = "/?" ]; then
  # 关键：输出必须以 OVERVIEW: Resource Converter 开头，并包含 no-preprocess
  echo "OVERVIEW: Resource Converter (zig rc shim, no-preprocess support)"
  echo "Usage: rc [options] <input>"
  exit 0
fi

output=""
codepage=""
sources=""

while [ $# -gt 0 ]; do
  case "$1" in
    /fo | /FO | -fo | --fo)
      output="$2"
      shift 2
      ;;
    /c | /C | -c | --c)
      codepage="$2"
      shift 2
      ;;
    /no-preprocess | -no-preprocess | --no-preprocess | /nologo | /NOLOGO)
      shift
      ;;
    -I | /I | --include-dir | --I)
      sources="$sources /I $2"
      shift 2
      ;;
    -D | /D)
      sources="$sources /d $2"
      shift 2
      ;;
    --)
      shift
      ;;
    *)
      sources="$sources $1"
      shift
      ;;
  esac
done

if [ -n "$output" ]; then
  # shellcheck disable=SC2086
  exec "$ZIG" rc /nologo /fo "$output" ${codepage:+"/c"} ${codepage:+"$codepage"} $sources
fi

# shellcheck disable=SC2086
exec "$ZIG" rc /nologo $sources
