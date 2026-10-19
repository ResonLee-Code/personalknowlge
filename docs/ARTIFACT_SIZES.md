# 产物体积对照表（Track E3）

回答一个问题：**"这个应用有多大"**——按**平台 + 产物形态**逐行给出，每行都附**复现命令**。

这份表存在的原因：以前体积数字零散地写在 CHANGELOG / 安装指南 / 构建速查里，
彼此**口径不同**（有的量二进制、有的量 `.dmg`、有的量解压后的目录），
于是出现"同一个版本三个数字"的情况。现在口径固定、生成方式固定、缺失如实标注。

## 0. 一条命令取数

```bash
scripts/size-report.sh          # 打印 Markdown 表（只读）
scripts/size-report.sh --json   # 机器可读
```

脚本只**量本机真实存在的产物**；不存在的形态写「**未产出**」，不拿别的数字顶替。

## 1. 口径（改口径必须和这张表一起改）

| 项 | 口径 |
| --- | --- |
| 二进制 / 压缩包 | `stat -f%z` 的**文件字节数**（精确值，不受磁盘块对齐影响） |
| 目录形态（前端 `dist`、`.app`） | 目录内**所有文件的文件字节数之和**（**不用 `du`**——`du` 报的是块占用，会随文件系统块大小漂移） |
| 跨平台比较 | **不可直接比**：不同平台、不同链接方式、不同打包器，差个几 MB 很正常 |
| 跨版本比较 | **必须同平台 + 同构建配置**（都 release、都 strip+LTO）才可比 |
| `.dmg` / `.zip` / `setup.exe` | 是**压缩/封装**后的体积，和二进制不是一回事，不能混在一列里比 |

**同一份代码、两种构建配置差多少**（本机实测，说明"口径"不是小事）：

| 构建配置 | 命令 | 体积 |
| --- | --- | ---: |
| release（strip + LTO + `codegen-units=1`，V1.3.0 当时） | `pnpm tauri build --no-bundle` | **10,570,208 B（10.08 MiB）** |
| debug（未 strip、带调试信息，V1.3.0 当时） | `pnpm tauri build --debug --no-bundle` | **90,607,688 B（86.41 MiB）** |

两者差 **8.6 倍**。所以任何"体积多少"的说法，不写清楚是哪个配置，等于没说。

## 2. 本机当前产物（自动生成，2026-10-20，V1.3.6 出包演练）

机器：**macOS 27.0.1 / arm64**。生成命令：`scripts/size-report.sh`。

| 产物形态 | 平台 | 体积 | MiB | 路径（相对仓库根） |
| --- | --- | ---: | ---: | --- |
| 前端构建产物（未压缩） | 当前平台 | 7,024,266 B | 6.70 | `dist` |
| release 可执行文件（strip + LTO） | macOS arm64 | **11,940,816 B** | 11.39 | `src-tauri/target/release/personal-knowledge-os` |
| debug 可执行文件（未 strip） | macOS arm64 | 90,665,144 B | 86.47 | `src-tauri/target/debug/personal-knowledge-os` |
| macOS 应用包（`.app`） | macOS arm64 | 12,030,902 B | 11.47 | `src-tauri/target/release/bundle/macos/Personal Knowledge OS.app` |
| macOS 磁盘镜像（`.dmg`，**旧版残留**） | macOS arm64 | 5,431,245 B | 5.18 | `src-tauri/target/release/bundle/dmg/Personal Knowledge OS_1.3.0_aarch64.dmg` |
| macOS 签名（`.app` / `.dmg` 里的那份） | macOS arm64 | ad-hoc，`Identifier=com.personalknowledge.os` | — | `codesign --verify --deep --strict …` 通过；**不签就会在下载副本上报"已损坏"**，见 [BUILD_COMMANDS.md](./BUILD_COMMANDS.md) |
| Windows 可执行文件（交叉编译） | Windows x64 | 8,495,616 B | 8.10 | `src-tauri/target/x86_64-pc-windows-gnu/release/personal-knowledge-os.exe` |
| Windows 安装包（NSIS，V1.2.0 产物） | Windows x64 | 3,259,700 B | 3.11 | `dist-windows/Personal Knowledge OS_1.2.0_x64-setup.exe` |
| Windows 便携版（zip，V1.2.0 产物） | Windows x64 | 3,973,990 B | 3.79 | `dist-windows/Personal Knowledge OS_1.2.0_x64-portable.zip` |

