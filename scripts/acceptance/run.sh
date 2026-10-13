#!/usr/bin/env bash
# E 节真机验收：一条命令入口（Track E1）。
#
#   scripts/acceptance/run.sh --list        # 看用例
#   scripts/acceptance/run.sh --preflight   # 自检权限与环境
#   scripts/acceptance/run.sh --dry-run     # 只出结论表骨架
#   scripts/acceptance/run.sh               # 真跑（应用需在运行 + 已授权辅助功能）
#
# 参数原样透传给 run.js；详见 scripts/acceptance/README.md
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec node "${SCRIPT_DIR}/run.js" "$@"
