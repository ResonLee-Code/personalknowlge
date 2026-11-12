# Personal Knowledge OS V2.0.0 发布候选验收

日期：2026-11-13

状态：本机 macOS 候选已验证；远端通过 `v2.0.0-unsigned.1` 发布未签名预发行版

## 版本身份

| 项目 | 值 |
| --- | --- |
| 应用版本 | `2.0.0` |
| 数据库 schema | `18` |
| 发布提交 | `v2.0.0` 标签指向的提交 |
| 升级来源 | 已有 V1.x 工作区原地迁移到 schema 18 |

## 本机产物

| 项目 | 值 |
| --- | --- |
| macOS 架构 | arm64 |
| `.dmg` | `Personal Knowledge OS_2.0.0_aarch64.dmg` |
| `.dmg` 大小 | 15,692,285 B |
| `.dmg` SHA-256 | `daee0515039572007c613947993ec225202fb3167fd36c03ce34dcebb7a7ed6e` |
| 应用二进制 SHA-256 | `915327c17abcc117d58a903ed4e8843227a9a15717411c40528d488a388d47a9` |
| 签名 | ad-hoc；无 Developer ID、无公证 |
| CDHash | `59438338e9b083c16c7446b9eb8c254773d2f680` |

DMG 已挂载验证：包含 `Personal Knowledge OS.app` 与 `Applications` 链接，应用版本
`CFBundleShortVersionString=2.0.0`，`codesign --verify --deep --strict` 通过；
随后从 release `.app` 实际启动，进程稳定运行。

## 自动验收

| 门禁 | 结果 |
| --- | --- |
| `pnpm test` | 90 个文件、456 项通过 |
| `pnpm build` | TypeScript 与生产构建通过 |
| `pnpm lint` | 通过 |
| `pnpm test:e2e` | 10 项通过 |
| `cargo test --offline` | 全部通过，仅保留既定 ignored 真机 / 性能用例 |
| 内容发现集成测试 | 8 项通过 |
| `cargo clippy --offline --all-targets` | 0 错误；仓库既有 warning |
| `cargo audit` | 3 个漏洞已通过依赖升级修复；8 个 allow-warning 保留 |
| `pnpm test:acceptance` | 7 项通过 |
| `pnpm test:release-manifest` | 2 项通过 |
| `pnpm test:release-workflow` | 7 项通过 |
| `pnpm test:update-signature` | 有效载荷通过、篡改拒绝、绑定 `2.0.0` |
| `pnpm bench` | 检索与图谱全部过线 |

## 性能

- 1 万篇检索：平均 8.80 ms，p95 20.12 ms；
- 显式全量排序：p95 37.72 ms；
- recall@1 / @5：100% / 100%；
- 3 万篇图谱首屏 / 全量：949.5 ms，扫描 14.90 s；
- 数据库：1 万篇 36.68 MB；3 万篇 93.42 MB。

## 发布边界

本机候选是 ad-hoc 签名。仓库正式 `v2.0.0` tag 仍要求 updater、Apple Developer ID / 公证与
Windows Authenticode Secrets；当前远端 Actions 未配置这些 Secret。

本次按用户确认走 `v2.0.0-unsigned.1` 未签名预发行通道：

- GitHub Release 标记为 `prerelease=true`；
- macOS 使用 ad-hoc 签名、不公证；
- Windows 使用未签名 NSIS / 便携包；
- 不生成 `latest.json`，不进入正式自动更新；
- 正式稳定版本仍为 V1.3.7，后续补齐签名后再发布正式 `v2.0.0`。
