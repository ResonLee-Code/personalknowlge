#!/usr/bin/env bash
# macOS 关闭回归：启动应用 → 点红色关闭键 → 确认没有新的 .ips 崩溃报告。
#
# 用法：
#   scripts/acceptance/close-smoke.sh [Personal Knowledge OS.app 路径]
#
# 前提：macOS 图形会话 + 终端/Codex 已获辅助功能权限。不要与其他验收同时跑。
set -euo pipefail

APP="${1:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/src-tauri/target/release/bundle/macos/Personal Knowledge OS.app}"
REPORTS="$HOME/Library/Logs/DiagnosticReports"

latest() {
  ls -t "$REPORTS"/personal-knowledge-os-*.ips 2>/dev/null | head -1 || true
}

before="$(latest)"
echo "启动：$APP"
open "$APP"
sleep 5
osascript \
  -e 'tell application "Personal Knowledge OS" to activate' \
  -e 'delay 0.4' \
  -e 'tell application "System Events" to tell process "Personal Knowledge OS" to click button 1 of window 1' \
  >/dev/null
sleep 4
after="$(latest)"

if [[ -n "$after" && "$after" != "$before" ]]; then
  echo "FAIL：关闭后出现新的崩溃报告：$after" >&2
  exit 1
fi
echo "PASS：关闭窗口后没有新的崩溃报告"
