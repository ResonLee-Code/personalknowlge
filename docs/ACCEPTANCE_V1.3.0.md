# V1.3.0 验收记录

口径与 [ACCEPTANCE_V1.2.1.md](./ACCEPTANCE_V1.2.1.md) 一致：**没验的不写成通过**；
只能人眼判断的标「待人工」；不确定的写清"哪里还不确定"。

本文件记录 **V1.3.0** 出包前后的验证实况（2026-10-14，macOS 27 / arm64 本机）。

## A. 发布前守护（✅ 本机全绿）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 版本一致性（四处） | ✅ `1.3.0` | `package.json` / `src-tauri/Cargo.toml` / `Cargo.lock` / `tauri.conf.json`；`pnpm test` 的版本身份守卫 + CI preflight 逻辑本地模拟 |
| 前端 | ✅ 66 文件 / 353 用例 | `pnpm test`；`pnpm lint`、`pnpm build` 通过 |
| Rust | ✅ 47 个测试目标全绿 | `cargo test --offline` |
| E2E 主流程 | ✅ 1 条通过 | `pnpm test:e2e` |
| 性能基准（E2） | ✅ 全部过线 | `pnpm bench`：检索 p95 **48.0 ms**、recall@5 **100%**；3 万图谱全量 **905.4 ms**（红线 1.5 s） |
| 真机验收（E1 脚本） | ✅ 自动步骤 **71 步通过 / 0 步失败** | `docs/acceptance/20261014-212209/report.md`（跑的是**已安装的发布版**）；16 次点击全部走 `AXPress` |

## B. 产物（✅ 本机产出并逐一验过）

| 产物 | 结果 | 怎么验的 |
| --- | --- | --- |
| `.app` | ✅ 10,668,262 B，`CFBundleShortVersionString = 1.3.0` | `pnpm tauri build --bundles app`（tauri 日志显示 `Signing with identity "-"`） |
| `.dmg` | ✅ 5,431,245 B | 本机默认路径需要「自动化 → 访达」授权（否则 `-1743`），改加 `--skip-jenkins` 产出；**已挂载验证**：里面有 `.app` 与 `Applications` 链接，卸载后卷正常卸载 |
| ad-hoc 签名 | ✅ `valid on disk` / `satisfies its Designated Requirement` | `codesign --verify --deep --strict`；`Identifier=com.personalknowledge.os` |

体积逐项对照见 [ARTIFACT_SIZES.md](./ARTIFACT_SIZES.md)（含 debug/release 口径差异、Windows 产物等）。

## B2. 发布（✅ 2026-10-14）

- 仓库：`https://github.com/ResonLee-Code/personalknowlge`；`main` 与标签 `v1.3.0` 已推送；
- **Release**：<https://github.com/ResonLee-Code/personalknowlge/releases/tag/v1.3.0>
  —— macOS `.dmg`、Windows `setup.exe` + `portable.zip`、Linux `.deb`/`.rpm`/`.AppImage`，外加 4 份 `SHA256SUMS-*.txt`；
- **发布产物抽验（不是"CI 绿了就算"）**：把 Release 里的 `.dmg` 下载回来 →
  sha256 与 `SHA256SUMS-macos.txt` **一致** → 挂载后 `codesign --verify --deep --strict` **通过**
  （`Identifier=com.personalknowledge.os`、版本 `1.3.0`）；`spctl --assess` 仍 `rejected`（预期，需用户放行一次）；
- 流水线首跑修掉 5 个"只在干净环境现形"的仓库缺陷（pnpm 版本冲突、Node 版本、esbuild 构建批准、
  归档链接守卫、preflight 缺 Linux 依赖），清单见 [CHANGELOG.md](./CHANGELOG.md) 的「出包」一节。

## C. 安装 / 卸载（✅ 本机演练；🖐 干净机器待补）

本机做的（2026-10-14，**用的是从 GitHub Release 真实下载的 `.dmg`**）：

1. **装**：把发布版 `.dmg` 里的 `.app` 拷进 `/Applications` → `codesign --verify --deep --strict` 通过、
   `CFBundleShortVersionString = 1.3.0`；
