#!/usr/bin/env bash
# 为完整 App 的 10 万篇人工抽样准备持久化验收库。
#
#   scripts/bench/prepare-scale-fixture.sh /private/tmp/pkos-100k
#   scripts/bench/prepare-scale-fixture.sh --notes 100000 /private/tmp/pkos-100k
#
# 目标目录必须不存在或为空；脚本拒绝覆盖已有知识库。生成后可启动应用并
# 手动打开该路径，测量完整窗口 + WebView 的首屏时间与峰值内存。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
NOTES=100000
TARGET=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --notes) NOTES="$2"; shift 2 ;;
    -h|--help) sed -n '2,10p' "${BASH_SOURCE[0]}"; exit 0 ;;
    -*) echo "未知参数：$1" >&2; exit 2 ;;
    *) TARGET="$1"; shift ;;
  esac
done

if [[ -z "${TARGET}" ]]; then
  echo "缺少目标目录。用法：prepare-scale-fixture.sh [--notes N] <空目录>" >&2
  exit 2
fi
if [[ -e "${TARGET}" && ! -d "${TARGET}" ]]; then
  echo "目标已存在且不是目录：${TARGET}" >&2
  exit 2
fi
if [[ -d "${TARGET}" && -n "$(find "${TARGET}" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  echo "拒绝覆盖非空目录：${TARGET}" >&2
  exit 2
fi

cd "${ROOT}"
BENCH_WORKSPACE="${TARGET}" \
BENCH_NOTES="${NOTES}" \
BENCH_PREPARE_ONLY=1 \
BENCH_STOP_WATCHER=1 \
cargo test --offline --manifest-path src-tauri/Cargo.toml \
  --test search_bench -- --ignored --nocapture

echo
echo "${NOTES} 篇验收库已准备：${TARGET}"
echo "下一步：启动应用，打开该知识库，记录完整窗口首屏时间与峰值 RSS。"
