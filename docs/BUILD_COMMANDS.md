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
| 真机验收（E 节，一条命令） | macOS + 应用在运行 | `scripts/acceptance/run.sh` | `docs/acceptance/<时间戳>/report.md` + 截图 |
| 真机验收：只看用例 / 环境自检 / 空跑 | macOS | `scripts/acceptance/run.sh --list / --preflight / --dry-run` | 同上（`--dry-run` 只出结论表骨架） |
| 发布前性能守护（E2：1 万检索 + 3 万图谱） | 当前平台 | `pnpm bench` | 控制台结论 + `src-tauri/target/bench/*.log` |
| 看产物体积对照表（E3） | 当前平台 | `scripts/size-report.sh` | 控制台的 Markdown 表（`--json` 可机读） |
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
| macOS arm64 | `src-tauri/target/release/personal-knowledge-os` | **10,537,136 B（10.05 MiB）**，2026-10-14 实测（V1.2.1 记录值 9,842,176 B、V1.2.0 为 7,787,328 B） |
| Windows x64（交叉编译） | `src-tauri/target/x86_64-pc-windows-gnu/release/personal-knowledge-os.exe` | 8,495,616 B（8.10 MiB，V1.2.0 源码状态的交叉编译产物） |
| Linux x64 | `src-tauri/target/release/personal-knowledge-os` | 与平台/链接方式有关 |

> **只看这一行数字容易被误导**：体积随 **平台 + 构建配置 + 版本** 三者变，release 与 debug
> 能差 8 倍多。逐形态的口径、复现命令与"哪个数不能和哪个数比"，见
> [ARTIFACT_SIZES.md](./ARTIFACT_SIZES.md)（一条命令重新量：`scripts/size-report.sh`）。

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
- `src-tauri/target/release/bundle/dmg/Personal Knowledge OS_<版本>_aarch64.dmg`

#### 为什么必须有 ad-hoc 签名（2026-10-14 实测）

不配签名身份时，tauri 只留下链接器给的那份 ad-hoc 签名，**bundle 的签名封条是坏的**：

```bash
codesign --verify --deep --strict "…/Personal Knowledge OS.app"   # ❌ code has no resources but signature indicates they must be present
spctl --assess --type execute "…/Personal Knowledge OS.app"       # ❌ 同上
```

后果不是"提示未验证开发者"，而是**"已损坏，无法打开。你应该将它移到废纸篓"**——
那条弹窗里**没有「打开」选项**，用户会以为下载坏了（实测：把 app 打上 `com.apple.quarantine`
再启动，就是这个弹窗）。配上 `bundle.macOS.signingIdentity = "-"` 之后：

| | 不配（只靠链接器签名） | 配 `-`（显式 ad-hoc 签名） |
| --- | --- | --- |
| `codesign --verify --deep --strict` | ❌ 封条无效 | ✅ `valid on disk` / `satisfies its Designated Requirement` |
| 签名标识 | `personal_knowledge_os-<hash>` | `com.personalknowledge.os` |
| 下载后首次打开 | ❌ **"已损坏"**（无「打开」选项） | ⚠️ **"无法验证开发者"**（可去 系统设置 → 隐私与安全性 → 仍要打开） |

两条纪律：① 这是**ad-hoc** 签名，不是 Developer ID——`spctl --assess` 仍会 `rejected`，
离"双击就能开"还差一张证书；② 出包后**顺手验一次**
（`codesign --verify --deep --strict` + 打 quarantine 试启动），别等用户来报"文件损坏"。

**没有 Developer ID 签名、也没有公证**（见 [ACCEPTANCE_V1.2.1.md](./ACCEPTANCE_V1.2.1.md) 的已知限制）。
但**必须做 ad-hoc 签名**——`tauri.conf.json` 里已固定
`bundle.macOS.signingIdentity = "-"`（实测差别很大，见下方"为什么必须有 ad-hoc 签名"）。
目前只出 arm64；要 Intel 包需 `--target x86_64-apple-darwin` 并提前 `rustup target add`。

> 建议**始终显式写 `--bundles`**：`tauri.conf.json` 的 `bundle.targets` 为了跨平台发版同时列了
> `app / dmg / nsis / deb / rpm / appimage`，裸跑 `pnpm tauri build` 会去尝试当前平台不支持的打包目标，
> 白等时间甚至直接报错。CI 的三个平台 job 分别用 `--bundles app dmg`（macOS）、
> `--bundles nsis`（Windows）、`--bundles deb rpm appimage`（Linux）。

**本地出 `.dmg`（跳过装饰）**：`create-dmg` 默认会调 AppleScript 摆 Finder 窗口布局，
本机没授权「自动化 → 访达」时会报 `-1743`（见下方排错表）。不授权也能拿到**可用**的 `.dmg`
——去掉装饰步骤即可（产物能挂载、能拖进「应用程序」，只是没有背景图与图标排布）：

