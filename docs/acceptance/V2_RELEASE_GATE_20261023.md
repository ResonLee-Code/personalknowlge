# V2 当前候选发布门禁

## 候选

- 应用代码提交：`fa07c6cb4670d2b7b69eeec435a32c9c7e355d04`
  （`fix(v2): 优化大规模搜索结果排序`）
- 版本号：`1.3.7`（V2 开发线，尚未创建 V2 tag）
- macOS release `.app` CDHash：
  `d8c982da9ceaef57c1883ef117df73de3c2c1545`
- macOS 签名：ad-hoc；Developer ID / 公证仍为人工验收项
- 数据库 schema：`17`（新增高频检索稳定排序索引）
- 产物目录：`dist-macos/candidate-fa07c6c`、`dist-windows/candidate-fa07c6c`、
  `dist-linux/candidate-fa07c6c`；各目录内的 `SHA256SUMS` 可逐文件复核。

## 当前产物

| 平台 / 对象 | 大小（字节） | SHA-256 |
| --- | ---: | --- |
| macOS `.app.zip` | 7,640,210 | `5854d674b2406ebb0171e45dde34b0ee312ed28b4225c645a46644428a7be2ea` |
| Windows NSIS | 4,615,605 | `1b9bd35f0f2290124e5b8b5824df8c879edd0f9e1c9cb90ac7d21c5ce645b715` |
| Windows 便携 ZIP | 6,144,167 | `b8f568e37baf542bfc7fbe856ec3b4d587b15570ac8b1ce5995db8ec690f48c3` |
| Linux `.deb` | 5,311,184 | `9b35fc5fb085c4d3c31228281b901ae828dacf51cc2322aaf908fdfca5fcd005` |
| Linux `.rpm` | 6,633,926 | `fa325703e6579d278fcd4b34eb086dbf06e331dfb3b6e2727d85e7d8b71d1d89` |
| Linux `.AppImage` | 84,683,256 | `c3fa8ce8085aa61fb8b99fb4b56fd78c4137f0d6b38be5cdc95a83863f4adf1e` |

| 平台可执行文件 | 大小（字节） | SHA-256 / Build ID |
| --- | ---: | --- |
| macOS arm64 | 13,570,288 | `ba4fa53546898b2b21a43af3bb6f757b4803ce68b289a8fe09eccf5dabb86e2c` |
| Windows x64 PE | 13,435,392 | `783eeaafe11cd0fee85f97cf9050746dc990b2ee961c1e258296cfb7f5ffc6e2` |
| Linux x64 ELF | 15,646,008 | Build ID `dec06f748f7fd5df7bb96adca9f2d920b75a7cd5` |

Windows NSIS 与便携 ZIP 内嵌的应用 PE 哈希一致。Linux deb / rpm 内嵌 ELF 与 release
构建逐字节一致；AppImage 仅额外设置可移植 RUNPATH `$ORIGIN/../lib`，Build ID 与 release
构建一致。

Linux 本次是手工复封装候选：Tauri 已完成当前 release 二进制编译，但容器内
linuxdeploy 在 WebKitGTK 依赖目录持续 I/O 无进展，因此 deb / rpm 从已验包骨架替换当前
ELF，AppImage 从上一组已验证的 Ubuntu 22.04 完整运行库中替换当前 ELF 后重新压包。
三种格式均重新做了 Build ID、依赖、归档和启动校验。它们用于当前人工候选，不替代正式
Release workflow 从零生成的资产。

## 自动化与脚本

| 检查 | 结果 |
| --- | --- |
| Rust library | 205 passed / 1 ignored |
| Rust 集成测试 | 全部通过；含四种搜索排序的全局分页一致性 |
| 数据库迁移 | schema 17；001–017 升级与 FTS 重建通过 |
| 前端 | 83 文件 / 418 用例 |
| Lint / production build | 通过 |
| Playwright E2E | 7 / 7 |
| 验收脚手架 Node 测试 | 7 / 7 |
| release manifest | 2 / 2 |
| release workflow | 7 / 7 |
| updater 签名 | 有效载荷通过、篡改拒绝、版本绑定通过 |
| `codesign --verify --deep --strict` | 通过 |
| macOS release 启动冒烟 | 进程稳定存活，数据目录与窗口状态日志正常 |
| Windows PE / NSIS / 便携包 | x64 GUI 结构、归档完整性、内嵌 PE 哈希通过 |
| Linux deb / rpm / AppImage | 元数据、依赖、内嵌 Build ID、Xvfb 启动冒烟通过 |
| 10 万全命中排序 | 默认相关度 p95 `160.2ms`；显式全库排序 p95 `326.3ms` |

`cargo audit` 本机未安装，未擅自修改工具链；CI 仍按仓库固定版本执行。

## 性能

命令：`pnpm bench`

- 1 万检索：平均 **7.32 ms**、p95 **16.94 ms**、显式排序 p95 **30.44 ms**、
  recall@1 **100%**、recall@5 **100%**；
- 3 万图谱：首屏 / 全量 **840.1 ms**、分类过滤 **343.9 ms**、
  2 跳聚焦 **617.1 ms**；
- 两项性能守护均通过。

10 万专项：默认检索 p95 **160.2ms**、recall@5 **100%**、重开 **1566ms**、
RSS **326.4MB**；全库命中的显式排序 p95 **326.3ms**，低于该类排序独立的 400ms
上限。默认检索仍执行 200ms 硬门禁。

## 边界

这些结果证明当前开发候选在本机自动门禁和性能守护上通过，不替代 Windows / Linux 真机、
正式签名、公证、真实外部同步或真实语料人工质量验收。Linux 的容器/Xvfb 启动冒烟也不能
替代 Ubuntu 22.04 / 24.04 物理机或独立虚拟机上的安装、升级、卸载和数据保留验收。
