# V1.3.2 验收记录

口径与 [ACCEPTANCE_V1.3.1.md](./ACCEPTANCE_V1.3.1.md) 一致：**没验的不写成通过**；
只能人眼判断的标「待人工」；不确定的写清"哪里还不确定"。

本文件记录 **V1.3.2**（2026-10-19）的验证实况。这一版是**纯修复版**：
非全屏窗口下把纵向空间还给正文，另外把发版流水线的校验文件统一成 LF。
没有新增功能，没有数据库 schema 变化。

## A. 发布前守护（本机）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 版本一致性（四处） | ✅ `1.3.2` | `package.json` / `src-tauri/Cargo.toml` / `Cargo.lock` / `tauri.conf.json`；`pnpm test` 的版本身份守卫 |
| 校准探针题库版本题 | ✅ 已同步 `1.3.2` | `decision_probe.rs` 的 `hidden_version_yes` 题面 |
| 前端 | ✅ 69 文件 / 374 用例 | `CI=true pnpm test` |
| 构建与静态检查 | ✅ | `pnpm lint`、`pnpm build` |
| Rust | ✅ 47 个测试目标全绿 | `cargo test --offline` |
| E2E 主流程 | ✅ 1 条通过 | `CI=true pnpm test:e2e`（含 1280×800 纵向空间断言） |

## B. 纵向布局：真机实测（本次修复的重点）

方法：同一台 Mac、同一份 `.app` 代码路径、同一个知识库、同一篇含 **9 条未解析链接**的笔记，
窗口位置 `120,80`、大小 `1280×800`（非全屏）；用 macOS 无障碍树读取「正文编辑区顶部」
与「关系区标题」的纵坐标，差值即**正文可见高度**。

对照包：修复前 V1.3.0 布局的旧 `.app`（本机 2026-10-18 23:37 构建）。

| 版本 / 状态 | 正文可见高度 | 底部信息区高度 | 截图 |
| --- | ---: | ---: | --- |
| V1.3.1（未解析常驻展开） | **204 px** | 322 px | `/tmp/pkos-vertical-1280-old.png` |
| V1.3.2 默认（未解析收起） | **371 px** | 167 px | `/tmp/pkos-vertical-1280.png` |
| V1.3.2 展开未解析（9 条） | **232 px** | 306 px（≈42% 封顶，内部滚动） | `/tmp/pkos-vertical-1280-new-expanded.png` |

- 默认状态正文 **+167 px（+82%）**，底部信息区 **−155 px（−48%）**；
- 展开未解析时正文仍保留 232 px，底部自己滚动，不再把正文压没；
- 更小的 **1024×700** 窗口下（`/tmp/pkos-vertical-1024.png`）：默认正文 **258 px**，
  工具栏自动收成图标（编辑 / 分屏 / 预览），底部 167 px。

> 口径说明：AX 读到的是逻辑坐标（pt），截图是 Retina 2×；表中数字均为逻辑像素。
> 「正文可见高度」= 关系区标题纵坐标 − 正文滚动视口顶部纵坐标，窗口底边为 `80+800=880`。

## C. 产物与发布

> 由「推 `v1.3.2` 标签」触发 CI 三平台构建并创建 Release；发布后逐项抽验见 §D。

| 平台 | 产物 |
| --- | --- |
| macOS（arm64） | `.dmg` + `.app`（ad-hoc 签名） |
| Windows（x64） | NSIS `setup.exe` + `portable.zip`（未签名） |
| Linux（x64） | `.deb` / `.rpm` / `.AppImage` |
| 校验 | 每平台一份 `SHA256SUMS-<platform>.txt` + 汇总 `SHA256SUMS-all.txt`（全部 LF） |

## D. 发布后抽验

CI run **39152786198**：preflight / macOS / Windows / Linux / Release 五个作业 **全部 success**
（总耗时 17 分 23 秒）。发布后把 10 个资产下载到空目录，按用户路径抽验：

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| Release 资产 | ✅ 10 个 | 6 个安装包 + 4 份校验文件 |
| **汇总校验** | ✅ **6/6 OK、退出码 0** | `shasum -a 256 -c SHA256SUMS-all.txt` |
| 校验文件行尾 | ✅ 全部 LF | `grep $'\r'` 四份均为 0 命中（本轮 workflow 修复已生效） |
| macOS `.dmg` | ✅ 可挂载、版本正确、签名有效 | `CFBundleShortVersionString = 1.3.2`、`CFBundleVersion = 1.3.2`；`codesign --verify --deep --strict` → *valid on disk* / *satisfies its Designated Requirement*；`spctl` → *rejected*（ad-hoc，见 §E） |
| Windows 便携版 exe | ✅ 结构校验通过 | PE32+ / x86-64 / GUI / 12.3 MB |
| Windows `setup.exe` | ✅ NSIS 安装器 | `PE32 ... Nullsoft Installer self-extracting archive`；外壳 i386 是 NSIS stub 的正常形态 |
| Linux `.deb` | ✅ 结构完整 | control：`Version: 1.3.2`、`Architecture: amd64`；payload 含 `usr/bin/personal-knowledge-os` 与 `.desktop` |
| Linux `.rpm` | ✅ 结构完整（未安装级验证） | `bsdtar -tf` 列出二进制 / `.desktop` / 图标 |
| Linux `.AppImage` | ✅ ELF x86-64 | 内嵌 squashfs（`hsqs` 魔数）；**未在 Linux 桌面实际运行** |

发布资产精确字节：

| 资产 | 字节 |
| --- | ---: |
| `Personal.Knowledge.OS_1.3.2_aarch64.dmg` | 6,972,175 |
| `Personal.Knowledge.OS_1.3.2_x64-setup.exe` | 5,820,475 |
| `Personal.Knowledge.OS_1.3.2_x64-portable.zip` | 7,019,936 |
| `Personal.Knowledge.OS_1.3.2_amd64.deb` | 7,741,508 |
| `Personal.Knowledge.OS-1.3.2-1.x86_64.rpm` | 7,742,677 |
| `Personal.Knowledge.OS_1.3.2_amd64.AppImage` | 85,727,736 |

## E. 已知限制（与 V1.3.1 相同，未因本版改变）

1. **未签名（Developer ID）/ 未公证**：`spctl --assess` 仍是 `rejected`；下载副本首次打开需放行一次；
2. **Windows / Linux 产物未签名**，且**尚未在真实设备上验收**（由 CI 构建；只做结构校验）；
3. **图（Mermaid）在导出里的表现**：HTML / Word 导出保留源码，不执行 mermaid；需要图时用 PDF 或截图；
4. 其余沿用 [ACCEPTANCE_V1.3.1.md](./ACCEPTANCE_V1.3.1.md) §D。

## F. 相关文档

- [CHANGELOG.md](./CHANGELOG.md)：V1.3.2 的用户可见变化
- [GUIDE.md](./GUIDE.md) / [GUIDE.en.md](./GUIDE.en.md) / [MANUAL.md](./MANUAL.md)：完全指南与逐控件手册
- [RELEASE_PROCESS.md](./RELEASE_PROCESS.md)：发版步骤 + `develop` / `main` 分支约定
