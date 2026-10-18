# 发布流程（三平台统一）

> 一条 `v*` 标签触发 macOS + Windows + Linux 三平台构建，并自动创建 GitHub Release。
> 工作流：[.github/workflows/release.yml](../.github/workflows/release.yml)

## 1. 触发方式

| 方式 | 行为 |
| --- | --- |
| 手动运行（Actions → **Release (macOS + Windows + Linux)** → Run workflow） | 只构建并上传工件，**不创建 Release**（便于验证） |
| 推送 `v*` 标签 | 先校验版本一致性，构建三平台，然后创建 Release 并附上全部产物与校验值 |

## 2. 流水线结构

```text
preflight（ubuntu-22.04）
  ├─ 标签 vX.Y.Z 与 package.json 版本一致？
  ├─ package.json / Cargo.toml / Cargo.lock / tauri.conf.json 四处版本一致？
  └─ pnpm test + cargo test（全部通过才继续）
        │
        ├── macos（macos-14 / arm64）→ .dmg              → SHA256SUMS-macos.txt
        ├── windows（windows-latest）→ NSIS .exe + 便携 .zip → SHA256SUMS-windows.txt
        └── linux（ubuntu-22.04）   → .deb + .rpm + .AppImage → SHA256SUMS-linux.txt
                │
                ▼（仅标签触发）
          release：下载三个工件 → 汇总 SHA256SUMS-all.txt → 创建 GitHub Release
```

三个构建 job 互不阻塞（除 preflight 外无依赖），因此总时长约等于最慢平台。

## 3. 发版步骤

### 分支约定（2026-10-19 起）

仓库有两个长期分支，职责不同，别混用：

| 分支 | 放什么 | 规则 |
| --- | --- | --- |
| `develop` | 日常开发 / 集成 | 所有功能与修复先落这里；本地自检（test / lint / build / e2e / 真机验收）在它上面跑 |
| `main` | **已发布 / 待打标签**的状态 | 只在发版时从 `develop` 快进合并（`git merge --ff-only develop`）；标签**只打在 `main` 上** |

发版那一刻两边一致（标签打在同一个提交上）；发布后 `develop` 继续往前走，下一次发版再快进合并。
**不要**把功能提交直接推到 `main`，也**不要**在 `develop` 上打 `v*` 标签。

```bash
# 发版（在 develop 上改完版本号与 CHANGELOG、自检全绿之后）
git switch main && git merge --ff-only develop
git tag -a vX.Y.Z -m "Personal Knowledge OS vX.Y.Z"
git push origin main vX.Y.Z
git switch develop            # 之后继续在 develop 上开发
```

```bash
# 1) 统一版本号（四处：package.json / Cargo.toml / Cargo.lock / tauri.conf.json）
#    并同步 docs/CHANGELOG.md 的版本段与「当前构建」
# 2) 本地回归
cargo test --offline --manifest-path src-tauri/Cargo.toml
pnpm test && pnpm build && pnpm test:e2e
# 3) 性能守护（1 万检索 + 3 万图谱，超阈值即失败；数字抄进 CHANGELOG）
pnpm bench
# 4) 提交并打标签
git add -A && git commit -m "release: VX.Y.Z"
git tag vX.Y.Z && git push origin main --tags
```

推送标签后流水线自动跑完并建 Release；若只想验证工件，用「手动运行」。

### 发布后抽验（2026-10-14 加，血泪教训）

流水线绿**不等于**发布物没问题。v1.3.0 发布后按「用户会怎么验」实测，发现 macOS 那份校验文件
**自己过不了自己的校验**（混进一行 `du` 输出，`shasum -c` 报格式错并以非 0 退出），
而且三份清单里写的是 CI 产物路径、与用户下载到的资产名对不上。所以发布后**必须**抽一次：

```bash
# 1) 把 Release 的产物与校验文件下到一个空目录，按用户的方式校验（必须 6/6 OK 且退出码 0）
shasum -a 256 -c SHA256SUMS-all.txt
# 2) macOS：挂载 .dmg，确认签名与版本
hdiutil attach "Personal.Knowledge.OS_X.Y.Z_aarch64.dmg"
codesign --verify --deep --strict --verbose=2 "/Volumes/Personal Knowledge OS/Personal Knowledge OS.app"
plutil -p "/Volumes/Personal Knowledge OS/Personal Knowledge OS.app/Contents/Info.plist" | grep CFBundleShortVersionString
# 3) Windows 便携版里那个应用 exe（不是 NSIS 外壳）过一遍结构校验
python3 scripts/cross/verify-pe.py "<解包出来的>/Personal Knowledge OS.exe"
```

> 注意：`x64-setup.exe` 是 NSIS 安装器，**外壳是 32 位 stub 属正常**，
> 别拿 `verify-pe.py` 直接判它「不是 x64」。

## 4. 产物与校验

| 平台 | 文件 | 备注 |
| --- | --- | --- |
| macOS（arm64） | `Personal Knowledge OS_X.Y.Z_aarch64.dmg` | **ad-hoc 签名**（无 Developer ID、无公证）：下载副本首次打开会提示「无法验证开发者」，到 系统设置 → 隐私与安全性 → **仍要打开** 放行一次。若看到的是「**已损坏**」，说明这一版没走 ad-hoc 签名——见 [BUILD_COMMANDS.md](./BUILD_COMMANDS.md) 的「为什么必须有 ad-hoc 签名」 |
| Windows（x64） | `..._x64-setup.exe`（NSIS）、`..._x64-portable.zip` | 未签名，SmartScreen 需放行 |
| Linux（x64） | `*.deb`、`*.rpm`、`*.AppImage` | 需 WebKitGTK 4.1；AppImage 需 `chmod +x` |

每个平台各自的 `SHA256SUMS-<platform>.txt` 会一并附在 Release 上，另有汇总的
`SHA256SUMS-all.txt`。验收清单见：
[当前版本（V1.3.2）](./ACCEPTANCE_V1.3.2.md) ·
[上一版（V1.3.1）](./ACCEPTANCE_V1.3.1.md) ·
[上一版（V1.3.0）](./ACCEPTANCE_V1.3.0.md) ·
[上一版（V1.2.1）](./ACCEPTANCE_V1.2.1.md) ·
[上一版产物（V1.2.0）](./ACCEPTANCE_V1.2.0.md) ·
[Windows 安装说明](./WINDOWS_INSTALL_GUIDE.md) · [Linux 安装说明](./LINUX_INSTALL_GUIDE.md)。

## 5. 注意事项

1. **版本号必须四处一致**（`package.json` / `Cargo.toml` / `Cargo.lock` / `tauri.conf.json`，
   preflight 会拦截不一致的标签）；
2. macOS 目前只构建 **arm64**（`macos-14`）。如需 Intel 包，在 `macos` job 上加矩阵
   （`macos-13`，若 GitHub 仍提供）即可，产物名会带架构后缀不会冲突；
3. Windows / Linux 产物均未签名；需要签名时在各自 job 增加证书步骤；
4. 本机（macOS）仍可用 `./scripts/linux/build-linux.sh`、`scripts/windows/*`、
   `pnpm tauri build --bundles app dmg` 做单平台出包，用于快速验证；
5. 工作流跑完前不要把标签当作「已发布」——`release` job 只在三个平台全部成功后创建 Release。
