# V2.0.x P1 / P2 自动验收记录

## 构建与环境

- 日期：2026-10-22
- 分支：`codex/v2.0-ask`
- 版本：1.3.7（开发线，不是 V2 正式发布号）
- schema：16（新增迁移 016：FTS 4 分片）
- 平台：macOS 27 / arm64 / 24 GB / 15 核

## 自动门禁

| 门禁 | 结果 |
| --- | --- |
| Rust 全量测试 | 通过；library 204 passed / 1 ignored，全部集成测试通过 |
| 前端 Vitest | **81 文件 / 412 用例通过** |
| ESLint | 通过 |
| TypeScript / Vite production build | 通过 |
| Playwright E2E | **7/7 通过**（含 Ask 保存、Research 计划/模板/重跑、Graph 洞察、Writing diff、Sync 设置） |
| updater 正负验签 | 有效载荷通过、篡改拒绝、版本绑定 `2.0.0` |
| 前端生产依赖审计 | `pnpm audit --prod --audit-level high`：**No known vulnerabilities** |
| Rust 依赖审计 | `cargo-audit 0.22.2`：**0 vulnerabilities**；6 个上游 unmaintained/unsound warnings |
| Graph 10 万首屏 | **2866.6 ms**（< 3 s） |
| 检索 1 万 | p95 **18.09 ms**、recall@5 **100%** |
| 检索 10 万（4 分片） | p95 **190.69 ms**、recall@5 **100%**、重开 **1246.09 ms**、RSS **344.64 MB** |
| release 二进制 | **13,553,840 B**；SHA-256 `0b47752d3897970da3d792e7a2b7fed4cb6e760f59645bcc31f52e8569f69361` |
| `.app` 包内二进制 / 包文件合计 | **13,553,840 B / 13,713,206 B**；包内二进制 SHA-256 `9b878b2d8bd59c20bc59220b91735c3fec7f98bc33eadf21e8535d0da84767bd` |
| macOS `.app` 构建 | ad-hoc 签名成功；公证因无凭据跳过 |
| macOS `.app` 后端启动冒烟 | 进程存活，迁移 1–16 全部完成，文件监听启动，主窗口存在 |

`cargo clippy --all-targets -D warnings` 仍被仓库已有的历史 lint 债务拦住（策略/服务/文本处理
等旧文件）；本轮新增文件与改动点已按 Clippy 反馈清零，未把全仓历史债混入本功能提交。
`pnpm format:check` 也仍被 83 个既有前端文件拦住；本轮新增 / 接触文件已通过 Prettier，
没有把全仓机械格式化混入功能提交。
`cargo-audit` 使用官方 v0.22.2 macOS arm64 二进制，经 GitHub 资产 SHA-256 校验后执行。
CI preflight 固定安装 `cargo-audit 0.22.2` 并以 `cargo audit --no-yanked` 阻断已知漏洞。
剩余 warning 来自上游未维护/unsound 标记；已如实保留，不能视为完全无警告。

## P1 / P2 专项

- Graph：近邻证据、重复聚类、桥梁/孤儿/孤岛测试通过；E2E 已实际走通“重复预览 →
  生成引用摘要 → 确认新建摘要笔记”；10 万抽样门禁通过。
- Sync：本地 WebDAV / S3 桩、双设备新增/修改/删除/冲突/合并、附件、索引缓存、
  密文、篡改拒绝、错误口令、离线队列跨重启、凭据 `0600`、缺失 ETag 拒绝覆盖、
  `If-None-Match` 首次创建保护、覆盖前历史留档/确认还原全部通过；
  非回环明文 HTTP 在凭据落盘 / 外发前即被拒绝；
  关闭同步并断开远端后检索、图谱和写作仍可用。
- Scale：迁移 016、四片映射一致性、跨分片检索、分页、后台扫描、虚拟结果列表通过。
- Scale：分片内增量更新/删除保持映射一致；含连字符的查询不再触发 FTS 语法错误。
- Writing：五模式、逐段引用、来源漂移、目标哈希、diff、确认写入通过。
- Writing：Graph 聚类摘要保存同样在来源漂移后拒绝写入。
- App 外壳：新增 React 错误边界；生产包发生未捕获渲染错误时显示可恢复界面并写入
  `render` 日志，不再只剩空白窗口（自动测试 1/1）。
- Ask：浏览器 E2E 已走通“提问 → 引用材料 → 存为笔记预览 → 确认写入 → 新笔记成为当前页”。
- Research：浏览器 E2E 已走通“编辑子问题/时间窗/语言/来源类型 → 保存模板 →
  本地收集 → 同课题重跑 → 查看新增/消失来源与结论变化”。
- Organize：后台任务 UI 覆盖运行中暂停、暂停后继续、失败后重试、运行中取消；
  Rust 集成测试补齐失败任务在阻塞解除后的重试完成。
- 安全边界：AI / 同步凭据改为同目录原子私有写入，Unix 创建即 `0600`，不跟随预置
  符号链接；导出 HTML 转义笔记原始 HTML、阻止脚本协议并写入 CSP；
  `127.evil.com` / `127.0.0.1.evil.com` 不再被误判为本机，明文同步凭据不会被放行。
- 备份安全：ZIP 条目先整体预检路径；含 `..` 穿越的归档校验为无效，解压在创建目标目录和
  写出任何文件前中止；正常备份创建、恢复及附件/历史/模板保留回归通过。
- 发布安全：GitHub Actions 默认权限降为 `contents: read`，只有 Release job 获得
  `contents: write`；所有外部 Action 锁定完整 commit SHA；新增 workflow 权限与引用
  静态测试并接入 preflight。
- updater 签名脚本使用 `cargo test --locked`，可在干净 runner 首次下载依赖并保持
  `Cargo.lock` 不变；有效载荷与篡改拒绝路径继续通过。
- Rust 依赖安全：`zip` 升至 9.0.5；updater 通过固定上游 commit 使用 `zip 9`；
  `rustls 0.23.46`、`rustls-webpki 0.103.16`、`webpki-roots 1.0.10` 消除审计发现的
  9 个漏洞。
- CI preflight 强制执行 ESLint、production build、验收器自测和 Playwright E2E，
  并由 workflow 静态测试固定顺序（build / Chromium 安装先于 E2E）。
- 发布 workflow 通过 `actionlint 1.7.12` 语义检查；Release job 直接依赖 `preflight`，
  可合法读取版本 output 生成 `latest.json`。

真机验收脚手架另有 4 个 Node 测试通过：错误 workspace 在任何 UI 写入前中止；
“最近生成 + 正文匹配”必须命中同一个最新文件。真实 App 复测用例 1 为 10/10、
用例 4 为 39/39。

2026-10-22 最后一轮重验时 macOS 会话已锁屏。进程级启动、数据库迁移、文件监听和主窗口
创建可验证，但锁屏会屏蔽窗口内容、辅助功能树和截图，因此最新 `.app` 的“内容可见且可
交互”冒烟不能在锁屏状态下记通过；解锁后必须补跑。

## 保留人工

见 [V2_P1_P2_MANUAL_ACCEPTANCE.md](../V2_P1_P2_MANUAL_ACCEPTANCE.md)。未完成项包括
200 对近邻标注、真实 WebDAV/S3 与物理双设备、真实模型质量、Developer ID / 公证、
Windows/Linux 真机及签名包升级。
