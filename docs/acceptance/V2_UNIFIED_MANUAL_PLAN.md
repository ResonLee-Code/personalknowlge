# V2.0.x 统一人工真机验收执行包

> 用途：解锁 macOS、准备真实模型 / 远端 / 签名凭据 / 多平台设备后，按本文件一次性执行。
> 权威勾选项仍是 [P0 清单](../V2_P0_MANUAL_ACCEPTANCE.md) 和
> [P1 / P2 清单](../V2_P1_P2_MANUAL_ACCEPTANCE.md)；本文件负责顺序、前置条件与证据口径。
> 当前未完成：P0 36 项、P1 / P2 24 项。自动测试通过不能替这些项目勾选。
> 当前候选已更新为 `fa07c6c`，三平台产物、完整哈希和自动门禁见
> [V2 当前候选发布门禁](./V2_RELEASE_GATE_20261023.md)。人工真机必须使用本文件第 1 节
> 冻结的 commit 与产物，不能与历史 `49de15f` 的报告混用。
> 当前环境缺项与剩余项分组见
> [人工验收准备报告](./V2_MANUAL_READINESS_20261022.md)。

## 1. 候选版本冻结

开始人工验收前先冻结一个候选提交，后续所有设备必须使用同一提交和同一组签名产物。

需要记录：

| 字段 | 值 |
| --- | --- |
| Git commit | `fa07c6cb4670d2b7b69eeec435a32c9c7e355d04` |
| 版本号 / tag | `1.3.7`（V2 开发线，尚未创建 V2 tag） |
| 自动验收报告 | [V2 当前候选发布门禁](./V2_RELEASE_GATE_20261023.md) + [V2 P1/P2 自动验收](./V2_P1_P2_AUTOMATED_20261022.md) |
| macOS 产物与 SHA-256 | `.app.zip` `5854d674b2406ebb0171e45dde34b0ee312ed28b4225c645a46644428a7be2ea`；包内二进制 `ba4fa53546898b2b21a43af3bb6f757b4803ce68b289a8fe09eccf5dabb86e2c`；CDHash `d8c982da9ceaef57c1883ef117df73de3c2c1545`；ad-hoc，非发布签名 |
| Windows 产物与 SHA-256 | NSIS `1b9bd35f0f2290124e5b8b5824df8c879edd0f9e1c9cb90ac7d21c5ce645b715`；便携版 `b8f568e37baf542bfc7fbe856ec3b4d587b15570ac8b1ce5995db8ec690f48c3`；内嵌 PE `783eeaafe11cd0fee85f97cf9050746dc990b2ee961c1e258296cfb7f5ffc6e2` |
| Linux 产物与 SHA-256 | deb `9b35fc5fb085c4d3c31228281b901ae828dacf51cc2322aaf908fdfca5fcd005`；rpm `fa325703e6579d278fcd4b34eb086dbf06e331dfb3b6e2727d85e7d8b71d1d89`；AppImage `c3fa8ce8085aa61fb8b99fb4b56fd78c4137f0d6b38be5cdc95a83863f4adf1e`；内嵌 ELF Build ID `dec06f748f7fd5df7bb96adca9f2d920b75a7cd5` |
| `latest.json` SHA-256 | 尚未生成：等待正式 updater 签名密钥与 Release 资产 |

当前候选 `fa07c6c` 的三平台对象与哈希已记录在
[V2 当前候选发布门禁](./V2_RELEASE_GATE_20261023.md)。这些对象尚未经过 Developer ID、
公证、Windows 代码签名或真实设备安装，不能当成 Release 签名产物。历史 `49de15f`
报告仍可追溯功能行为，但正式人工验收不得使用旧包。

