# V2.0.x P0 执行台账

> 目标：完成 [PLAN_V2.0.0.md](./PLAN_V2.0.0.md) 中 Ask、Organize、Research 2.0、Platform 2.0
> 四个 P0 主题，并落实 V2.0.0 发布出口中属于 P0 的验证门槛。
> 本文件是执行状态的事实清单；只有代码、测试、产物或真机报告能作为完成证据。

**当前结论（2026-10-23）**：P0 的可编码、可自动验证部分已完成；全量自动化测试、
桌面二进制构建、签名清单生成器和性能守护均通过。剩余项全部是需要外部凭据、真实模型或
真实设备的人工验收，统一见 [V2_P0_MANUAL_ACCEPTANCE.md](./V2_P0_MANUAL_ACCEPTANCE.md)。
在这些人工项完成前，本目标仍保持 active，不能宣称 V2.0.x P0 全量完成。

### 自动化证据（2026-10-22 / 23）

| 检查 | 结果 |
| --- | --- |
| `pnpm test` | 83 文件 / 418 测试通过 |
| `cargo test --offline` | 全部通过；library 205 passed / 1 ignored，全部集成测试通过 |
| `pnpm lint` / `pnpm build` | 通过 |
| `pnpm test:e2e` | 7 条通过，含 Ask、Research、Graph、Writing、Sync 用户路径 |
| `pnpm tauri build --debug --no-bundle` | 桌面二进制构建通过 |
| `pnpm test:ask-eval` | 50 正样本、20 负样本全部过门禁 |
| Ask 作用域与追问隔离 | 10 条查询覆盖 Note / Category / Tag / All；5 轮虚构历史答案均未进入本轮结论或引用 |
| Research 网络中断恢复 | 真实本机 TCP 中途断连后保留已完成来源、进入可恢复状态；恢复网络后补缺且不复制来源 |
| AI 总开关本地链路 | 真实 UI 开启后自检为 `template.draft`；Ask `2ms`、Research 立项 / 收集 / 验证 / 汇总 `5/4/7/5ms`，审计均 `outbound=false`（[证据](./acceptance/V2_AI_ON_SMOKE_20261022.md)） |
| AI 关闭 / 模板零外发 | 真实 Ask 交互下，AI 关闭与模板档均被 macOS `nettop` 观察为无 TCP / UDP 连接 |
| 本地 Ollama 真实模型 | `qwen2.5:7b` 100% Metal GPU、RSS 4.84GB；应用内 Ask `1769ms` / 追问 `905ms`，provider `local.ollama:qwen2.5:7b` |
| 本机真实同步服务 | rclone WebDAV / S3 均完成 PUT / GET / DELETE；应用两逻辑设备上传、dry-run、下载解密通过，远端无明文 |
| DeepSeek 在线模型 | `deepseek-flash` 首个正文 token 中位 1201.6ms、总耗时中位 1780.5ms；10 题最终引用正确 10/10，首轮在线引用合规 9/10，失败均安全降级 |
| 决策 Key 持久化 | 修复后端忽略 `decisionApiKey` 与前端保存失败仍清空输入的问题；Rust 凭据测试和前端保留输入测试通过 |
| macOS release `.app` 最终复建 | ad-hoc 签名校验通过；真实 `knowledge` 工作区打开成功；假决策 Key 可保存并清除，凭据文件仍为 `0600` |
| `pnpm test:release-manifest` | 三平台映射、Linux `AppImage.tar.gz`、GitHub 资产名归一化与缺签名拒绝测试通过 |
| `pnpm test:update-signature` | 一次性测试密钥：有效载荷通过、篡改载荷拒绝、版本签名绑定校验通过 |
| `runtime_contract` / release App 实启 | updater 配置可反序列化；窗口位置恢复权限完整；macOS release 包真实打开 10 万篇工作区 |
| macOS 真机关键 UI + P0 原生入口 | 126 个自动步骤通过 / 0 失败；6 条视觉判断保留人工 |
| macOS P0 用例 4 状态污染复测 | 39 个自动步骤通过 / 0 失败；拖拽/悬停手感仍人工 |
| macOS 当前候选完整原生套件 | `49de15f` 全新隔离工作区 321 步通过 / 0 失败；11 条视觉或外部条件待人工 |
| 验收脚手架工作区核验 / 文件断言 | 4 个 Node 测试通过；错误 workspace 写入前中止；用例 1 为 10/10 |
| `pnpm bench` | 1 万检索 p95 8.06ms；3 万图谱 821.5ms |
| 10 万扩展门禁 | 检索 p95 77.76ms、recall@5 100%、后端重开 1140.77ms、RSS 350.84MB |
| macOS 完整 App 10 万篇 | release/ad-hoc：稳定首帧 1001ms、RSS 224.23MB；重建后首次 2947ms 已单独记录；截图为 100001 篇就绪界面 |
| updater manifest 脚本 / release.yml | 合成夹具通过；YAML 解析通过 |
| 当前 V2 开发版 Windows / Linux 产物 | Windows PE / NSIS / 便携包结构通过；Ubuntu 22.04 x86_64 deb 安装启动与卸载数据保留通过；AppImage 解包启动通过 |