2. **首次打开（E 节第 10 条）**：给副本打上下载标记后双击 → macOS 弹
   「**Apple 无法验证"Personal Knowledge OS"…**」（*不是*「已损坏」——这正是 ad-hoc 签名换来的可放行变体），
   且 系统设置 → 隐私与安全性 里出现「**已阻止"Personal Knowledge OS"以保护 Mac。**」与放行入口；
   放行后应用**正常启动出窗口**（本机用等效的 `xattr -d com.apple.quarantine` 放行：
   「仍要打开」按钮在系统设置里点不到——那是 web 化界面，AX 树几乎不暴露侧边栏，已如实记录）；
3. **清空应用数据后的"首跑"**：把 `~/Library/Application Support/PersonalKnowledgeOS` 挪走后重启 →
   欢迎页正确显示空态（「还没有记录。创建或打开一个目录后会显示在这里。」），并**自动重建**应用数据目录；
4. **首次建库**：填目录 → 创建 → 工作区结构完整（`.knowledge/{config.json,database.sqlite,index,logs,templates,backups,research}`
   + `00_Inbox/` 欢迎笔记）✓；
5. **用已安装的发布版跑 E 节自动验收**：**71 步自动断言通过 / 0 失败**（16 次点击全走 `AXPress`）——
   这是"发布出来的那个二进制真的能用"的最硬证据；
6. **卸载**：退出 → 把 `/Applications` 里的应用移除 → 知识库数据（32 篇笔记 + `.knowledge`）
   与应用数据（4 条「最近打开」）**完好**，重装后列表还在。

> 这一步又抓到**脚手架自己的一个真缺陷**：跑验收时 Chrome 抢到了前台，
> 而 `paste`/`keys` 这类**键盘事件只送给最前面的应用**，于是文本打进了 Chrome、应用里标题是空的，
> 18 步断言失败——而报告里每一步都写着 ✅（"步骤通过 ≠ 动作生效"的又一次现形）。
> 已修：每个键盘类步骤前重新激活应用（`scripts/acceptance/run.js`），复跑后 71/0 通过。

**待干净机器补（A2）**：在一台**没装过本应用**的机器上（本机当前是锁屏状态，交互步骤做不了；解锁后我也可以代跑），走一遍
「从网络下载 `.dmg` → 首次打开放行 → 拖进应用程序 → 建库 / 恢复备份 → 卸载 → 确认数据仍在」，
并逐条截图。第 10 条（首次启动放行）**只对"从网络下载的包"有意义**，本机构建没有 quarantine 标记，
所以这一条脚本覆盖不了。

> 关于"首次打开放行"的实测（本机用 `xattr` 人工打上 quarantine 复现，2026-10-14）：
> - **不配签名身份**（只留链接器签名）时，弹的是「已损坏，无法打开。你应该将它移到废纸篓。」
>   ——**没有「打开」选项**，用户只能取消或移到废纸篓；
> - 配上 `bundle.macOS.signingIdentity = "-"`（ad-hoc 签名）后，弹的是「**无法验证开发者**」，
>   可以到 系统设置 → 隐私与安全性 → **仍要打开** 放行。
>
> 两种弹窗都截了图（`src-tauri/target/acceptance-debug/gk-*.png`），对照表见
> [BUILD_COMMANDS.md](./BUILD_COMMANDS.md) 的「为什么必须有 ad-hoc 签名」。

### A2 的可复制操作（给人工/给解锁后的自动化）

```bash
# 1) 下载并校验（应 6/6 OK、退出码 0）
curl -L -O https://github.com/ResonLee-Code/personalknowlge/releases/download/v1.3.0/SHA256SUMS-all.txt
for f in Personal.Knowledge.OS_1.3.0_*; do curl -L -O "https://github.com/ResonLee-Code/personalknowlge/releases/download/v1.3.0/$f"; done
shasum -a 256 -c SHA256SUMS-all.txt

# 2) 首次打开放行：挂载 .dmg → 把 app 拖进「应用程序」→ 双击
#    看到「无法验证开发者」时：系统设置 → 隐私与安全性 → 「仍要打开」→ 再确认一次
hdiutil attach Personal.Knowledge.OS_1.3.0_aarch64.dmg

# 3) 建库 / 恢复备份 / 卸载后确认数据仍在
#    卸载 = 把 /Applications 里的 app 拖到废纸篓；知识库目录（含 .knowledge/）不应受影响
```

## C2. 产物完整性（✅ 2026-10-14，对着 **Release 里发布的文件**逐项验）