`.app` 内的 `Info.plist` 显示 `CFBundleShortVersionString = 1.3.6`（与四处版本号一致，已核对）。

`.app` 的构成（2026-10-20 带 ad-hoc 签名后的实测）：`MacOS/personal-knowledge-os` 11,871,536 B +
`Resources/icon.icns` 155,840 B + `_CodeSignature/CodeResources` 2,440 B + `Info.plist` 1,086 B。
注意：**包内那个二进制（11,871,536 B）与独立构建出来的（11,940,816 B）不是一个数**——
打包时 tauri 会用 `codesign` 重新签一遍（带上 hardened runtime 标记），签名数据不同，别混着比。

### 2.1 已发布版本的 Release 资产（CI 实测，精确字节）

这一节按**用户下载到的资产**记账（`Content-Length` / 下载后 `stat -f%z` 一致）。
机器是 GitHub Actions：macOS 为 `macos-14`（arm64），Windows 为 `windows-latest`（x64），
Linux 为 `ubuntu-22.04`（x64）。**不可与本机 arm64 构建的数字混着比**。

| 资产 | 平台 | 字节 | MiB |
| --- | --- | ---: | ---: |
| `Personal.Knowledge.OS_1.3.1_aarch64.dmg` | macOS arm64 | 6,971,535 | 6.65 |
| `Personal.Knowledge.OS_1.3.1_x64-setup.exe` | Windows x64（NSIS） | 5,822,259 | 5.55 |
| `Personal.Knowledge.OS_1.3.1_x64-portable.zip` | Windows x64（便携） | 7,019,895 | 6.69 |
| `Personal.Knowledge.OS_1.3.1_amd64.deb` | Linux x64 | 7,740,832 | 7.38 |
| `Personal.Knowledge.OS-1.3.1-1.x86_64.rpm` | Linux x64 | 7,741,952 | 7.38 |
| `Personal.Knowledge.OS_1.3.1_amd64.AppImage` | Linux x64 | 85,723,640 | 81.75 |

> 对照 V1.3.0（同为 CI macOS arm64）：`.dmg` 5,431,245 → 6,971,535 B（**+1,540,290 B**）。
> 增量主要来自 V1.3.1 新增的图形渲染链路（mermaid / elk / cytoscape 等前端依赖，
> 前端 `dist` 从 1.9 MB 涨到 6.7 MB，构建后内嵌进二进制）。

**V1.3.2**（纯布局修复，CI 资产）：

| 资产 | 平台 | 字节 | 相对 V1.3.1 |
| --- | --- | ---: | ---: |
| `Personal.Knowledge.OS_1.3.2_aarch64.dmg` | macOS arm64 | 6,972,175 | +640 B |
| `Personal.Knowledge.OS_1.3.2_x64-setup.exe` | Windows x64（NSIS） | 5,820,475 | −1,784 B |
| `Personal.Knowledge.OS_1.3.2_x64-portable.zip` | Windows x64（便携） | 7,019,936 | +41 B |
| `Personal.Knowledge.OS_1.3.2_amd64.deb` | Linux x64 | 7,741,508 | +676 B |
| `Personal.Knowledge.OS-1.3.2-1.x86_64.rpm` | Linux x64 | 7,742,677 | +725 B |
| `Personal.Knowledge.OS_1.3.2_amd64.AppImage` | Linux x64 | 85,727,736 | +4,096 B |

> 几百字节到几 KB 的差异是打包时间戳 / 压缩分块带来的，**不是功能增量**；
> 本机 release 二进制在 V1.3.1 与 V1.3.2 之间字节数完全相同（见 §4）。

**V1.3.5**（取消自绘标题栏、原生缩放；CI 资产）：

