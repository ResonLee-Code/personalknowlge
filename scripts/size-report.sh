#!/usr/bin/env bash
# E3 体积对照表：把「当前机器上真实存在的产物」量出来，输出一段可直接贴进文档的 Markdown 表。
#
#   scripts/size-report.sh            # 打印表格（只读，不改任何文件）
#   scripts/size-report.sh --json     # 机器可读（给脚本/CI 用）
#
# 口径（写进 docs/ARTIFACT_SIZES.md，改口径必须两处一起改）：
#   - **二进制/压缩包**：`stat` 的文件字节数（精确值，不受块对齐影响）；
#   - **目录形态（前端 dist、.app 包）**：目录内**所有文件的文件字节数之和**
#     （不用 `du`——`du` 报的是磁盘块占用，会随文件系统块大小漂移）；
#   - 体积随 **平台 + 构建配置 + 版本** 三者变化，换任何一个都不可直接比；
#   - 产物不存在就如实标「未产出」，不拿别的数字顶替（这正是 E3 当初被点名的毛病）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

MODE="${1:-table}"

# file_bytes <路径>：文件 → 字节；目录 → 目录内所有文件字节之和；不存在 → 空
file_bytes() {
  local target="$1"
  if [[ -f "${target}" ]]; then
    stat -f%z "${target}"
  elif [[ -d "${target}" ]]; then
    find "${target}" -type f -exec stat -f%z {} \; | awk '{sum += $1} END {printf "%d", sum + 0}'
  else
    printf ''
  fi
}

mib() {
  awk -v bytes="$1" 'BEGIN { printf "%.2f", bytes / 1048576 }'
}

platform() {
  printf '%s / macOS %s' "$(uname -m)" "$(sw_vers -productVersion 2>/dev/null || echo '未知')"
}

# 形态|平台|路径|说明
ROWS=(
  '前端构建产物（未压缩）|当前平台|dist|WebView 里跑的 HTML/JS/CSS；不是可独立运行的产物'
  'release 可执行文件（strip + LTO）|macOS arm64|src-tauri/target/release/personal-knowledge-os|`pnpm tauri build --no-bundle` 的产物'
  'debug 可执行文件（未 strip、带调试信息）|macOS arm64|src-tauri/target/debug/personal-knowledge-os|`pnpm tauri build --debug --no-bundle`；日常迭代用，不是发布形态'
  'macOS 应用包（.app）|macOS arm64|src-tauri/target/release/bundle/macos/Personal Knowledge OS.app|`pnpm tauri build --bundles app`；双击可运行的最小发布形态'
  'macOS 磁盘镜像（.dmg）|macOS arm64|src-tauri/target/release/bundle/dmg/Personal Knowledge OS_1.3.0_aarch64.dmg|`pnpm tauri build --bundles dmg`；需要 Finder 自动化授权'
  'Windows 可执行文件（交叉编译）|Windows x64|src-tauri/target/x86_64-pc-windows-gnu/release/personal-knowledge-os.exe|`scripts/cross/build-windows.sh`（需 Zig）'
  'Windows 安装包（NSIS）|Windows x64|dist-windows/Personal Knowledge OS_1.2.0_x64-setup.exe|`scripts/windows/build-installer.sh <版本>`'
  'Windows 便携版（zip）|Windows x64|dist-windows/Personal Knowledge OS_1.2.0_x64-portable.zip|同上的脚本一并产出'
)

if [[ "${MODE}" == "--json" ]]; then
  printf '{"platform":"%s","generatedAt":"%s","artifacts":[' "$(platform)" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  first=1
  for row in "${ROWS[@]}"; do
    IFS='|' read -r form plat path note <<<"${row}"
    bytes="$(file_bytes "${path}")"
    [[ "${first}" -eq 1 ]] || printf ','
    first=0
    printf '{"form":"%s","platform":"%s","path":"%s","bytes":%s}' \
      "${form}" "${plat}" "${path}" "${bytes:-null}"
  done
  printf ']}\n'
  exit 0
fi

echo "# 产物体积实测（由 scripts/size-report.sh 生成）"
echo
echo "- 机器：$(platform)"
echo "- 生成时间：$(date '+%Y-%m-%d %H:%M')"
echo
echo '| 产物形态 | 平台 | 体积 | MiB | 路径（相对仓库根） |'
echo '| --- | --- | ---: | ---: | --- |'
for row in "${ROWS[@]}"; do
  IFS='|' read -r form plat path note <<<"${row}"
  bytes="$(file_bytes "${path}")"
  if [[ -z "${bytes}" ]]; then
    printf '| %s | %s | **未产出** | — | `%s` |\n' "${form}" "${plat}" "${path}"
  else
    printf '| %s | %s | %s B | %s | `%s` |\n' "${form}" "${plat}" "${bytes}" "$(mib "${bytes}")" "${path}"
  fi
done
echo
echo "> 「未产出」= 这台机器上没有该产物，**不是 0**，也不拿旧版本的数字顶替。"
echo '> 口径与逐项说明见 `docs/ARTIFACT_SIZES.md`；数字随机器与版本变，别跨机器比。'