当前候选的 Rust / 前端 / E2E / Lint / production build / 发布清单与 updater 验签均已通过；
历史 `49de15f` 的 macOS 原生完整套件为 **321 步通过 / 0 失败 / 11 条待人工**；
当前 `fa07c6c` 只完成自动门禁、启动冒烟和三平台产物结构校验，原生完整套件必须在统一
人工验收阶段用当前包重跑。AI 总开关本地 / 模板链路也已通过真实 UI 冒烟，但真实模型质量
和正式签名仍等待外部条件。

人工测试开始前，再次核对本机文件哈希；任何不一致都必须停止并按新候选版本重新开始。

在候选提交上先完成：

```bash
cargo test --offline --manifest-path src-tauri/Cargo.toml
pnpm lint
pnpm test
pnpm build
pnpm test:e2e
pnpm test:acceptance
pnpm test:release-manifest
pnpm test:release-workflow
pnpm test:update-signature
cargo audit --file src-tauri/Cargo.lock --no-yanked
```

任一门禁失败时停止人工验收，先建立新候选提交。人工验收期间不更新依赖、不改代码。

## 2. 前置材料

### 质量数据集

- Ask：50 条带黄金答案和可引用段落的样本，20 条库中无答案负样本。
- Ask 多轮：至少 5 组连续追问，记录上一轮是否被误当成事实。
- Organize：200 条标签建议标注、关联建议接受记录、摘要事实核对表。
- Graph：200 对目标笔记 / 期望相关笔记标注，20 组近重复检查。
- Writing：大纲、续写、改写、缩短、扩写各至少 3 个真实课题。

### 外部环境

- 用户自有 WebDAV endpoint。
- 用户自有、非官方托管云要求的 S3 兼容服务。
- 两台可安装候选版本的设备，用于物理双设备同步。
- 已授权的在线模型；可选且已安装的本地模型。
- 可执行网络抓包的代理或系统工具。

### 平台与签名

- Apple Developer ID、公证账号 / app-specific password、Team ID。
- Windows Authenticode PFX 与密码。
- updater 公钥、私钥及密码，并已离线备份。
- Windows 10/11 与 Ubuntu 22.04 / 24.04 真机或独立虚拟机。
- 干净 macOS 用户目录或独立机器，用于 Developer ID / 公证包冷启动。

## 3. 执行顺序

### 阶段 A：本机质量与原生 UI

先在解锁的 macOS 上执行：

```bash
scripts/acceptance/run.sh --preflight --app "Personal Knowledge OS"
scripts/acceptance/run.sh --workspace /private/tmp/pkos-acceptance \
  --out docs/acceptance/<日期>-V2-manual-macos
```

同时完成：

- Ask 真实引用精确率、无依据断言率、负样本和多轮追问。
- Organize 标签 / 关联 / 摘要质量，以及真实模型的确认与撤销体验。
- Research 真实联网中断、恢复和已完成来源续跑。
- Graph 200 对近邻标注、真实嵌入、近重复和桥梁 / 孤儿 / 孤岛判断。
- Writing 五模式质量、逐段引用、diff、取消、确认和历史回滚。
- 隐私抓包：AI 关闭、未授权、逐次确认未勾选时均为 0 请求。
- 人工视觉：拖拽跟手、悬停动效、留白、长文滚动和虚拟列表跳动。

执行清单：

- [V2 P0 A / B / C / E](../V2_P0_MANUAL_ACCEPTANCE.md)
- [V2 P1 / P2 Graph / Writing](../V2_P1_P2_MANUAL_ACCEPTANCE.md)

### 阶段 B：真实加密同步

先在设备 1 配置 WebDAV，再对同一知识库配置 S3；两端使用同一同步口令。

必须保留：

- dry-run 计划截图。
- 两侧知识库路径、设备和版本。
- 服务端文件列表与随机对象名。
- 网络抓包文件。
- 冲突三版本截图、最终选择和恢复结果。
- 断网修改时间点、待重试队列、恢复时间。

执行清单：

