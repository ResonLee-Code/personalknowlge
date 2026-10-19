# V1.3.5 验收记录

本版按用户要求**取消自绘标题栏**：任何状态都使用 macOS 系统原生红黄绿。
无功能 / 数据结构变化。

## A. 发布前守护（本机）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 版本一致性（四处） | ✅ `1.3.5` | `package.json` / `Cargo.toml` / `Cargo.lock` / `tauri.conf.json` |
| 校准探针版本题 | ✅ `1.3.5` | `hidden_version_yes` |
| 前端 | ✅ 70 文件 / 377 用例 | 删除自绘标题栏的 1 个组件 + 3 条用例 |
| 构建与静态检查 | ✅ | `pnpm lint`、`pnpm build` |
| E2E 主流程 | ✅ 1 条通过 | `CI=true pnpm test:e2e` |
| Rust | ✅ 47 个测试目标全绿 | `cargo test --offline`（含 macOS 窗口配置编译） |

## B. 原生窗口行为（真机）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 自绘组件 | ✅ 已删除 | 源码 / 测试中不再有 `FullscreenWindowControls` |
| 绿色按钮语义 | ✅ `zoom button` | 无障碍树：`close button, zoom button, minimize button` |
| 绿色按钮图标 | ⚠️ 已设为系统模板，待真实鼠标复核 | `NSImageNameExitFullScreenTemplate` 写入 image / alternateImage；自动化鼠标不触发原生红黄绿的 hover 绘制（Finder 同样），截图看不到图标 |
| 点击绿色按钮 | ✅ 系统缩放 | 窗口 `1512×859 @ (0,33)`；菜单栏、标题栏、红黄绿完整可见 |
| 再次触发菜单 | ✅ 缩放切换 | `Zoom`（原 `Toggle Full Screen`）把窗口恢复为 `1048×859 @ (256,87)` |
| 自绘 / 系统按钮叠加 | ✅ 不可能再出现 | 自绘组件已删除 |

截图：`/tmp/pkos-zoom.png`（系统缩放状态，原生标题栏 + 菜单栏）。

实现：`lib.rs` 在 macOS setup 阶段把主窗口的 `NSWindowCollectionBehavior`
设为 `FullScreenNone` 并移除 `FullScreenPrimary`；菜单项 `Toggle Full Screen` 改名为 `Zoom`。

## C. 产物与发布

推 `v1.3.5` 触发 CI；发布后抽验见 §D。

## D. 发布后抽验

Release **已生成**（`published 2026-10-19T16:10:38Z`，10 个资产）。
注意：GitHub Actions 页面/接口一度仍显示 `queued`（GitHub 状态为 Partially Degraded Service），
但 Release 与资产实际已上传；以 Release API 与下载校验为准。

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 校验文件行尾 | ✅ 全部 LF | 4 份 `SHA256SUMS-*.txt` 均无 `\r` |
| macOS `.dmg` | ✅ 哈希一致、版本 `1.3.5`、`codesign` 有效 | 挂载后 `CFBundleShortVersionString/CFBundleVersion = 1.3.5`；`spctl` 仍为 rejected（ad-hoc） |
| Windows 便携版 exe | ✅ PE32+ / x86-64 / GUI / 12.3 MB | `scripts/cross/verify-pe.py` |
| Windows `setup.exe` | ✅ NSIS 安装器 | PE32 i386 stub 属正常 |
| Linux `.deb` | ✅ `Version: 1.3.5`、`Architecture: amd64` | control + payload 二进制 / `.desktop` |
| Linux `.rpm` | ✅ 结构完整 | `bsdtar -tf` 列出二进制 / `.desktop` / 图标 |
| Linux `.AppImage` | ✅ ELF x86-64 + 内嵌 squashfs | `file`: ELF 64-bit LSB pie / x86-64；`hsqs` 魔数存在；85,723,640 B |
| **汇总校验** | ✅ **6/6 OK、退出码 0** | 空目录下载全部 10 个资产后 `shasum -a 256 -c SHA256SUMS-all.txt` |

## E. 已知限制

- 不再有 macOS 原生全屏 Space；“最大化”使用系统缩放（窗口铺满可用区域，保留标题栏 / 菜单栏）；
- macOS 未做 Developer ID 签名与公证；Windows / Linux 未在真实设备运行。