人工执行时从
[V2_P0_REPORT_TEMPLATE.md](./acceptance/V2_P0_REPORT_TEMPLATE.md)
复制报告骨架，确保每项都带设备、构建 SHA、证据与结论。

## 0. 执行原则

1. 先数据契约与持久化，再服务，再 UI；不把“界面能点”当作完成。
2. 所有 AI 写入都是“建议 → 预览 / diff → 明确确认 → 常规业务写入 → 可撤销”。
3. 默认保守：AI 总开关关闭、仅本地、外发独立授权、未授权零请求。
4. 每个切片必须补自动化测试，并跑相关 Rust / frontend / E2E 回归。
5. 需要证书、真实设备、真实模型凭据或人工判断的事项，进入
   [V2_P0_MANUAL_ACCEPTANCE.md](./V2_P0_MANUAL_ACCEPTANCE.md)，不得用 mock 结果冒充。
6. P1（Graph 2.0 / Sync 2.0 / Scale 2.0）与 P2（Writing）不混入当前 P0 范围；
   其中发布出口要求的 10 万篇基准单独记录，未达标前不得宣称 V2.0.0 可发布。

## 1. 依赖顺序

```text
M0-A 契约与持久化
  ├─ citation/suggestion/job DTO
  ├─ ai_suggestions + ai_jobs 迁移
  ├─ SQLite repository
  └─ 持久化状态机测试
        │
        ├─ Ask：50 条评测集 → 指标门禁
        ├─ Organize：生成 → 队列 → 接受/拒绝/撤销 → 批量任务
        ├─ Research 2.0：计划编辑 → 重跑对比 → 三入口 → 恢复
        └─ Platform 2.0：更新器 → 签名闸门 → 诊断 → 真机清单
```

## 2. P0-A：Ask 引用式问答

| 项目 | 状态 | 完成证据 |
| --- | --- | --- |
| 当前笔记 / 分类 / 标签 / 全库入口 | ✅ 已实现 | `ask_question` + 桌面 / mock 测试 |
| 精确原文块引用与点击跳转 | ✅ 已实现 | `tests/ask_flow.rs` + 面板测试 |
| 未知引用编号整份拒绝并降级 | ✅ 已实现 | Rust 单元 / 集成测试 |
| 在线 → 本地 → 模板降级 | ✅ 已实现 | Ask 集成测试、零外发审计测试 |
| 多轮追问、每轮重新检索 | ✅ 已实现 | Ask 多轮集成测试 |
| 单次输入 Token 预算 / 超限零模型请求 | ✅ 已实现 | 6000 输入 Token 估算闸门 + 本地模型调用数断言 |
| 选中文字右键 / 格式条 / 命令面板入口 | ✅ 已实现 | 右键菜单、格式条与命令面板组件测试 |
| 答案预览后存为笔记、来源冲突拦截 | ✅ 已实现 | Ask 保存闭环与 `NOTE_CONFLICT` 测试 |
| 50 条标注问答集 | ✅ 自动 fixture | `tests/ask_eval_flow.rs`，结果见 [V2_ASK_EVAL.md](./V2_ASK_EVAL.md) |
| 引用可解析率 100% 门禁 | ✅ 已实现 | 评测实测 50/50 |
| 引用精确率 ≥ 95% 门禁 | ✅ 已实现 | 模板档结构化评测 100%（100/100 citation 断言） |
| 无依据断言率 < 5% | 🟡 代理检查 | 模板答案只整理命中片段；真实模型断言级复核待人工 |
| “库里没有”问题 100% 证据不足 | ✅ 已实现 | 20/20 负样本干净返回证据不足 |
| 全弱来源 / 全过期来源拒绝给结论 | ✅ 已实现 | E 级线索全过滤；来源全部 stale 时零引用并说明缺口 |
| 50 条标注样本人工复核 | 🧑 人工 | 人工验收清单 |