- [真实 WebDAV](../V2_P1_P2_MANUAL_ACCEPTANCE.md#21-真实-webdav)
- [真实 S3 兼容存储](../V2_P1_P2_MANUAL_ACCEPTANCE.md#22-真实-s3-兼容存储)
- [同步安全与平台](../V2_P1_P2_MANUAL_ACCEPTANCE.md#23-安全与平台)

### 阶段 C：签名发布与安装升级

先运行一次带正式 Secrets 的 Release workflow，再下载实际 Release 资产进行安装。

macOS：

- `codesign --verify --deep --strict`。
- `spctl --assess`。
- 浏览器下载、首次打开、自动更新、篡改签名拒绝和失败回滚。

Windows：

- NSIS 全新安装、中文 / 空格路径、普通用户权限。
- 便携版、升级、卸载、数据保留。
- 正签更新与篡改更新拒绝。

Linux：

- Ubuntu 22.04 / 24.04 `.deb`。
- AppImage 赋权启动。
- 升级、卸载、更新签名失败拒绝。

执行清单：

- [Platform 真机与发布凭据](../V2_P0_MANUAL_ACCEPTANCE.md)
- [发布前人工抽验](../V2_P0_MANUAL_ACCEPTANCE.md)

### 阶段 D：10 万篇与最终交互

每个平台使用干净用户目录和独立知识库：

```bash
scripts/bench/prepare-scale-fixture.sh --notes 100000 \
  --data-dir /private/tmp/pkos-100k-data /private/tmp/pkos-100k
```

记录：

- 首次启动和稳定启动分别计时，不混写。
- 主进程首屏与 RSS。
- 检索 p95 与 recall@5。
- 背景索引是否完成、失败是否可见。
- 高频词搜索滚动、翻页、虚拟列表空白与跳动。
- 升级 / 卸载后附件、历史、收藏、标签、回收站、研究和 Organize 状态。

## 4. 证据目录

每轮使用独立目录，不覆盖历史报告：

```text
docs/acceptance/YYYYMMDD-V2-manual-<platform>/
├── report.md
├── report.json
├── screenshots/
├── logs/
├── captures/
├── artifacts.sha256
└── datasets/
```

每个勾选项必须能定位到：

1. 执行人和时间。
2. 设备、系统和候选版本。
3. 操作步骤或命令。
4. 指标、截图、日志、抓包或产物哈希。
5. 未通过时的复现步骤。

只有“看到了”“应该可以”而没有证据，不得勾选。

## 5. 自动项不要重复计入人工

下列内容已有自动证据，不再要求人工重新证明语义：

- 100 篇 Organize 暂停 / 恢复 / 取消 / 重试。
- 真实子进程强杀后的 Organize paused 恢复与续跑。
- Research 计划编辑、模板保存 / 复用、重跑差异。
- Ask 保存预览、确认写入和引用跳转。
- Graph 重复预览与摘要确认写入。
- Writing 来源漂移、目标哈希冲突、逐段 diff 和确认写入。
- Unix 凭据 `0600`、同步密文和本地离线可用性。

这些功能的“真实模型质量”“真实设备体验”和“真实远端互操作”仍需按对应人工项评估。

## 6. 退出标准

- P0 清单勾选项全部有证据，36 项不再剩余。
- P1 / P2 清单勾选项全部有证据，24 项不再剩余。
- 所有平台报告使用同一个候选 commit 和同一组产物。
- `latest.json` 中三平台 URL 均可下载并通过 SHA-256。
- 正签更新成功，篡改签名被拒绝且旧版仍可启动。
- Developer ID / 公证和 Windows 代码签名验证通过。
- 真实 WebDAV / S3 与物理双设备完成新增、修改、删除、冲突、合并与恢复。
- 10 万篇在发布形态下达到首屏 `< 2s`、检索 p95 `< 200ms`、RSS `< 1.5GB`。
- 主观 UI 项由执行人明确签字通过，不由脚本代替。

任一条件缺失时，整体状态保持“未完成”，不能以 CI 绿灯或本地桩替代。