| 资产 | 平台 | 字节 |
| --- | --- | ---: |
| `Personal.Knowledge.OS_1.3.5_aarch64.dmg` | macOS arm64 | 6,973,243 |
| `Personal.Knowledge.OS_1.3.5_x64-setup.exe` | Windows x64（NSIS） | 5,820,930 |
| `Personal.Knowledge.OS_1.3.5_x64-portable.zip` | Windows x64（便携） | 7,020,572 |
| `Personal.Knowledge.OS_1.3.5_amd64.deb` | Linux x64 | 7,742,466 |
| `Personal.Knowledge.OS-1.3.5-1.x86_64.rpm` | Linux x64 | 7,743,520 |
| `Personal.Knowledge.OS_1.3.5_amd64.AppImage` | Linux x64 | 85,723,640 |

**V1.3.6**（原生全屏箭头 + 点击缩放；CI 资产）：

| 资产 | 平台 | 字节 |
| --- | --- | ---: |
| `Personal.Knowledge.OS_1.3.6_aarch64.dmg` | macOS arm64 | 6,973,069 |
| `Personal.Knowledge.OS_1.3.6_x64-setup.exe` | Windows x64（NSIS） | 5,821,680 |
| `Personal.Knowledge.OS_1.3.6_x64-portable.zip` | Windows x64（便携） | 7,020,407 |
| `Personal.Knowledge.OS_1.3.6_amd64.deb` | Linux x64 | 7,742,284 |
| `Personal.Knowledge.OS-1.3.6-1.x86_64.rpm` | Linux x64 | 7,743,418 |
| `Personal.Knowledge.OS_1.3.6_amd64.AppImage` | Linux x64 | 85,723,640 |

## 3. 每个形态"包含什么、能不能独立运行"

| 形态 | 里面是什么 | 能独立运行吗 | 复现命令 |
| --- | --- | --- | --- |
| `dist` | WebView 里加载的 HTML / JS / CSS（未压缩） | ❌ 只是前端资源，要壳去加载 | `pnpm build` |
| release 二进制 | Rust 后端 + 宿主窗口（WebView 由系统提供） | ⚠️ 能启动，但**需要 `dist/` 在位**，否则白屏 | `pnpm tauri build --no-bundle` |
| debug 二进制 | 同上 + 调试符号、未优化 | ⚠️ 同上（日常迭代用） | `pnpm tauri build --debug --no-bundle` |
| `.app` | 上述二进制 + `icon.icns` + `Info.plist` + 资源 | ✅ 双击即用（macOS 最小发布形态） | `pnpm tauri build --bundles app` |
| `.dmg` | 压缩后的 `.app` + `/Applications` 快捷方式（供拖拽安装） | ✅ 挂载后拖进应用程序 | `pnpm tauri build --bundles dmg` |
| Windows `.exe` | 交叉编译出的 Windows 可执行文件（未签名） | ⚠️ 需随包 `dist/` 一起（Tauri 已内嵌前端资源，见下） | `ZIG=… scripts/cross/build-windows.sh` |
| Windows `setup.exe` / `portable.zip` | NSIS 安装包 / 便携版压缩包 | ✅ | `scripts/windows/build-installer.sh <版本>` |

> 说明：Tauri 会把前端资源**内嵌**进二进制，所以发布形态（`.app` / `.exe`）**不需要**随附 `dist`；
> 只有直接跑 `target/{release,debug}` 里的裸二进制时才需要 `dist/` 在位。

## 4. 版本趋势（同口径：macOS arm64 release 二进制）

