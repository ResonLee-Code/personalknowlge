# V1.3.7 验收记录

本版只修一个崩溃：**手动关闭窗口弹出“意外退出”**。无功能 / 数据结构变化。

## A. 发布前守护（本机）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 版本一致性（四处） | ✅ `1.3.7` | `package.json` / `Cargo.toml` / `Cargo.lock` / `tauri.conf.json` |
| 校准探针版本题 | ✅ `1.3.7` | `hidden_version_yes` |
| 前端 | ✅ 70 文件 / 377 用例 | `CI=true pnpm test` |
| 构建与静态检查 | ✅ | `pnpm lint`、`pnpm build` |
| E2E 主流程 | ✅ 1 条通过 | `CI=true pnpm test:e2e` |
| Rust | ✅ 47 个测试目标全绿 | `cargo test --offline` |

## B. 关闭崩溃：复现与修复

### B.1 复现

- 系统：macOS 27.0.1（26A434）
- 启动 v1.3.6，等待 10 秒，点红色关闭键
- 结果：macOS “意外退出”对话框；新增
  `personal-knowledge-os-2026-10-20-213344.ips`
- 崩溃：`EXC_CRASH / SIGABRT`；主线程
  `-[NSView removeFromSuperview]` → ObjC exception rethrow → `abort()`

### B.2 根因

v1.3.6 用 `object_setClass` 把主窗口换成动态子类 `PKOSZoomWindow`，用于把
`toggleFullScreen:` 重定向到 `zoom:`。AppKit 在关闭窗口、拆视图层级时不接受这种窗口类替换。

### B.3 修复与验证

- 改为 `class_replaceMethod`：只替换 `NSWindow` 的 `toggleFullScreen:` 实现，
  **窗口类保持原生 `NSWindow`**；
- 修复后红键关闭：无新增 `.ips`；
- `⌘Q` 退出：无新增 `.ips`；
- 绿色按钮仍为 `full screen button`（系统对角箭头）；点击后 `AXFullScreen = false`，
  窗口只缩放。

可复跑回归：`scripts/acceptance/close-smoke.sh`（启动 → 5 秒后点红键 → 检查是否有新 `.ips`）。
本机连续跑 **3 次**，3/3 **PASS：关闭窗口后没有新的崩溃报告**。

> 说明：原生红黄绿的 hover 图标由系统绘制，自动化抓图仍无法触发，但按钮语义已经是
> `full screen button`，不再是 `zoom button` 的 `+`。

## C. 产物与发布

推 `v1.3.7` 触发 CI；发布后抽验见 §D。

## D. 发布后抽验

（CI 完成后填写。）

## E. 已知限制

- “全屏”实际是系统缩放：铺满可用区域，保留菜单栏 / 标题栏 / 红黄绿；
- macOS 未做 Developer ID 签名与公证；Windows / Linux 未在真实设备运行。
