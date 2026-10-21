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
DATA_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --notes) NOTES="$2"; shift 2 ;;
    --data-dir) DATA_DIR="$2"; shift 2 ;;
    -h|--help) sed -n '2,10p' "${BASH_SOURCE[0]}"; exit 0 ;;
    -*) echo "未知参数：$1" >&2; exit 2 ;;
    *) TARGET="$1"; shift ;;
  esac
done

if [[ -z "${TARGET}" ]]; then
  echo "缺少目标目录。用法：prepare-scale-fixture.sh [--notes N] [--data-dir DIR] <空目录>" >&2
  exit 2
fi
if [[ -n "${DATA_DIR}" && -d "${DATA_DIR}" && -n "$(find "${DATA_DIR}" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  echo "拒绝覆盖非空应用数据目录：${DATA_DIR}" >&2
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
if [[ -n "${DATA_DIR}" ]]; then
  mkdir -p "${DATA_DIR}"
  WORKSPACE_ID="$(sqlite3 "${TARGET}/.knowledge/database.sqlite" \
    "SELECT id FROM workspaces ORDER BY created_at ASC LIMIT 1;")"
  if [[ -z "${WORKSPACE_ID}" ]]; then
    echo "数据库里没有 workspace id，无法写入应用注册表" >&2
    exit 1
  fi
  PKOS_WORKSPACE_ID="${WORKSPACE_ID}" \
  PKOS_WORKSPACE_ROOT="${TARGET}" \
  PKOS_DATA_DIR_TARGET="${DATA_DIR}" \
  node -e '
    const fs = require("node:fs");
    const path = require("node:path");
    const root = process.env.PKOS_WORKSPACE_ROOT;
    const registry = {
      workspaces: [{
        id: process.env.PKOS_WORKSPACE_ID,
        name: path.basename(root),
        rootPath: root,
        lastOpenedAt: new Date().toISOString(),
      }],
      settings: {
        defaultWorkspaceRoot: null,
        recentWorkspaceRoots: [root],
        theme: "light",
        language: "",
      },
    };
    fs.writeFileSync(
      path.join(process.env.PKOS_DATA_DIR_TARGET, "workspaces.json"),
      `${JSON.stringify(registry, null, 2)}\n`,
    );
  '
  echo "隔离应用数据目录已准备：${DATA_DIR}"
  echo "启动示例：PKOS_DATA_DIR=\"${DATA_DIR}\" <应用可执行文件>"
fi
echo "下一步：启动应用，打开该知识库，记录完整窗口首屏时间与峰值 RSS。"
