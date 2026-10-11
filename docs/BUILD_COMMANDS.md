# 构建命令速查（只编译 / 出安装包 / 发版）

> 本文只讲**怎么把代码变成可执行文件与安装包**，不含发版检查清单（那份在
> [RELEASE_PROCESS.md](./RELEASE_PROCESS.md)）。所有命令都对应仓库里真实存在的脚本与配置，
> 可在 `package.json`、`src-tauri/tauri.conf.json`、`scripts/` 里逐一核对。

## 0. 速查表

| 我想…… | 平台 | 命令 | 产物 |
| --- | --- | --- | --- |
| 只编译可执行文件（不出包） | 当前平台 | `pnpm tauri build --no-bundle` | `src-tauri/target/release/personal-knowledge-os`（Windows 为 `.exe`） |
| 编译快到能忍 | 当前平台 | `pnpm tauri build --debug --no-bundle` | `src-tauri/target/debug/personal-knowledge-os` |
| 开发态跑起来 | 当前平台 | `pnpm tauri dev` | 无产物（热更新） |
| 出 macOS 安装包 | macOS | `pnpm tauri build --bundles app dmg` | `.dmg` + `.app` |
| 出 Windows 可执行文件 | 任意（交叉） | `ZIG=/path/to/zig scripts/cross/build-windows.sh` | `…-windows-gnu/release/personal-knowledge-os.exe` |
| 出 Windows 安装包 | macOS/Linux（交叉） | `scripts/windows/build-installer.sh [版本号]` | `dist-windows/…_x64-setup.exe` + `…-portable.zip` |
| 出 Windows 安装包 | Windows | `.\scripts\windows\build-windows.ps1` | 同上 |
| 出 Linux 安装包 | Linux（必须） | `./scripts/linux/build-linux.sh [deb rpm appimage]` | `.deb` / `.rpm` / `.AppImage` |
| 三平台一次产出 + Release | CI | `git tag vX.Y.Z && git push origin main --tags` | GitHub Release + 三平台工件 + `SHA256SUMS-*` |

## 1. 前提条件（一次性）

```bash
# 前端与 Tauri CLI
node -v                     # 建议 20+
pnpm install --frozen-lockfile

# Rust（macOS/Linux/Windows 原生构建）
rustup show
cargo --version
```

