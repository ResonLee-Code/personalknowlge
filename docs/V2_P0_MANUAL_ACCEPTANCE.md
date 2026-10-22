# V2.0.x P0 统一人工验收清单

> 本清单只放自动化无法证明、需要真实设备 / 真实凭据 / 人工判断的事项。
> 在代码 P0 收口后统一执行，不提前用 mock 或 CI 绿结果代替。
> 每轮执行时从 [验收报告模板](./acceptance/V2_P0_REPORT_TEMPLATE.md) 复制一份并填写证据。
> 统一执行顺序、前置材料与证据目录见
> [V2 统一人工真机验收执行包](./acceptance/V2_UNIFIED_MANUAL_PLAN.md)。
> 2026-10-21 macOS 已执行关键 UI 与 P0 原生入口自动步骤 126/126；完整记录见
> [macOS P0 真机报告](./acceptance/20261021-V2-P0-macOS/report.md)。
> 2026-10-22 当前候选 `49de15f` 已在全新隔离工作区完成完整原生套件
> **321 步通过 / 0 失败 / 11 条待人工**，原始报告见
> [V2 原生验收记录](./acceptance/V2_NATIVE_EXPORT_RESTORE_20261022.md)。

## A. Ask 质量评测

- [ ] 人工复核 50 条问答样本的问题、黄金答案与可引用段落。
- [ ] 核对每条答案的引用是否真正支持对应断言，而不是只检查 ID 存在。
- [x] 记录引用精确率；真实 DeepSeek 连续两轮均为 100%（50/50），见
      [V2 Ask 真实 DeepSeek 50 题评测](./acceptance/V2_ASK_LIVE_DEEPSEEK_20261023.md)。
- [ ] 记录无依据断言率；目标 < 5%。
- [x] 20 条“库中没有答案”的负样本均明确回答证据不足（`ask_eval_flow.rs` 20/20）。
- [x] 10 条查询覆盖不同笔记 / 分类 / 标签 / 全库作用域
      （`ask_flow.rs::ten_queries_cover_note_category_tag_and_all_scopes`）。
- [x] 5 条多轮追问使用不同虚构历史事实，确认上一轮答案未被当成事实
      （`ask_flow.rs::five_follow_ups_never_treat_prior_answers_as_facts`）。

## B. Organize 质量与人工判断

- [ ] 标注 200 条标签建议，计算精确率；目标 ≥ 80%。
      已完成 200 条合成预检：精确率 97.86%、召回 100%，但真实用户知识库的人工标注仍需执行，
      见 [V2 Organize 标签建议 200 题预检](./acceptance/V2_ORGANIZE_TAGGING_20261023.md)。
- [ ] 抽样确认关联建议的“为什么”，记录接受率；目标 ≥ 60%。
- [ ] 对摘要建议做事实一致性抽检，不允许摘要加入原文没有的事实。
- [x] 100 篇批量任务执行暂停、恢复、取消、重试（Rust 集成测试；含解除阻塞后重试）。
- [x] 处理中强制终止进程，重启后确认任务状态与已完成项可恢复（真实子进程 `abort()` 回归）。
- [x] 接受建议后手工修改来源文件，确认旧建议不能再应用（Rust 集成测试）。
- [x] 接受建议后继续编辑正文，再点旧建议的“撤销”，确认被 `NOTE_CONFLICT` 拦截且新内容未丢失（Rust 集成测试）。
- [x] 接受并撤销后，比较文件 SHA-256，必须字节级恢复（Rust 集成测试）。

## C. Research 2.0

- [x] 从选中文字发起研究（上下文菜单 / seed 自动化测试）。
- [x] 从命令面板发起研究（命令面板组件测试）。
- [x] 从 Ask 会话点击“深入调研”（Ask 组件测试）。
- [x] 编辑子问题、时间窗、来源类型与语言后重跑（浏览器 E2E + Rust 集成测试）。
- [x] 确认编辑后的子问题实际影响本地与外部检索查询，而不是只改变界面文字（Rust 集成测试）。
- [x] 给本地笔记设置 frontmatter `applicable_version`，确认来源 UI、报告和来源清单显示一致（Rust 集成测试）。
- [x] 保存研究计划模板，并在新课题中复用（浏览器 E2E 保存 / 可选 + Rust 复用测试）。
- [x] 同一课题重跑，检查新增 / 消失来源与结论变化是否清楚（浏览器 E2E + Rust 对比测试）。
- [x] 手改子问题后重跑，确认新会话沿用同一组子问题（Rust 集成测试）。
- [x] 在联网收集过程中断网 / 中止，恢复后从已完成来源继续
      （真实本机 TCP 断连 + 手动中止回归；恢复后来源去重且补齐缺口）。
- [x] 确认取消后没有半成品笔记（Rust 集成测试）。