| 版本 | 体积 | 来源 |
| --- | ---: | --- |
| V1.2.0 | 7,787,328 B（7.43 MiB） | **记录值**（该版构建时实测，产物已不在本机、仓库也无 tag，**无法复测**） |
| V1.2.1 | 9,842,176 B（9.39 MiB） | **记录值**（2026-10-11 实测，同上不可复测） |
| **V1.3.0** | **10,570,208 B（10.08 MiB）** | 本次实测（2026-10-14，`pnpm tauri build --bundle app` 的 release 二进制） |
| **V1.3.1** | **11,940,704 B（11.39 MiB）** | 实测（2026-10-19，本机 release 二进制；相对 V1.3.0 主要是图形渲染依赖） |
| **V1.3.2** | **11,940,704 B（11.39 MiB）** | 实测（2026-10-19，纯布局修复；版本号等长，二进制字节数与 V1.3.1 相同） |
| **V1.3.3** | **11,940,720 B（11.39 MiB）** | 实测（2026-10-19，布局与全屏控制；+16 B） |
| **V1.3.4** | **11,940,720 B（11.39 MiB）** | 实测（2026-10-19，视觉对齐；字节数与 V1.3.3 相同） |
| **V1.3.5** | **11,940,736 B（11.39 MiB）** | 实测（2026-10-19，删除自绘标题栏 + macOS 原生缩放；+16 B） |
| **V1.3.6** | **11,940,816 B（11.39 MiB）** | 实测（2026-10-20，原生全屏箭头 + toggleFullScreen→zoom；+80 B） |

差值：V1.2.0 → V1.2.1 **+2,054,848 B**；V1.2.1 → V1.3.0 **+728,032 B**；
V1.3.0 → V1.3.1 **+1,370,496 B**；V1.3.1 → V1.3.2 **0 B**；V1.3.2 → V1.3.3 **+16 B**；
V1.3.3 → V1.3.4 **0 B**；V1.3.4 → V1.3.5 **+16 B**；V1.3.5 → V1.3.6 **+80 B**。

**这些差值归因到功能时要小心**：仓库**没有任何 tag**，V1.2.0 / V1.2.1 的源码状态无法重建，
上面的旧值只能引用当时的记录，**不能**现在重新量一遍。能确证的只有两件事：

1. 当前工作树相对 HEAD **没有**新增任何依赖（`src-tauri/Cargo.toml` 与 `Cargo.lock` 无改动），
   所以 V1.2.1 → V1.3.0 的 +728,032 B 是**自有代码**（决策档 / 校准探针 / 向量检索 / 分块索引 /
   证据充分性）带来的；
2. V1.2.0 → V1.2.1 的 +2 MB 在当时被记录为网页剪藏与联网检索引入的
   `ureq`（rustls + charset）与 `dom_query`（html5ever）——这是**当时的口径**，
   本条属于引用历史记录，不是本次复测。

**要根治"口径不明"**：以后每个版本的发布产物按平台留档（Windows 已有 `dist-windows/` 的先例），
macOS 侧同理留 `dist-macos/`，这样下一个版本能拿**同一台机器上真实存在的两个产物**直接比。

### 4.1 体积 vs 功能：钱花在哪、关掉会怎样

先把一件事说清楚，否则这张表一定会被误读：

> **本项目的"可选项"是运行时可选，不是构建期可选。**
> 决策模型、向量嵌入、联网检索、网页剪藏——不配置就不联网、不调用、不产生费用，
> 但**它们的代码始终在二进制里**，关掉不会让包变小。想让包变小，只有改代码/删依赖一条路。

已实测的构成（本机，2026-10-19）：

| 层 | 体积 | 说明 |
| --- | ---: | --- |
| release 二进制 | 11,940,816 B | 后端 + 宿主窗口；**前端资源已内嵌** |
| `.app` 包 | 12,030,902 B | = 包内二进制 11,871,536 B + `icon.icns` 155,840 B + `CodeResources` 2,440 B + `Info.plist` 1,086 B |
| 前端 `dist`（未压缩源） | 7,024,266 B | 构建期输入；发布时被内嵌进二进制，**不是**额外增量 |
| debug 二进制 | 90,658,408 B | 同码不同配置，见 §1 |

**已知的体积代价（历史记录值，不是本次实测）**：