| 平台 | 额外依赖 |
| --- | --- |
| macOS | Xcode Command Line Tools（`xcode-select --install`） |
| Windows 原生 | MSVC 工具链（rustup 默认）、Node 20+、pnpm |
| Linux | `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf build-essential pkg-config libssl-dev`（`scripts/linux/build-linux.sh` 会自动用 apt 装） |
| Windows 交叉编译 | `rustup target add x86_64-pc-windows-gnu` + [Zig](https://ziglang.org/)（提供 clang / mingw-w64 sysroot / windres） |
| Windows 安装包（交叉） | NSIS 3.x（macOS：`brew install makensis`） |

本仓库为离线/受限网络做了两处配置，一般不需要额外操作：

- `.npmrc` 里 `verify-deps-before-run=false`，避免 `pnpm tauri build` 每次先尝试联网校验依赖；
- `.cargo/config.toml` 把 crates 源指向 USTC 镜像，Rust 侧可配合 `--offline`。

## 2. 场景 A：只编译可执行文件（不出安装包）

```bash
# 仓库根目录
pnpm tauri build --no-bundle
```

`--no-bundle` 是 Tauri v2 的「跳过打包」开关（`pnpm tauri build --help` 原文：
*Skip the bundling step even if `bundle > active` is `true`*）。它会先执行
`tauri.conf.json → build.beforeBuildCommand`（即 `pnpm build` = `tsc --noEmit && vite build` → `dist/`），
再执行 `cargo build --release`。

| 平台 | 可执行文件路径 | 体积（2026-10-11 实测） |
| --- | --- | --- |
| macOS arm64 | `src-tauri/target/release/personal-knowledge-os` | **9,842,176 B（9.84 MB）**；V1.2.0 为 7,787,328 B，差值是网页剪藏与联网检索引入的 `ureq` / `dom_query` |
| Windows x64（交叉编译） | `src-tauri/target/x86_64-pc-windows-gnu/release/personal-knowledge-os.exe` | 约 8.5 MB |
| Linux x64 | `src-tauri/target/release/personal-knowledge-os` | 与平台/链接方式有关 |

**实测耗时（macOS arm64，M 系列，依赖已缓存）**

| 命令 | 耗时 | 说明 |
| --- | --- | --- |
| `pnpm tauri build --no-bundle` | 约 **58 s** | 前端 0.8 s + `release` 编译 54.6 s（改动了 Rust 代码后的增量） |
| `pnpm tauri build --debug --no-bundle` | 约 **20 s** | 改了 Rust 代码后的增量；跳过 LTO / strip |

首次编译或 `cargo clean` 之后是全量构建（依赖图 + LTO），时间会显著长于上表；
只改前端（CSS / 文案 / 组件）时 Rust 侧不重编译，通常十几秒内结束。

其它等价或更细的写法：

```bash
pnpm tauri build --debug --no-bundle          # 调试二进制：target/debug/personal-knowledge-os
pnpm tauri build --no-bundle --target aarch64-apple-darwin   # 指定 target
cargo build --manifest-path src-tauri/Cargo.toml --release   # 只跑 Rust（前提：dist/ 已是最新）
pnpm build                                    # 只构建前端到 dist/
```

> ⚠️ 直接 `cargo build` 出来的裸二进制**需要 `dist/` 已经存在**（`tauri.conf.json` 的
> `frontendDist` 指向它），否则运行时会白屏；日常用 `pnpm tauri build --no-bundle` 更省心。

## 3. 场景 B：单平台出安装包

### macOS

```bash
# 只出 .app 与 .dmg（与 CI 的 macos job 完全一致）
pnpm tauri build --bundles app dmg
```

产物：

- `src-tauri/target/release/bundle/macos/Personal Knowledge OS.app`
- `src-tauri/target/release/bundle/dmg/Personal Knowledge OS_1.2.0_aarch64.dmg`

未签名，首次打开需右键 →「打开」（见 [ACCEPTANCE_V1.2.1.md](./ACCEPTANCE_V1.2.1.md) 的已知限制）。
目前只出 arm64；要 Intel 包需 `--target x86_64-apple-darwin` 并提前 `rustup target add`。

> 建议**始终显式写 `--bundles`**：`tauri.conf.json` 的 `bundle.targets` 为了跨平台发版同时列了
> `app / dmg / nsis / deb / rpm / appimage`，裸跑 `pnpm tauri build` 会去尝试当前平台不支持的打包目标，
> 白等时间甚至直接报错。CI 的三个平台 job 分别用 `--bundles app dmg`（macOS）、
> `--bundles nsis`（Windows）、`--bundles deb rpm appimage`（Linux）。

### Windows

**A. 在 macOS/Linux 上交叉编译（本仓库当前产线的方式）**

```bash
# 1) 编译 exe
rustup target add x86_64-pc-windows-gnu           # 首次
ZIG=/path/to/zig scripts/cross/build-windows.sh   # 加 --debug 可编译调试版

# 2) 校验 PE 结构（MZ/PE 头、GUI 子系统、资源节）
python3 scripts/cross/verify-pe.py \
  src-tauri/target/x86_64-pc-windows-gnu/release/personal-knowledge-os.exe

# 3) 生成 NSIS 安装包（需要 makensis）
brew install makensis            # 首次
scripts/windows/build-installer.sh 1.2.0
```

产物在 `dist-windows/`：`Personal Knowledge OS_1.2.0_x64-setup.exe`（安装包）与
`…_x64-portable.zip`（便携版）。可用环境变量覆盖：
`NSIS_PATH`（自建 makensis）、`SOURCE_DIR`（exe 目录）、`OUTPUT_DIR`（输出目录）。
CI 的 windows job 走的是等价路径：`pnpm tauri build --bundles nsis` 出安装包，
再用 PowerShell 把 `personal-knowledge-os.exe` 压成便携版 zip。

**B. 在 Windows 上原生构建**

```powershell
.\scripts\windows\build-windows.ps1               # 编译 + NSIS 安装包
.\scripts\windows\build-windows.ps1 -NoBundle     # 只要 exe
.\scripts\windows\build-windows.ps1 -DebugBuild   # 调试版
```

安装与放行说明见 [WINDOWS_INSTALL_GUIDE.md](./WINDOWS_INSTALL_GUIDE.md)。

### Linux

```bash
./scripts/linux/build-linux.sh                  # 默认 deb + rpm + appimage
./scripts/linux/build-linux.sh deb appimage     # 只出指定格式
```

- **必须在 Linux 上执行**：Tauri 不支持从 macOS/Windows 交叉编译到 Linux；
- 脚本会检查并用 apt 安装系统依赖（需要 `sudo`）；
- 产物在 `src-tauri/target/release/bundle/{deb,rpm,appimage}/`，脚本结束时打印体积与 SHA-256；
- 安装方式（WebKitGTK 4.1、AppImage 需 `chmod +x`）见 [LINUX_INSTALL_GUIDE.md](./LINUX_INSTALL_GUIDE.md)。

## 4. 场景 C：三平台一次产出 + 创建 Release

流水线：[.github/workflows/release.yml](../.github/workflows/release.yml)，
结构是 `preflight → (macos ‖ windows ‖ linux) → release`。

| 触发方式 | 行为 |
| --- | --- |
| Actions → **Release (macOS + Windows + Linux)** → *Run workflow* | 只构建并上传工件，**不创建 Release**（用于验证） |
| 推送 `v*` 标签 | 校验版本一致性 → 三平台构建 → 汇总校验值并创建 Release |

```bash
# 版本号必须四处一致：package.json / src-tauri/Cargo.toml / src-tauri/Cargo.lock / src-tauri/tauri.conf.json
cargo test --manifest-path src-tauri/Cargo.toml --offline
pnpm test && pnpm build && pnpm test:e2e
git add -A && git commit -m "release: VX.Y.Z"
git tag vX.Y.Z && git push origin main --tags
```

产物命名：`Personal Knowledge OS_X.Y.Z_aarch64.dmg`、
`…_x64-setup.exe` / `…_x64-portable.zip`、`*.deb` / `*.rpm` / `*.AppImage`，
并附各平台 `SHA256SUMS-<platform>.txt` 与汇总的 `SHA256SUMS-all.txt`。
仅手动运行（不打标签）时不会创建 Release。

## 5. 产物路径一览

```text
dist/                                   # 前端构建产物（vite build）
src-tauri/target/release/
  personal-knowledge-os                 # 可执行文件（macOS / Linux）
  bundle/macos/…app                     # macOS 应用包
  bundle/dmg/…dmg                       # macOS 安装镜像
  bundle/{deb,rpm,appimage}/…           # Linux 包
src-tauri/target/debug/personal-knowledge-os          # --debug 产物
src-tauri/target/x86_64-pc-windows-gnu/release/*.exe  # Windows 交叉编译可执行文件
dist-windows/                           # Windows 安装包与便携版（NSIS 脚本输出）
```

## 6. 常见问题

| 现象 | 原因与处理 |
| --- | --- |
| `pnpm tauri build` 卡在联网校验依赖 | 确认没删掉 `.npmrc` 里的 `verify-deps-before-run=false` |
| Rust 依赖拉不下来 | 使用 `.cargo/config.toml` 的镜像；必要时加 `--offline`（依赖已在本地缓存时） |
| 编译很慢 | `[profile.release]` 开了 `lto = true` + `codegen-units = 1` + `strip`（体积换时间）。迭代期用 `--debug` 或 `--no-bundle`；只在出包时跑 release |
| 交叉编译 Windows 报找不到 `dlltool/windres` | 必须先设 `ZIG=/path/to/zig`，脚本会为 mingw-w64 建 Zig 工具链软链 |
| `build-installer.sh` 报找不到 `makensis` | `brew install makensis`，或用 `NSIS_PATH` 指向自建 NSIS 目录 |
| 运行时白屏（裸二进制） | `dist/` 缺失或过期，先 `pnpm build` 再用 `pnpm tauri build --no-bundle` |
| 想彻底重来 | `cargo clean --manifest-path src-tauri/Cargo.toml`（清 Rust 缓存，下一次全量编译），必要时再删 `dist/`。**注意**：这两个目录删除后需要重新编译，不要在出包验收前顺手清 |
| 版本号不一致导致 preflight 失败 | 四处版本（见场景 C）必须一致，`tauri.conf.json` 里现在是 `1.2.0` |

## 7. 相关文档

- [RELEASE_PROCESS.md](./RELEASE_PROCESS.md)：发版流程与 CI 流水线细节
- [DEVELOPMENT_RULES.md](./DEVELOPMENT_RULES.md)：本地回归命令与文档同步规则
- [WINDOWS_INSTALL_GUIDE.md](./WINDOWS_INSTALL_GUIDE.md) / [LINUX_INSTALL_GUIDE.md](./LINUX_INSTALL_GUIDE.md)：安装与放行说明
- [ACCEPTANCE_V1.2.1.md](./ACCEPTANCE_V1.2.1.md)：当前版本的验收清单（回归矩阵 / 性能实测 / 待打包与真机项）
- [ACCEPTANCE_V1.2.0.md](./ACCEPTANCE_V1.2.0.md)：上一版产物完整性（安装包仍对应该版本）
