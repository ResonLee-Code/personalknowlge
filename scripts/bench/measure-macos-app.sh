#!/usr/bin/env bash
# 在隔离数据目录中启动 macOS 应用，测量从进程启动到 10 万篇工作区主界面出现的时间与 RSS。
#
# 前置：
#   scripts/bench/prepare-scale-fixture.sh --data-dir <data-dir> <workspace>
#   pnpm tauri build --debug --bundles app  或 pnpm tauri build --debug --no-bundle
#
# 用法：
#   scripts/bench/measure-macos-app.sh \
#     --workspace /private/tmp/pkos-100k \
#     --data-dir /private/tmp/pkos-100k-data \
#     --binary src-tauri/target/debug/personal-knowledge-os \
#     --app-name personal-knowledge-os
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKSPACE=""
DATA_DIR=""
BINARY="src-tauri/target/debug/personal-knowledge-os"
APP_NAME="personal-knowledge-os"
MAX_START_MS=2000
MAX_RSS_MB=1536

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace) WORKSPACE="$2"; shift 2 ;;
    --data-dir) DATA_DIR="$2"; shift 2 ;;
    --binary) BINARY="$2"; shift 2 ;;
    --app-name) APP_NAME="$2"; shift 2 ;;
    --max-start-ms) MAX_START_MS="$2"; shift 2 ;;
    --max-rss-mb) MAX_RSS_MB="$2"; shift 2 ;;
    -h|--help) sed -n '2,14p' "${BASH_SOURCE[0]}"; exit 0 ;;
    *) echo "未知参数：$1" >&2; exit 2 ;;
  esac
done

[[ "$(uname -s)" == "Darwin" ]] || { echo "只支持 macOS" >&2; exit 2; }
[[ -d "${WORKSPACE}" ]] || { echo "工作区不存在：${WORKSPACE}" >&2; exit 2; }
[[ -x "${BINARY}" ]] || { echo "应用不可执行：${BINARY}" >&2; exit 2; }

cd "${ROOT}"
mkdir -p "${DATA_DIR}"
LOG="${DATA_DIR}/startup.log"
START_MS="$(node -e 'process.stdout.write(String(Date.now()))')"
PKOS_DATA_DIR="${DATA_DIR}" \
PKOS_AUTO_OPEN_WORKSPACE="${WORKSPACE}" \
"${BINARY}" >"${LOG}" 2>&1 &
APP_PID=$!

cleanup() {
  if kill -0 "${APP_PID}" 2>/dev/null; then
    kill "${APP_PID}" 2>/dev/null || true
  fi
}
trap cleanup EXIT

APP_LOG="${DATA_DIR}/logs/app.log"
FIRST_PAINT_MS=""
for _ in $(seq 1 120); do
  if ! kill -0 "${APP_PID}" 2>/dev/null; then
    echo "应用在窗口出现前退出，日志：${LOG}" >&2
    tail -40 "${LOG}" >&2 || true
    exit 1
  fi
  FIRST_PAINT_MS="$(node -e '
    const fs = require("node:fs");
    const [log, started] = process.argv.slice(1);
    if (!fs.existsSync(log)) process.exit(0);
    const startedAt = Number(started);
    const entries = fs.readFileSync(log, "utf8")
      .split("\n")
      .filter((line) => line.includes("workspace-first-paint"))
      .map((line) => Date.parse(line.split(" ")[0]))
      .filter((value) => Number.isFinite(value) && value >= startedAt);
    if (entries.length > 0) {
      process.stdout.write(String(Math.max(...entries) - startedAt));
    }
  ' "${APP_LOG}" "${START_MS}" 2>/dev/null || true)"
  [[ -n "${FIRST_PAINT_MS}" ]] && break
  sleep 0.25
done
END_MS="$(node -e 'process.stdout.write(String(Date.now()))')"
if [[ -z "${FIRST_PAINT_MS}" ]]; then
  echo "30 秒内没有看到 workspace-first-paint 标记，日志：${LOG}" >&2
  tail -40 "${LOG}" >&2 || true
  exit 1
fi
SCREENSHOT="${DATA_DIR}/scale-first-paint.png"
/usr/sbin/screencapture -x "${SCREENSHOT}"
RSS_KB="$(ps -o rss= -p "${APP_PID}" | tr -d ' ')"
RSS_MB="$(node -e 'process.stdout.write((Number(process.argv[1]) / 1024).toFixed(2))' "${RSS_KB:-0}")"
ELAPSED_MS="${FIRST_PAINT_MS}"

echo "PKOS_APP_SCALE app=${APP_NAME} workspace=${WORKSPACE} start_ms=${ELAPSED_MS} rss_mb=${RSS_MB} first_paint_marker=yes screenshot=${SCREENSHOT}"
node -e '
  const [elapsed, rss, maxStart, maxRss] = process.argv.slice(1).map(Number);
  if (elapsed >= maxStart) {
    console.error(`首屏 ${elapsed} ms 超过红线 ${maxStart} ms`);
    process.exit(1);
  }
  if (rss >= maxRss) {
    console.error(`RSS ${rss} MB 超过红线 ${maxRss} MB`);
    process.exit(1);
  }
' "${ELAPSED_MS}" "${RSS_MB}" "${MAX_START_MS}" "${MAX_RSS_MB}"
