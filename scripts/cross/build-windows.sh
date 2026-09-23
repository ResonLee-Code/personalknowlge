#!/bin/sh
# 在 macOS / Linux 主机上交叉编译 Windows 版 Personal Knowledge OS。
#
# 用法：
#   ZIG=/path/to/zig scripts/cross/build-windows.sh [--debug]
#
# 前置条件：
#   1. rustup target add x86_64-pc-windows-gnu
#      （如果 rustup 官方源不可达，可先设置
#       RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static）
#   2. 下载 Zig（提供 clang + mingw-w64 sysroot + resource compiler）
#
# 产物：src-tauri/target/x86_64-pc-windows-gnu/<profile>/personal-knowledge-os.exe
#
# 说明：该 .exe 是未签名的交叉编译产物，安装包（NSIS）请在 Windows 上生成，
# 或使用 .github/workflows/windows-release.yml（推荐，可产出并验证安装包）。
set -eu

ROOT_DIR="$(cd "$(dirname "$0")/../.." && pwd)"
CROSS_DIR="$ROOT_DIR/scripts/cross"
PROFILE="release"
PROFILE_FLAG="--release"

if [ "${1:-}" = "--debug" ]; then
  PROFILE="debug"
  PROFILE_FLAG=""
  shift
fi

if [ -z "${ZIG:-}" ]; then
  echo "错误：请设置 ZIG 环境变量指向 zig 可执行文件" >&2
  exit 1
fi

chmod +x "$CROSS_DIR"/*.sh

echo "==> 使用 Zig：$ZIG"
"$ZIG" version

# mingw-w64 工具链软链接（dlltool/ar/ranlib/windres/cc 均由 Zig 提供）
SHIM_DIR="$("$CROSS_DIR/make-tool-shims.sh" /tmp/pkos-win-tools/bin)"
PATH="$SHIM_DIR:$PATH"
export PATH

# 交叉编译时 rustc 不会把 -l 交给链接器兜底搜索，需要真实的导入库文件。
# webview2-com-sys 与 windows-* crate 只提供 MSVC 命名（*.lib），
# 这里生成 GNU 命名的副本（libxxx.a / libxxx.dll.a）。
LIB_DIR="$("$CROSS_DIR/make-win-import-libs.sh" /tmp/pkos-win-libs "$ROOT_DIR")"
export RUSTFLAGS="${RUSTFLAGS:-} -L native=$LIB_DIR"

export CC_x86_64_pc_windows_gnu="$CROSS_DIR/zig-cc-win.sh"
export AR_x86_64_pc_windows_gnu="$CROSS_DIR/zig-ar-win.sh"
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$CROSS_DIR/zig-cc-win.sh"
export RC="$CROSS_DIR/zig-rc-win.sh"
export ZIG

echo "==> 编译 Windows 目标（${PROFILE}）"
cd "$ROOT_DIR/src-tauri"
# 只构建可执行文件目标：桌面版不需要 staticlib / cdylib（那是移动端用途），
# 同时避免 Zig 链接器处理 cdylib 的 .def 参数。
cargo build --target x86_64-pc-windows-gnu ${PROFILE_FLAG} --bin personal-knowledge-os "$@"

BIN="$ROOT_DIR/src-tauri/target/x86_64-pc-windows-gnu/$PROFILE/personal-knowledge-os.exe"
echo "==> 产物：$BIN"
ls -la "$BIN"
