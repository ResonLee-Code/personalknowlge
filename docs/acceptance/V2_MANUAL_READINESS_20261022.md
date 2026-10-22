# V2.0.x 统一人工验收准备报告

## 结论

当前候选的代码、自动测试、三平台开发产物和 Linux 隔离环境证据已就绪，但还不具备开始完整
人工真机验收的外部条件。本文只记录准备状态，不是验收通过报告，不勾选任何权威人工清单项。

- 候选代码：`49de15fbb17cbc468157ef2c477e901c4cffef9a`
- 候选产物：[V2_PLATFORM_ARTIFACTS_20261022.md](./V2_PLATFORM_ARTIFACTS_20261022.md)
- 权威剩余项：[V2 P0 人工清单](../V2_P0_MANUAL_ACCEPTANCE.md)
  + [V2 P1 / P2 人工清单](../V2_P1_P2_MANUAL_ACCEPTANCE.md)
- 当前剩余：P0 **41 项**、P1 / P2 **26 项**，合计 **67 项**

## 2026-10-22 环境盘点

| 前置条件 | 当前状态 | 影响 |
| --- | --- | --- |
| macOS 交互会话 | 锁屏（`CGSSessionScreenIsLocked=true`） | 当前 `.app` 进程可启动，但不能创建或驱动可见窗口；原生套件暂缓 |
| Apple 代码签名 | 钥匙串 0 个有效 codesigning identity | Developer ID、公证和签名冷启动全部未解锁 |
| Apple 公证 | Apple ID / app-specific password / Team ID / API Key 均未设置 | 无法提交公证 |
| Windows 签名 | 证书 base64、密码、thumbprint 均未设置 | 无法生成正签 NSIS 或验证 SmartScreen |
| updater 签名 | `TAURI_SIGNING_PRIVATE_KEY`、`PKOS_UPDATER_PUBKEY` 均未设置 | 无法生成正式 `.sig` 和可信 `latest.json` |
| GitHub 发布操作 | 未安装 `gh` CLI；远端为 `ResonLee-Code/personalknowlge` | 可通过网页或安装 CLI 后触发并下载 Release |
| Windows 物理机 | 未提供 | NSIS、便携版、ACL、升级和 SmartScreen 保留人工 |
| Linux 物理机 | 未提供；已有 Ubuntu 22.04 x86_64 隔离 VM | 仅 VM 证据，不能替代 22.04 / 24.04 物理桌面 |
| 在线模型 | 未提供真实授权 | Ask / Organize / Writing 的真实模型质量评测保留人工 |
| WebDAV / S3 | 未提供用户自有服务 | 真实同步互操作与远端密文检查保留人工 |
| 第二台同步设备 | 未提供 | 物理双设备、断网恢复与冲突选择保留人工 |

未设置只按变量名检查，未读取或记录任何凭据值。

## 剩余项分组

### P0：41 项

| 分组 | 数量 | 主要前置条件 |
| --- | ---: | --- |
| Ask 断言级人工复核 | 4 | 真实模型（或对模板结果逐条签字） |
| Organize 标签 / 关联 / 摘要质量 | 3 | 真实模型与人工标注 |
| 发布凭据与 workflow | 5 | Apple、Windows、updater 凭据及 Release 权限 |
| Windows 真机 | 8 | Windows 10/11、PFX、上一版安装包 |
| Linux 真机 | 6 | Ubuntu 22.04 / 24.04 物理桌面 |
| macOS 签名与更新 | 6 | Developer ID、公证、旧版 / 新版签名包 |
| 隐私外发抓包 | 4 | 真实模型配置、系统代理或抓包工具 |
| 发布前抽验 | 5 | 正式签名 Release、旧版数据、公证冷启动 |

### P1 / P2：26 项

| 分组 | 数量 | 主要前置条件 |
| --- | ---: | --- |
| Graph 2.0 | 6 | 200 对标注、真实嵌入端点、人工判断 |
| 真实 WebDAV | 6 | 用户 endpoint、两台真实设备 |
| 真实 S3 兼容存储 | 3 | 用户 bucket / region / endpoint / AK / SK |
| 同步安全与 Windows ACL | 2 | 抓包工具、Windows 真机 |
| Scale 2.0 | 4 | 公证后的 macOS 干净机、Windows / Linux 物理机 |
| Writing 2.0 | 5 | 真实在线 / 本地模型、macOS / Windows 真机 |

## 解锁后的执行顺序

1. 解锁 macOS，启动当前 `.app`，重跑完整原生套件，确认锁定候选上的 UI 回归。
2. 配置 updater 密钥、Apple Developer ID 与公证凭据，生成正式 macOS 候选并完成签名 / 公证。
3. 配置 Windows PFX，触发三平台 Release；核对 `.sig`、`latest.json`、实际下载 URL 和哈希。
4. 在 Windows 真机完成安装、路径、升级、卸载、便携版、SmartScreen 和凭据 ACL。
5. 在 Ubuntu 22.04 / 24.04 物理机完成 deb、AppImage、升级、卸载和诊断导出。
6. 用真实模型完成 Ask / Organize / Writing 质量评测，并同步抓包核对零请求边界。
7. 用用户自有 WebDAV / S3 和两台设备完成加密同步、断网恢复、冲突及历史恢复。
8. 完成 Graph 200 对近邻标注、20 组近重复和真实嵌入质量判断。
9. 在正式签名候选上复跑 10 万篇冷启动、检索、Graph 与高频词滚动检查。
10. 按统一报告模板归档全部截图、日志、抓包、数据集和签名 / 下载证据。

## 不该被误读的现有证据

- Linux QEMU 中 deb 安装启动、AppImage 软件渲染启动和卸载数据保留已通过，但不等同于物理桌面、
  FUSE 双击、Ubuntu 24.04 或签名更新验证。
- macOS 旧基线 `7539e4f` 的 321 步原生结果仍是有效历史证据，但不能冒充当前 `49de15f`
  已完成原生门禁。
- 本地 WebDAV / S3 桩、updater 测试密钥和模板模型分别证明了协议、验签与降级逻辑，
  不能冒充真实服务、正式签名或真实模型质量。