## D. Platform 2.0 真机

### 发布前凭据

- [ ] 生成并离线备份 `PKOS_UPDATER_PUBKEY` / `TAURI_SIGNING_PRIVATE_KEY`。
- [ ] 配置 Apple Developer ID 证书、Apple ID / app-specific password、Team ID。
- [ ] 配置 Windows 代码签名 PFX（base64）与密码。
- [ ] 在 GitHub Secrets 写入 RELEASE_PROCESS 中列出的全部凭据。
- [ ] 手动运行一次 workflow，确认三平台都生成 `.sig` 与 `latest.json`。

### Windows

- 构建对象：使用 `49de15f` 对应的 Windows 安装器与便携版；哈希见
  [跨平台产物记录](./acceptance/V2_PLATFORM_ARTIFACTS_20261022.md)。以下真实设备项仍须执行。
- [ ] 全新用户目录安装 NSIS 包。
- [ ] 安装到含中文与空格的路径。
- [ ] 普通用户权限安装，无非必要管理员权限。
- [ ] 从上一版升级，知识库与应用数据不丢失。
- [ ] 卸载应用后知识库数据不受损。
- [ ] 便携版从只读目录 / 普通目录启动。
- [ ] 签名更新包可安装；篡改签名的包被拒绝且旧版仍可启动。
- [ ] 导出诊断日志。

### Linux

- 构建对象：使用 `49de15f` 对应的 deb / rpm / AppImage；哈希见
  [跨平台产物记录](./acceptance/V2_PLATFORM_ARTIFACTS_20261022.md)。以下物理机 / 指定版本项仍须执行。
- [ ] Ubuntu 22.04 安装 `.deb` 并启动。
- [ ] Ubuntu 24.04 安装 `.deb` 并启动。
- [ ] AppImage 赋权后启动。
- [ ] 升级与卸载不损坏知识库。
- [ ] 更新包签名失败时拒绝更新并保留旧版。
- [ ] 导出诊断日志。

### macOS

- [ ] Developer ID 签名通过 `codesign --verify --deep --strict`。
- [ ] 公证通过 `spctl --assess`。
- [ ] 从浏览器下载后首次打开不需要“仍要打开”。
- [ ] 从上一版自动更新到新版。
- [ ] 篡改更新包被拒绝，旧版继续可用。
- [ ] 更新失败时可回滚。

## E. 隐私与数据安全

- [x] AI 总开关关闭时，系统级 `nettop` 对应用进程无 TCP / UDP 记录，审计为
      `AI_DISABLED`（[证据](./acceptance/V2_AI_ON_SMOKE_20261022.md)）。
- [x] 在线模型未授权时，网络代理 / 抓包确认 0 外发
      （真实 DeepSeek Key、`nettop` 15 次采样零 TCP，见
      [V2 在线模型未授权零外发实测](./acceptance/V2_AI_ZERO_OUTBOUND_LIVE_20261023.md)）。
- [x] “逐次外发确认”未勾选时，确认 0 请求
      （真实 DeepSeek Key + `OUTBOUND_UNCONFIRMED` + macOS `nettop` 零连接）。
- [x] 审计日志不含笔记正文与 API Key（`ai_policy_flow.rs` / `remote_model_flow.rs`）。
- [x] 在真实模型请求中核对审计日志不包含完整提示词
      （DeepSeek 实测问题、唯一标记和提示词均未出现在 `ai-audit.jsonl`）。
- [x] 取消 / 拒绝 / 撤销后不残留半成品或未登记文件（Research abandon、Organize reject/undo、
      批量删除撤销与历史恢复测试）。

## F. 发布前人工抽验

- [x] 三平台产物哈希按用户下载方式校验
      （[跨平台产物记录](./acceptance/V2_PLATFORM_ARTIFACTS_20261022.md)）。
- [ ] macOS 签名、版本号、公证。
- [x] Windows PE 结构、安装器与便携版版本一致（PE x64/GUI、NSIS 内嵌本体哈希一致）。
- [x] Linux 包依赖与 AppImage 可执行权限（deb/rpm 元数据、AppImage ELF 权限与实启通过）。
- [ ] 更新源指向正确稳定通道，不使用本地测试地址。
- [ ] `latest.json` 三个平台 URL 都指向 GitHub 实际资产名，均可下载；不得残留 `%20` 空格名。
- [ ] 真实旧版升级到候选版本后，收藏 / 标签 / 版本历史 / 附件 / 回收站均保持。
- [ ] 在 Developer ID/公证后的干净 macOS 真机复测 10 万篇完整 App 冷启动；当前
  ad-hoc release 稳定口径已记录为 1001 ms / 224.23 MB，见
  [macOS 记录](./acceptance/V2_P0_SCALE_MACOS_20261021.md)。
