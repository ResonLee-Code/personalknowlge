# V2.0 P0 macOS 10 万篇完整 App 首屏记录

日期：2026-10-21

机器：macOS 27 / arm64 / 15 核 / 24 GB

构建：release `.app`，ad-hoc 签名（无 Developer ID / 公证）

工作区：`/private/tmp/pkos-100k`，100000 篇基准笔记

数据目录：隔离的 `/private/tmp/pkos-100k-data`

## 方法

```bash
scripts/bench/prepare-scale-fixture.sh --notes 100000 \
  --data-dir /private/tmp/pkos-100k-data /private/tmp/pkos-100k

pnpm tauri build --bundles app

scripts/bench/measure-macos-app.sh \
  --workspace /private/tmp/pkos-100k \
  --data-dir /private/tmp/pkos-100k-data \
  --binary "src-tauri/target/release/bundle/macos/Personal Knowledge OS.app/Contents/MacOS/personal-knowledge-os"
```

首屏以应用日志 `workspace-first-paint` 标记为准，时间从应用进程启动计算到前端完成
工作区首帧；同一次启动同时记录主进程 RSS。测量通过测试环境变量自动打开工作区，
不使用辅助功能树轮询，因此不会把 AX 扫描时间混入首屏。

## 结果

| 指标 | 首次启动（重建后冷进程） | 再次启动（稳定口径） | 红线 |
| --- | ---: | ---: | ---: |
| 进程启动 → 工作区首帧 | 2947 ms | **1001 ms** | < 2000 ms |
| 主进程 RSS | 224.67 MB | **224.23 MB** | < 1536 MB |
| 前端自身首帧标记 | 536 ms | 529 ms | 仅记录 |

首屏截图确认显示 `全部笔记 100001`、`索引就绪（100001/100001）`，工作区内容真实渲染。
启动日志同时确认窗口尺寸与位置恢复成功，旧的 Tauri ACL 拒绝警告已消失。

同一构建的后端 10 万篇基准：

- 检索平均 66.19 ms，p95 77.76 ms；
- recall@5 100%；
- 后端重新打开 1140.77 ms；
- 后端基准进程 RSS 350.84 MB。

## 结论与边界

- 当前 macOS/ad-hoc release 构建的稳定首屏和 RSS 均过线。
- 重建后第一次启动为 2947 ms，包含 macOS 首次验证新二进制/冷磁盘页成本；不能把它隐藏，
  但也不等同于后续稳定启动。最终发布仍需在干净真机上复测冷启动。
- 本记录不能代替 Developer ID 签名、公证、Windows/Linux 真机或真实签名更新测试。

## 2026-10-22 最新安全构建复测

在 `zip 9` / `rustls` 安全升级和最新前端刷新修复后重新构建 `.app`，复测同一
`/private/tmp/pkos-100k`（100001 条记录）：

| 指标 | 首次签名后冷启动 | 稳定启动 | 红线 |
| --- | ---: | ---: | ---: |
| 进程启动 → 工作区首帧 | 2411 ms | **989 ms** | < 2000 ms |
| 主进程 RSS | 223.02 MB | **223.03 MB** | < 1536 MB |

截图 `/private/tmp/pkos-100k-data/scale-first-paint.png` 确认界面真实显示
`全部笔记 100001`、`索引就绪（100001/100001）`，分类、标签和活动统计均已渲染。
首次签名后冷启动仍高于 2 秒，稳定口径低于红线；最终 Developer ID / 公证包仍需在
干净真机复测。