不再是"CI 绿了就算"：把 Release 上的 9 个产物**全部下载回来**（经本机代理），逐项核验。

| 项 | 结果 |
| --- | --- |
| 6 个产物 × 发布的校验值 | ✅ **6/6 一致**（dmg / setup.exe / portable.zip / deb / rpm / AppImage） |
| macOS `.dmg` | ✅ 挂载正常；内含 `.app` 与 `Applications` 链接；`codesign --verify --deep --strict` **通过**（`Identifier=com.personalknowledge.os`）；版本 `1.3.0`；`spctl` 仍 `rejected`（预期：无 Developer ID） |
| Windows `setup.exe` | ✅ `file` 识别为 **PE32 NSIS 自解压安装器**；⚠️ **NSIS 的安装器外壳本身是 32 位 stub，这是正常的**（真正的应用在载荷里），别拿 `verify-pe.py` 直接判它"不是 x64" |
| Windows `portable.zip` → 里面的 `Personal Knowledge OS.exe` | ✅ 解包后用仓库自带 `scripts/cross/verify-pe.py` 校验：**x86-64 / GUI / 有资源节 / 校验通过** |
| Linux `.deb` | ✅ `ar t` 结构完整（`debian-binary` + `control.tar.gz` + `data.tar.gz`） |
| Linux `.rpm` | ✅ `file` 识别为 `RPM v3.0 bin` |
| Linux `.AppImage` | ✅ `file` 识别为 `ELF 64-bit LSB pie executable, x86-64`（84.4 MB） |
| **用户视角的校验流程** | ✅ 在下载目录里直接 `shasum -a 256 -c SHA256SUMS-all.txt` → **6/6 OK、退出码 0** |

> **这一步顺带抓到一个真缺陷**：v1.3.0 发布后按"用户会怎么验"实测，
> `shasum -c SHA256SUMS-macos.txt` 报 `1 line is improperly formatted` 并以非 0 退出——
> 根因是 macOS job 把一行 `du -k` 的体积输出也写进了哈希清单；而且三个平台的清单里写的都是
> **CI 产物路径**（`dmg/Personal Knowledge OS_x.dmg`、`deb/…`），用户在下载目录里按文件名找**对不上**
> （GitHub 上传资产时会把空格换成点）。
> 已修：workflow 改为**只写哈希行 + 按用户下载到的资产名写**；并把已发布的 4 份 `SHA256SUMS-*.txt`
> 一并替换（哈希值不变，只是格式与文件名；替换时四个文件的下载数均为 0）。

## D. 已知限制（不改代码也成立）

1. **未签名（Developer ID）/ 未公证**：即便是 ad-hoc 签名，`spctl --assess` 仍是 `rejected`；
   下载副本首次打开需要用户放行一次（见上）。要"双击即开"需要 Apple 开发者证书 + 公证；
2. **Windows 产物未签名**：SmartScreen 需放行；且由 macOS 交叉编译产出，**尚未在真实 Windows 上验收**；
3. **`.dmg` 的窗口装饰**：本机没授权「自动化 → 访达」，本地产出的 `.dmg` 用 `--skip-jenkins`
   跳过了装饰（没有背景图与图标排布）；CI（有图形会话）产出的是带装饰的版本；
4. **观察记录**：本机用 AppleScript 的 `quit` 事件没能让应用退出（应用不响应该事件），
   `⌘Q` 与 `SIGTERM` 正常——日常使用无影响，但写自动化脚本时要注意；
5. 其余沿用 [ACCEPTANCE_V1.2.1.md](./ACCEPTANCE_V1.2.1.md) §G（模型默认不接、检索源自己填等）。

## E. 相关文档

- [GUIDE.md](./GUIDE.md) / [GUIDE.en.md](./GUIDE.en.md)：完全指南（README 的推荐入口）
- [ACCEPTANCE_V1.2.1.md](./ACCEPTANCE_V1.2.1.md)：上一版完整验收清单与已知限制
- [PLAN_V1.3.0.md](./PLAN_V1.3.0.md)：本版计划、剩余任务与证据
- [BUILD_COMMANDS.md](./BUILD_COMMANDS.md)：打包命令、签名要求、常见坑
- [RELEASE_PROCESS.md](./RELEASE_PROCESS.md)：发版步骤与 CI 流水线
- [ARTIFACT_SIZES.md](./ARTIFACT_SIZES.md)：体积对照与口径