| 能力 | 代价 | 来源 |
| --- | --- | --- |
| 网页剪藏 + 联网检索（`ureq` rustls/charset + `dom_query`/html5ever） | V1.2.0 → V1.2.1 二进制 **+2,054,848 B**（+1.96 MiB），当时记录其中约 **+1.39 MB** 归剪藏两个依赖 | 引用 [CHANGELOG.md](./CHANGELOG.md) 与 [PLAN_V1.2.1.md](./PLAN_V1.2.1.md) 的当时记录 |
| 决策档 / 校准探针 / 向量检索 / 分块索引 / 证据充分性 | V1.2.1 记录值 → V1.3.0 **+728,032 B**（+0.69 MiB）；这一批**没有新增任何依赖**（`src-tauri/Cargo.toml`、`Cargo.lock` 相对 HEAD 只有版本号改动），增量是自有代码 | 本次实测 + `git diff` |

**"移掉可选项后功能不回退"这条验收怎么落**：本轮**没有删除任何能力或依赖**，
所以不存在"为省体积而回退功能"的情况——证据是全量回归绿
（前端 64 文件 / 344 用例、Rust 全套、E2E 主流程、`pnpm bench` 越线即失败）。
真要动体积，先看上面那行：**唯一一次大幅增长来自剪藏/联网检索这两个依赖**，
它们是"可选能力"而非"可选依赖"，删掉就等于删功能——那时必须**先有你的指令**，
并同步更新本节与 [CHANGELOG.md](./CHANGELOG.md)。

## 5. 本机 `.dmg`：两种产出方式与一个误导性错误

`pnpm tauri build --bundles dmg` 在本机**失败**（2026-10-14 复核，V1.3.0 演练时仍是这个结果），
原因是 `create-dmg` 要用 AppleScript 摆 Finder 窗口布局（背景图、图标位置这些**装饰**），
而本机未授权自动化：`execution error: 未获得授权将 Apple 事件发送给 Finder。（-1743）`。

**不授权也能出可用的 `.dmg`**：手动跑同一个脚本并加 `--skip-jenkins`（跳过装饰步骤）。
2026-10-14 用它产出并**验过**（挂载 → 里面有 `Personal Knowledge OS.app` 与 `Applications` 链接 →
应用版本 `1.3.0` → 正常卸载卸载），命令见 [BUILD_COMMANDS.md](./BUILD_COMMANDS.md) 的
「本地出 `.dmg`（跳过装饰）」。三种路径的分工：

| 路径 | 产物 | 装饰（背景图 / 图标排布） | 用途 |
| --- | --- | --- | --- |
| `pnpm tauri build --bundles dmg`（默认） | `.dmg` | ✅ | 需要先授权「自动化 → 访达」 |
| 同一脚本 + `--skip-jenkins` | `.dmg` | ❌ | **本地出包演练**（不授权也能跑） |
| CI 的 macOS job（`--bundles app dmg`） | `.dmg` | ✅ | **发布产物**（runner 有图形会话） |

还有一个**误导性的错误**值得记住：失败时脚本会把临时镜像 `rw.<PID>.<名字>.dmg` 留在**源目录**
（`src-tauri/target/release/bundle/macos/`）。下一次 `hdiutil create -srcfolder <源目录>`
会把这个几十 MB 的半成品一起算进镜像，报 **`hdiutil: create failed - 设备上无剩余空间`**
——而磁盘其实空着 700 GB。真正的修法是先把残留移走：

```bash
ls src-tauri/target/release/bundle/macos/rw.*.dmg   # 有残留就先移走，再重试
```

## 6. 复现清单

```bash
pnpm build                              # 前端 dist
pnpm tauri build --no-bundle            # macOS release 二进制
pnpm tauri build --debug --no-bundle    # macOS debug 二进制（对照口径用）
pnpm tauri build --bundles app          # macOS .app
pnpm tauri build --bundles dmg          # macOS .dmg（需 Finder 自动化授权）
scripts/size-report.sh                  # 把上面产出的东西量成一张表
```

## 7. 相关文档

- [BUILD_COMMANDS.md](./BUILD_COMMANDS.md)：构建与打包命令速查
- [RELEASE_PROCESS.md](./RELEASE_PROCESS.md)：发版步骤与 CI 流水线
- [ACCEPTANCE_V1.2.1.md](./ACCEPTANCE_V1.2.1.md)：本版验收清单
