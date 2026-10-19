# V1.3.6 验收记录

本版只修绿色按钮：**保留系统全屏箭头外观，但点击只做缩放**。无功能 / 数据结构变化。

## A. 发布前守护（本机）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 版本一致性（四处） | ✅ `1.3.6` | `package.json` / `Cargo.toml` / `Cargo.lock` / `tauri.conf.json` |
| 校准探针版本题 | ✅ `1.3.6` | `hidden_version_yes` |
| 前端 | ✅ 70 文件 / 377 用例 | `CI=true pnpm test` |
| 构建与静态检查 | ✅ | `pnpm lint`、`pnpm build` |
| E2E 主流程 | ✅ 1 条通过 | `CI=true pnpm test:e2e` |
| Rust | ✅ 47 个测试目标全绿 | `cargo test --offline` |

## B. 绿色按钮（真机）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 按钮语义 | ✅ `full screen button` | 无障碍树 `description of button 2`；系统绘制对角箭头 |
| 点击行为 | ✅ 只缩放 | 点击后 `AXFullScreen = false`；窗口 `0,33,1512×859` |
| 菜单 `Zoom` | ✅ 缩放切换 | 再点菜单恢复 `66,33,1380×859`，仍未进全屏 |
| 自绘 | ✅ 无 | 窗口按钮全部由 macOS 绘制；只在窗口层重定向 `toggleFullScreen:` |

截图：`/tmp/pkos-zoom.png`（系统缩放状态）。

## C. 产物与发布

推 `v1.3.6` 触发 CI；发布后抽验见 §D。

## D. 发布后抽验

（CI 完成后填写。）

## E. 已知限制

- “全屏”实际是系统缩放：铺满可用区域，但保留菜单栏 / 标题栏 / 红黄绿；
- macOS 未做 Developer ID 签名与公证；Windows / Linux 未在真实设备运行。