```bash
cd src-tauri/target/release/bundle/dmg
bash ./bundle_dmg.sh --volname "Personal Knowledge OS" --icon "Personal Knowledge OS.app" \
  180 170 --app-drop-link 480 170 --window-size 660 400 \
  --hide-extension "Personal Knowledge OS.app" --skip-jenkins \
  "Personal Knowledge OS_$(node -p "require('../../../../../package.json').version")_aarch64.dmg" \
  "$PWD/../macos"
```

> CI（GitHub runner 有图形会话）走的是**默认**路径，产出的 `.dmg` 带装饰；
> 本地这条只用于出包演练与"先看产物长什么样"，不作为发布产物。

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
| 出 `.dmg` 报 `hdiutil: create failed - 设备上无剩余空间`（磁盘明明空着） | **误导性错误**：上一次失败把半成品临时镜像 `rw.<PID>.<名字>.dmg` 留在了**源目录**（`src-tauri/target/release/bundle/macos/`），这一次它被当成要打包的内容，撑爆了镜像。先移走残留文件再重跑 |
| 出 `.dmg` 报 `未获得授权将 Apple 事件发送给 Finder。（-1743）` | `create-dmg` 要用 AppleScript 摆 Finder 窗口布局（背景图、图标位置这些**装饰**）。两条路：① 授权——系统设置 → 隐私与安全性 → **自动化** → 允许终端 / Codex 控制「访达」，之后 `pnpm tauri build --bundles dmg` 直接可用；② **不授权也能出可用的 `.dmg`**：手动跑一次脚本并加 `--skip-jenkins`（跳过装饰步骤），命令见下方「本地出 `.dmg`（跳过装饰）」 |

## 7. 发布前性能守护（E2：一条命令，超阈值即失败）

```bash
pnpm bench                                     # 发布规格：1 万篇检索 + 3 万篇图谱
pnpm bench -- --search-notes 2000 --graph-notes 3000   # 冒烟（约 10 s）
pnpm bench -- --only search                    # 只跑检索（或 --only graph）
```

脚本是 `scripts/bench/run.sh`，它依次跑两个已有基准（`src-tauri/tests/search_bench.rs`、
`src-tauri/tests/graph_bench.rs`，平时靠 `--ignored` 挂起），并对**超阈值以非零码退出**：

| 指标 | 红线 | 覆盖变量 |
| --- | --- | --- |
| 检索 p95 延迟（1 万篇） | < 200 ms | `BENCH_MAX_P95_MS` |
| 检索 recall@5（1 万篇） | ≥ 100% | `BENCH_MIN_RECALL_PCT` |
| 图谱全量查询（3 万篇） | < 1500 ms（M3 口径） | `BENCH_MAX_GRAPH_MS` |
| 扫描采纳超时 | < 600 s | `BENCH_SCAN_TIMEOUT_S` |

两个基准各自会在末尾打印一行机器可读的 `PKOS_BENCH search …` / `PKOS_BENCH graph …`；
脚本**核对这一行是否存在**——基准没真的跑起来（例如测试名过滤写错、0 个用例）会被判失败，
不会出现"什么都没跑也算通过"。日志与摘要落在 `src-tauri/target/bench/`（在 `.gitignore` 里）。

本机实测（2026-10-13，macOS arm64，`pnpm bench` 约 56 s）：
检索 1 万篇 **p95 47.4 ms / 平均 20.0 ms / recall@5 100%**、库 34.5 MB；
图谱 3 万篇全量 **895.9 ms**、分类过滤 351.7 ms、2 跳聚焦 669.3 ms。数字用于发版记录，
不是硬编码期望值——**换机器只比红线，不比具体毫秒**。

## 8. 相关文档

- [GUIDE.md](./GUIDE.md) / [GUIDE.en.md](./GUIDE.en.md)：**完全指南**（面向使用者：安装、上手、AI、排错）
- [RELEASE_PROCESS.md](./RELEASE_PROCESS.md)：发版流程与 CI 流水线细节
- [ARTIFACT_SIZES.md](./ARTIFACT_SIZES.md)：产物体积对照表（口径 / 逐形态 / 复现命令，E3）
- [DEVELOPMENT_RULES.md](./DEVELOPMENT_RULES.md)：本地回归命令与文档同步规则
- [WINDOWS_INSTALL_GUIDE.md](./WINDOWS_INSTALL_GUIDE.md) / [LINUX_INSTALL_GUIDE.md](./LINUX_INSTALL_GUIDE.md)：安装与放行说明
- [ACCEPTANCE_V1.3.3.md](./ACCEPTANCE_V1.3.3.md)：**当前版本**（V1.3.3）的验收记录（发布前守护 / 产物与签名 / 发布后抽验）
- [ACCEPTANCE_V1.3.0.md](./ACCEPTANCE_V1.3.0.md)：上一版（V1.3.0）的验收记录
- [ACCEPTANCE_V1.2.1.md](./ACCEPTANCE_V1.2.1.md)：上一版的验收清单（回归矩阵 / 性能实测 / 已知限制）
- [ACCEPTANCE_V1.2.0.md](./ACCEPTANCE_V1.2.0.md)：上一版产物完整性（安装包仍对应该版本）
