# V2.0.x 统一人工验收准备报告

## 结论

当前候选的代码、自动测试、三平台开发产物和 Linux 隔离环境证据已就绪，但还不具备开始完整
人工真机验收的外部条件。本文只记录准备状态，不是验收通过报告，不勾选任何权威人工清单项。

- 当前候选代码：`fa07c6cb4670d2b7b69eeec435a32c9c7e355d04`
- 当前候选产物：[V2 当前候选发布门禁](./V2_RELEASE_GATE_20261023.md)
- 权威剩余项：[V2 P0 人工清单](../V2_P0_MANUAL_ACCEPTANCE.md)
  + [V2 P1 / P2 人工清单](../V2_P1_P2_MANUAL_ACCEPTANCE.md)
- 当前剩余：P0 **36 项**、P1 / P2 **24 项**，合计 **60 项**

> 下方“2026-10-22 环境盘点”保留为历史快照；当前状态以紧接着的
> “2026-10-23 复盘点”为准。

## 2026-10-23 当前环境复盘点

| 前置条件 | 当前状态 | 对统一人工验收的影响 |
| --- | --- | --- |
| 三平台开发候选 | `fa07c6c` 的 macOS ad-hoc、Windows NSIS/便携版、Linux deb/rpm/AppImage 已重建并校验 | 可以开始使用当前包的人工复跑；仍需按平台逐项留存证据 |
| macOS 签名身份 | 钥匙串 `0 valid identities found` | Developer ID、公证、浏览器首次打开和签名冷启动未解锁 |
| Apple 公证环境变量 | `APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID`、`APPLE_API_KEY*` 均未设置 | 无法提交公证 |
| Windows 签名环境变量 | `WINDOWS_CERTIFICATE`、密码均未设置 | 正签安装器、SmartScreen 和签名更新拒绝未解锁 |
| updater 密钥 | `TAURI_SIGNING_PRIVATE_KEY`、密码、`PKOS_UPDATER_PUBKEY` 与环境变量 / `~/.tauri` 均未发现 | 正式 `.sig`、可信 `latest.json` 和更新回滚未解锁 |
| GitHub 发布 | `gh` CLI 未安装；远端为 `ResonLee-Code/personalknowlge` | 可由网页或安装 CLI 后执行正式 Release |
| 在线模型 | 应用凭据文件中 DeepSeek Key 已配置；Perplexity Decisions Key 未配置 | DeepSeek 质量评测可复跑；System One 决策档仍不可启用 |
| 本地模型 | Ollama `qwen2.5:7b` 已安装并可用 | 可完成本地模型质量与离线链路复测 |
| WebDAV / S3 | `rclone` 当前没有已列出 remote；应用配置中未发现可用同步 profile | 真实 WebDAV、S3 与物理双设备仍未解锁 |
| Windows / Linux 真机 | 当前只有交叉构建、隔离容器 / VM 与 Xvfb 冒烟 | 安装、升级、卸载、ACL 和物理桌面体验保留人工 |

未设置和凭据存在性只按变量名 / 文件元数据检查，未读取、打印或写入任何密钥值。

## 2026-10-22 环境盘点

| 前置条件 | 当前状态 | 影响 |
| --- | --- | --- |
| macOS 交互会话 | 已解锁 | 当前 `49de15f` 的 `.app` 已完成 321 步原生套件；签名 / 公证仍需凭据 |
| Apple 代码签名 | 钥匙串 0 个有效 codesigning identity | Developer ID、公证和签名冷启动全部未解锁 |
| Apple 公证 | Apple ID / app-specific password / Team ID / API Key 均未设置 | 无法提交公证 |
| Windows 签名 | 证书 base64、密码、thumbprint 均未设置 | 无法生成正签 NSIS 或验证 SmartScreen |
| updater 签名 | `TAURI_SIGNING_PRIVATE_KEY`、`PKOS_UPDATER_PUBKEY` 均未设置 | 无法生成正式 `.sig` 和可信 `latest.json` |
| GitHub 发布操作 | 未安装 `gh` CLI；远端为 `ResonLee-Code/personalknowlge` | 可通过网页或安装 CLI 后触发并下载 Release |
| Windows 物理机 | 未提供 | NSIS、便携版、ACL、升级和 SmartScreen 保留人工 |
| Linux 物理机 | 未提供；已有 Ubuntu 22.04 x86_64 隔离 VM | 仅 VM 证据，不能替代 22.04 / 24.04 物理桌面 |
| 在线模型 | DeepSeek 已选定，API Key 未提供 | 本地 Ollama 已可用；付费在线模型质量仍保留人工 |
| WebDAV / S3 | 本机 rclone 服务已配置并通过应用双设备链路 | HTTPS 可达的真实远端和物理双设备仍保留人工 |
| 第二台同步设备 | 未提供 | 物理双设备、断网恢复与冲突选择保留人工 |