## 3. P0-B：Organize 摘要 / 标签 / 关联建议

### 3.1 数据与状态机

| 项目 | 状态 | 完成证据 |
| --- | --- | --- |
| `ai_suggestions` 表：kind/status/model/promptVersion/evidence/sourceHash/撤销前后哈希 | ✅ 已实现 | 迁移 014 / 015 + schema 文档 |
| 同一笔记同一类只保留最新 pending | ✅ 已实现 | 部分唯一索引 + repository 测试 |
| `ai_jobs` / `ai_job_items` 持久化队列 | ✅ 已实现 | 迁移 + 状态机测试 |
| 进程重启后恢复 / 暂停状态 | ✅ 已实现 | 真实子进程 `abort()` → 重开工作区 → paused → 续跑完成 |
| 接受时外部修改冲突检查 | ✅ 已实现 | 哈希变化返回 `NOTE_CONFLICT` |
| 接受前文件字节不变 | ✅ 已实现 | 生成 / 拒绝路径文件字节断言 |
| 接受后一键撤销 | ✅ 已实现 | raw bytes 快照 + 接受后哈希冲突拦截 + 字节级恢复测试 |

### 3.2 能力

| 项目 | 状态 | 完成证据 |
| --- | --- | --- |
| 单篇摘要建议 | ✅ 已实现 | 服务 + DTO + 增量预览 |
| 多选 / 当前列表范围摘要 | ✅ 已实现 | 批量 job |
| 标签建议，优先复用已有标签 | 🟡 代码完成 | 规范化 / 200 条精确率样本待人工 |
| 关联建议与“为什么” | ✅ 已实现 | 普通 Wiki 链接 + 类型化关系两种形态，均带相似度理由 |
| 重复 / 近重复检测三路投票 | ✅ 已实现 | 标题 / 正文 / 可选向量票，至少两票 |
| 待确认队列 UI | ✅ 已实现 | 接受 / 拒绝 / 批量接受 / 增量预览 |
| 后台任务进度 / 暂停 / 恢复 / 取消 / 重试 | ✅ 已实现 | UI + 持久化 job |

### 3.3 验收

| 项目 | 状态 | 完成证据 |
| --- | --- | --- |
| 100 篇任务暂停/恢复/取消/重试 | ✅ 已实现 | 自动化集成测试；解除阻塞后可重试完成 |
| 杀进程重启后继续 | ✅ 已实现 | 持久化状态重建测试 |
| 标签精确率 ≥ 80% | 🧑 人工 | 200 条人工标注 |
| 关联接受率 ≥ 60% | 🧑 人工 | 使用记录 / 标注 |
| 拒绝 / 撤销字节级恢复 | ✅ 已实现 | 字节断言 |
| 外部修改冲突 100% 拦截 | ✅ 已实现 | 冲突矩阵测试 |
| 未接受前工作区文件不变 | ✅ 已实现 | 文件字节快照断言 |

## 4. P0-C：Research 2.0

