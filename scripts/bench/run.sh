#!/usr/bin/env bash
# E2 性能守护：发布前必跑（1 万篇检索 / 3 万篇图谱）。
#
#   pnpm bench                       # 按发布规格跑两个基准
#   pnpm bench -- --search-notes 2000 --graph-notes 3000   # 冒烟（快）
#   pnpm bench -- --only search      # 只跑检索（或 --only graph）
#
# 口径（与 docs/PLAN_V1.3.0.md 的 E2 一致）：
#   - 两个基准都是**真跑**（不是读旧数字），各自打印一行 `PKOS_BENCH …` 机器可读摘要；
#   - **超阈值即失败**：检索 p95 < 200 ms、召回 ≥ 100%；图谱全量查询 < 1.5 s（M3 红线）；
#   - 阈值可用环境变量覆盖：BENCH_MAX_P95_MS / BENCH_MIN_RECALL_PCT / BENCH_MAX_GRAPH_MS；
#   - 脚本会核对 `PKOS_BENCH …` 摘要存在——**没跑到就不会变绿灯**（防「0 个用例也算通过」）；
#   - 日志默认落在 `src-tauri/target/bench/`（已在 .gitignore 里），失败时打印路径供复查。
#
# 退出码：0 = 两个基准都过线；非零 = 有基准失败或没跑起来。
set -euo pipefail

BENCH_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${BENCH_ROOT}"

SEARCH_NOTES="${BENCH_SEARCH_NOTES:-10000}"
GRAPH_NOTES="${BENCH_GRAPH_NOTES:-30000}"
OUT_DIR="${BENCH_OUT_DIR:-src-tauri/target/bench}"
ONLY="both"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --search-notes) SEARCH_NOTES="$2"; shift 2 ;;
    --graph-notes)  GRAPH_NOTES="$2";  shift 2 ;;
    --out)          OUT_DIR="$2";      shift 2 ;;
    --only)         ONLY="$2";         shift 2 ;;
    -h|--help)      sed -n '2,16p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "未知参数：$1（用 --help 看用法）" >&2; exit 2 ;;
  esac
done

case "${ONLY}" in
  both|search|graph) ;;
  *) echo "--only 只接受 both / search / graph，收到：${ONLY}" >&2; exit 2 ;;
esac

command -v cargo >/dev/null 2>&1 || { echo "找不到 cargo，请先装 Rust 工具链" >&2; exit 127; }

mkdir -p "${OUT_DIR}"
STAMP="$(date +%Y%m%d-%H%M%S)"
LOG="${OUT_DIR}/bench-${STAMP}.log"
SUMMARY="${OUT_DIR}/bench-${STAMP}.summary.txt"

MANIFEST="src-tauri/Cargo.toml"
failed=0

echo "E2 性能守护（发布前必跑）"
echo "  检索：${SEARCH_NOTES} 篇   图谱：${GRAPH_NOTES} 篇"
echo "  日志：${LOG}"
echo

# run_case <名字> <测试目标> <篇数> <摘要前缀> [附加上限环境变量…]
run_case() {
  local label="$1" target="$2" notes="$3" prefix="$4"; shift 4

  echo "──── ${label}（${notes} 篇）────"
  local started
  started="$(date +%s)"

  set +e
  env BENCH_NOTES="${notes}" "$@" \
    cargo test --offline --manifest-path "${MANIFEST}" --test "${target}" \
    -- --ignored --nocapture 2>&1 | tee -a "${LOG}"
  local status="${PIPESTATUS[0]}"
  set -e

  local elapsed=$(( $(date +%s) - started ))
  local line
  line="$(grep -m1 "^${prefix} " "${LOG}" | tail -n1 || true)"

  if [[ -z "${line}" ]]; then
    echo "✗ ${label}：日志里没有 \`${prefix} …\` 摘要 —— 基准没真的跑起来（不是通过）" >&2
    failed=1
    echo
    return 1
  fi

  if [[ "${status}" -ne 0 ]]; then
    echo "✗ ${label}：超过阈值或断言失败（退出码 ${status}，耗时 ${elapsed}s）" >&2
    failed=1
    echo
    return 1
  fi

  echo "✓ ${label}：过线（耗时 ${elapsed}s）"
  echo "  ${line}" | tee -a "${SUMMARY}"
  echo
}

if [[ "${ONLY}" != "graph" ]]; then
  run_case "检索基准" search_bench "${SEARCH_NOTES}" "PKOS_BENCH search" \
    BENCH_MAX_P95_MS="${BENCH_MAX_P95_MS:-200}" \
    BENCH_MIN_RECALL_PCT="${BENCH_MIN_RECALL_PCT:-100}" || true
fi

if [[ "${ONLY}" != "search" ]]; then
  run_case "图谱基准" graph_bench "${GRAPH_NOTES}" "PKOS_BENCH graph" \
    BENCH_MAX_GRAPH_MS="${BENCH_MAX_GRAPH_MS:-1500}" || true
fi

if [[ "${failed}" -ne 0 ]]; then
  echo "结果：✗ 有基准未过线。完整日志：${LOG}" >&2
  exit 1
fi

echo "结果：✓ 全部过线。"
echo "  摘要：${SUMMARY}"
echo "  日志：${LOG}"
echo
echo "提示：把上面两行 \`PKOS_BENCH …\` 抄进 docs/CHANGELOG.md / 体积对照表，"
echo "      数字就是这次发布的实测值（口径 = 本脚本 + 本机）。"