未设置只按变量名检查，未读取或记录任何凭据值。

AI 总开关已额外通过真实 UI 打开并完成本地 / 模板链路冒烟，本地 Ollama 与付费 DeepSeek
在线模型现已可用。真实 50 题引用评测已连续两轮达标，但逐题语义断言审查仍保留人工。模板证据见
[AI 总开关开启与本地性能冒烟](./V2_AI_ON_SMOKE_20261022.md)。

2026-10-23 的阶段性准备曾覆盖 updater 一次性测试密钥、本地 `qwen2.5:7b`、本机
WebDAV/S3 桩及应用跨设备加密同步预检；当前复盘点确认正式 updater 密钥与真实同步 remote
尚未保留在本机。当前准备了什么、性能如何以及还缺什么，见
[本地模型、updater 与同步存储准备记录](./V2_LOCAL_AI_STORAGE_20261023.md)。

## 剩余项分组

### P0：36 项

| 分组 | 数量 | 主要前置条件 |
| --- | ---: | --- |
| Ask 断言级人工复核 | 3 | 真实模型输出已就绪，待逐题语义签字与无依据断言统计 |
| Organize 标签 / 关联 / 摘要质量 | 3 | 真实模型与人工标注 |
| 发布凭据与 workflow | 5 | Apple、Windows、updater 凭据及 Release 权限 |
| Windows 真机 | 8 | Windows 10/11、PFX、上一版安装包 |
| Linux 真机 | 6 | Ubuntu 22.04 / 24.04 物理桌面 |
| macOS 签名与更新 | 6 | Developer ID、公证、旧版 / 新版签名包 |
| 隐私外发抓包 | 0 | 真实模型未授权与逐次确认场景均已完成 |
| 发布前抽验 | 5 | 正式签名 Release、旧版数据、公证冷启动 |

### P1 / P2：24 项

| 分组 | 数量 | 主要前置条件 |
| --- | ---: | --- |
| Graph 2.0 | 6 | 200 对标注、真实嵌入端点、人工判断 |
| 真实 WebDAV | 6 | 用户 endpoint、两台真实设备 |
| 真实 S3 兼容存储 | 3 | 用户 bucket / region / endpoint / AK / SK |
| 同步安全与 Windows ACL | 2 | 抓包工具、Windows 真机 |
| Scale 2.0 | 4 | 公证后的 macOS 干净机、Windows / Linux 物理机 |
| Writing 2.0 | 3 | 人工质量、逐段语义支持、macOS / Windows 真机 |

## 解锁后的执行顺序

1. 配置 updater 密钥、Apple Developer ID 与公证凭据，生成正式 macOS 候选并完成签名 / 公证。
2. 配置 Windows PFX，触发三平台 Release；核对 `.sig`、`latest.json`、实际下载 URL 和哈希。
3. 在 Windows 真机完成安装、路径、升级、卸载、便携版、SmartScreen 和凭据 ACL。
4. 在 Ubuntu 22.04 / 24.04 物理机完成 deb、AppImage、升级、卸载和诊断导出。
5. 用真实模型完成 Ask / Organize / Writing 质量评测，并同步抓包核对零请求边界。
6. 用用户自有 WebDAV / S3 和两台设备完成加密同步、断网恢复、冲突及历史恢复。
7. 完成 Graph 200 对近邻标注、20 组近重复和真实嵌入质量判断。
8. 在正式签名候选上复跑 10 万篇冷启动、检索、Graph 与高频词滚动检查。
9. 按统一报告模板归档全部截图、日志、抓包、数据集和签名 / 下载证据。

## 不该被误读的现有证据

- Linux QEMU 中 deb 安装启动、AppImage 软件渲染启动和卸载数据保留已通过，但不等同于物理桌面、
  FUSE 双击、Ubuntu 24.04 或签名更新验证。
- 当前 `49de15f` 已完成 321 步原生套件；其中 11 条主观视觉、原生排版和真实网络判断仍需人工。
- 本地 WebDAV / S3 桩、updater 测试密钥和模板模型分别证明了协议、验签与降级逻辑，
  不能冒充真实服务、正式签名或真实模型质量。
