# V1.3.1 验收记录

口径与 [ACCEPTANCE_V1.3.0.md](./ACCEPTANCE_V1.3.0.md) 一致：**没验的不写成通过**；
只能人眼判断的标「待人工」；不确定的写清"哪里还不确定"。

本文件记录 **V1.3.1**（2026-10-19）出包前后的验证实况。
本版是 **V1.3.0 之后 main 上 20 个提交的收口**：编辑器补上图形 / 插入 / 格式 / 折叠，
界面去重复、收拢工具栏、设置页按 Codex 重排，并修掉「插入图片点了没反应」。

## A. 发布前守护（本机）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 版本一致性（四处） | ✅ `1.3.1` | `package.json` / `src-tauri/Cargo.toml` / `Cargo.lock` / `tauri.conf.json`；`pnpm test` 的版本身份守卫 |
| 前端 | ✅ 69 文件 / 374 用例 | `CI=true pnpm test`；`pnpm lint`、`pnpm build` 通过 |
| Rust | ✅ 47 个测试目标全绿 | `cargo test --offline` |
| E2E 主流程 | ✅ 1 条通过 | `CI=true pnpm test:e2e`（新增：插入菜单、格式条实测宽度、收藏橙色、设置页导航与搜索、大纲折叠、mermaid 渲染、插入图片必须弹选择框） |
| 性能基准 | ✅ 全部过线 | `pnpm bench`（1 万检索 p95 + 3 万图谱全量，阈值见脚本） |
| 真机验收（E1 脚本） | ✅ **13 条用例 / 自动步骤 215 步通过 / 0 步失败** | `docs/acceptance/20261018-235856/report.md`（含新增第 13 条：插入图片整链路——原生选文件 → `attachments/` 落盘 → 正文写入链接） |

## B. 产物与发布

> 由「推 `v1.3.1` 标签」触发 CI 三平台构建并创建 Release；**发布后逐项抽验**见 §C。

| 平台 | 产物 |
| --- | --- |
| macOS（arm64） | `.dmg` + `.app`（ad-hoc 签名） |
| Windows（x64） | NSIS `setup.exe` + `portable.zip`（未签名） |
| Linux（x64） | `.deb` / `.rpm` / `.AppImage` |
| 校验 | 每平台一份 `SHA256SUMS-<platform>.txt` + 汇总 `SHA256SUMS-all.txt` |

## C. 发布后抽验

按「用户下载之后会怎么验」实测（2026-10-19，空目录 + Release 下载链接）：

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| Release 资产数量 | ✅ 10 个 | 6 个安装包 + 4 份校验文件；逐个下载 HTTP 200 |
| **用户视角的汇总校验** | ✅ **6/6 OK、退出码 0** | `shasum -a 256 -c SHA256SUMS-all.txt`（下载目录里直接执行） |
| 分平台校验 | ✅ macOS 1/1、Windows 2/2、Linux 3/3 | 三份 `SHA256SUMS-<platform>.txt` 各自通过 |
| macOS `.dmg` | ✅ 可挂载、签名有效 | `CFBundleShortVersionString = 1.3.1`、`CFBundleVersion = 1.3.1`；`codesign --verify --deep --strict` → *valid on disk* / *satisfies its Designated Requirement*；`spctl` → *rejected*（ad-hoc，见 §D） |
| Windows 便携版 exe | ✅ 结构校验通过 | `scripts/cross/verify-pe.py`：PE32+ / x86-64 / GUI / 12.3 MB |
| Windows `setup.exe` | ✅ 是 NSIS 安装器 | `file`：`PE32 ... Nullsoft Installer self-extracting archive`；**外壳是 32 位 stub 属正常**，它解包出的应用 exe 才是 x64（已用同一脚本验过） |
| Linux `.deb` | ✅ 结构完整 | `ar t` = `debian-binary` / `control.tar.gz` / `data.tar.gz`；control 里 `Version: 1.3.1`、`Architecture: amd64`；payload 含 `usr/bin/personal-knowledge-os`、`.desktop` 与三档图标 |
| Linux `.rpm` | ✅ 结构完整（未做安装级验证） | `bsdtar -tf` 能列出 `usr/bin/personal-knowledge-os`、`.desktop`、图标；包内版本文本为 `1.3.1`。本机没装 `rpm`，**没有在 RPM 系统上实际安装** |
| Linux `.AppImage` | ✅ ELF x86-64 | `file`：ELF 64-bit LSB pie / x86-64；内含 squashfs（`hsqs` 魔数）。**没有在 Linux 桌面上实际运行** |
| Windows 便携版 zip | ✅ 单文件 | 解包得到 `Personal Knowledge OS.exe`（12,883,456 B） |

### 抽验中发现并修复的发布物缺陷：校验文件行尾

`SHA256SUMS-windows.txt` 与 `SHA256SUMS-all.txt` 是 **CRLF 行尾**。哈希值本身正确，
但在 macOS / Linux 上 `shasum -c` / `sha256sum -c` 会把文件名末尾的 `\r` 当成文件名的一部分，
报 `No such file or directory` 并以非 0 退出——等于这份校验文件自己过不了自己。

处理：

1. workflow 的 Windows 生成步骤改为**显式写 LF、无 BOM**
   （`[System.IO.File]::WriteAllText`，不再用默认 CRLF 的 `Out-File`）；
2. Release 汇总步骤再 `tr -d '\r'` **兜底**一次；
3. 重新上传已发布 Release 上的两份文件：`SHA256SUMS-windows.txt` 221 → **219 B**、
   `SHA256SUMS-all.txt` 647 → **645 B**；文件名与哈希值不变，只换行尾；
4. 从 Release **重新下载**后再按用户方式校验 → **6/6 OK、退出码 0**。

> 注意：刚替换完时浏览器下载链接可能仍命中旧缓存（实测第一次下载还是 221 B 的 CRLF 版本）；
> 加 `Cache-Control: no-cache` 或换个查询串重新下载即为 219 B 的 LF 版本。
> 二进制产物没有变化，重传前后 SHA256 一致。

## D. 已知限制（与 V1.3.0 相同，未因本版改变）

1. **未签名（Developer ID）/ 未公证**：`spctl --assess` 仍是 `rejected`；下载副本首次打开需放行一次；
2. **Windows / Linux 产物未签名**，且**尚未在真实设备上验收**（由 macOS 交叉编译；已做结构校验）；
3. **图（Mermaid）在导出里的表现**：预览与 PDF（打印视图）是渲染后的图；
   HTML / Word 导出保留的是**源码**（导出器不是浏览器，不执行 mermaid）——需要图时用 PDF 或截图；
4. 其余沿用 [ACCEPTANCE_V1.3.0.md](./ACCEPTANCE_V1.3.0.md) §D。

## E. 相关文档

- [CHANGELOG.md](./CHANGELOG.md)：V1.3.1 的用户可见变化
- [GUIDE.md](./GUIDE.md) / [GUIDE.en.md](./GUIDE.en.md) / [MANUAL.md](./MANUAL.md)：完全指南与逐控件手册（均按 V1.3.1 更新）
- [RELEASE_PROCESS.md](./RELEASE_PROCESS.md)：发版步骤 + `develop` / `main` 分支约定
