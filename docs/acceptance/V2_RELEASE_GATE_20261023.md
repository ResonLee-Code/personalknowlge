# V2 当前候选发布门禁

## 候选

- 应用代码提交：`d385f9fcf9c698410883a666db370ac0bacc9b23`
  （`fix(v2): 保证搜索结果全局排序与稳定分页`）
- 版本号：`1.3.7`（V2 开发线，尚未创建 V2 tag）
- macOS release `.app` CDHash：
  `a150213cda67cb507972f37076cb19ac8d50fc44`
- macOS 签名：ad-hoc；Developer ID / 公证仍为人工验收项
- 产物目录：`dist-macos/candidate-d385f9f`、`dist-windows/candidate-d385f9f`、
  `dist-linux/candidate-d385f9f`；各目录内的 `SHA256SUMS` 可逐文件复核。

## 当前产物

| 平台 / 对象 | 大小（字节） | SHA-256 |
| --- | ---: | --- |
| macOS `.app.zip` | 7,631,741 | `95ef957be1de4aa3d7e4f2079d7872e39008401200ca33ebe4840ff1676daed9` |
| Windows NSIS | 4,607,686 | `8f723311d96e6385b4c5487eff7cd790e594a612988f347e0deb0fea51da650a` |
| Windows 便携 ZIP | 6,133,423 | `0006cbbd2561ebc4d0f53285227f26f8f56e9442ae73b736eeeac9ac53a56b82` |
| Linux `.deb` | 7,191,442 | `133b721d262eb7d475d8d02418d10adda34b8ec55887650dedacacc8c820655a` |
| Linux `.rpm` | 8,508,562 | `6447f942ecead367f0e49c1a5dfc9a3b04e1f53902f416433995d3b3e255a1b8` |
| Linux `.AppImage` | 86,542,840 | `0e7ccf3555ca930e8acc8415b1f7a174be226c9db716c18adcf3840be8a44fe2` |

| 平台可执行文件 | 大小（字节） | SHA-256 / Build ID |
| --- | ---: | --- |
| macOS arm64 | 13,537,408 | `10764db258426909a89ebd2ea1f62f1c2635b6bffdc7803007d19f1b3be804bc` |
| Windows x64 PE | 13,410,816 | `36344b330c3b555a884047329b2a12c387c9689ec0e55bf54cc40f0c1a573fa5` |
| Linux x64 ELF | 17,515,576 | Build ID `27c1b3fec49db494df42b6551edba7d35353619e` |

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

`cargo audit` 本机未安装，未擅自修改工具链；CI 仍按仓库固定版本执行。

## 性能

命令：`pnpm bench`

- 1 万检索：平均 **10.96 ms**、p95 **20.10 ms**、recall@1 **100%**、
  recall@5 **100%**；
- 3 万图谱：首屏 / 全量 **903.9 ms**、分类过滤 **347.2 ms**、
  2 跳聚焦 **670.7 ms**；
- 两项性能守护均通过。

## 边界

这些结果证明当前开发候选在本机自动门禁和性能守护上通过，不替代 Windows / Linux 真机、
正式签名、公证、真实外部同步或真实语料人工质量验收。Linux 的容器/Xvfb 启动冒烟也不能
替代 Ubuntu 22.04 / 24.04 物理机或独立虚拟机上的安装、升级、卸载和数据保留验收。