| 项目 | 状态 | 完成证据 |
| --- | --- | --- |
| 现有 `/research` 五步流程 | ✅ 已实现 | `tests/research_flow.rs` |
| 选中文字入口 | ✅ 已实现 | 编辑器右键菜单 / 格式条 → Research seed |
| 命令面板入口 | ✅ 已实现 | 命令面板动作 + 组件测试 |
| Ask 对话“深入调研”入口 | ✅ 已实现 | Ask 答案操作 → Research seed |
| 研究计划可编辑（子问题 / 时间窗 / 来源类型 / 语言） | ✅ 已实现 | DTO + Playwright 用户路径 + 查询重算测试 |
| 计划保存为模板、可复用 | ✅ 已实现 | Playwright 保存/可选 + `.knowledge/research-templates` 复用测试 |
| 同一课题重跑并对比来源 / 结论变化 | ✅ 已实现 | `research_rerun / research_compare` + Playwright 差异视图 |
| 来源 freshness：fetched/published/applicable_version | ✅ 已实现 | 本地笔记从 frontmatter 读取适用版本；发布时间参与 stale 判定；报告与来源 UI 展示 |
| 采集中断后按已完成来源继续 | ✅ 已实现 | `research_resume` 保留来源并去重续跑 |
| 三入口走同一流程 | ✅ 已实现 | 统一 `/research` seed + 后端同一 start 流程 |
| 单来源结论 100% 标未交叉验证 | ✅ 已实现 | Research 测试 |
| 证据不足 100% 不给结论 | ✅ 已实现 | Research 测试 |
| 写回前确认、取消无半成品 | ✅ 已实现 | Research 写入 / 中止测试 |
| 报告 + 来源清单双链 | ✅ 已实现 | 报告正文 `[[来源清单]]`，来源清单反链报告标题 |

## 5. P0-D：Platform 2.0

| 项目 | 状态 | 完成证据 |
| --- | --- | --- |
| Windows / Linux / macOS CI 三平台产物 | ✅ 已实现 | `release.yml` |
| macOS ad-hoc 签名 | ✅ 已实现 | 当前 `signingIdentity = "-"` |
| macOS Developer ID + 公证 | 🧑 凭据 / 待真机 | workflow 已接 Apple 环境变量；证书/账号待配置 |
| Windows 代码签名 | 🧑 凭据 / CI 已接线 | PFX 导入 + thumbprint 注入；SmartScreen 待真机 |
| Tauri updater 方案 | ✅ 已实现 | Tauri updater + GitHub `latest.json` |
| 更新包签名校验，失败拒绝并保留旧版 | ✅ 代码 / 验签完成 | 有效载荷通过、篡改载荷拒绝；真机安装失败保留旧版仍需人工 |
| 自动更新失败可回滚 | 🧑 人工 | 需真机验证“拒绝后旧版仍可启动”，并保留上一版安装包 |
| 崩溃日志本地保存、默认不上传、可导出 | ✅ 已实现 | 本地 app.log + `app_diagnostic_export` |
| Windows 真机安装 / 升级 / 卸载 / 便携版 | 🧑 人工 | 真机报告 |
| Linux 22.04 / 24.04 deb + AppImage | 🧑 人工 | 真机报告 |
| macOS 首次打开无“仍要打开” | 🧑 人工 | Developer ID + 公证后验证 |

## 6. P0 出口（自动 + 人工）

1. Ask 评测指标全部过门禁。
2. Organize 所有写入路径都有确认、冲突检查、撤销和审计。
3. Research 三入口、可编辑计划、重跑对比和续跑完成。
4. Platform 更新链路在测试签名包上完成正向 + 负向验证。
5. 未授权 AI 时网络 mock + 审计均证明 0 次外发。
6. Windows / Linux / macOS 真机清单统一执行并留存报告。
7. 发布前执行完整测试、E2E、性能基准和发布物抽验。

> 10 万篇后端指标和当前 macOS 完整 App 已过线；Developer ID/公证后的干净机器冷启动、
> Windows/Linux 真机仍需统一人工验收。
