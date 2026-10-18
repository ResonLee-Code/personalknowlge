# V1.3.1 验收记录

口径与 [ACCEPTANCE_V1.3.0.md](./ACCEPTANCE_V1.3.0.md) 一致：**没验的不写成通过**；
只能人眼判断的标「待人工」；不确定的写清"哪里还不确定"。

本文件记录 **V1.3.1**（2026-10-19）出包前后的验证实况。
本版是 **V1.3.0 之后 main 上 20 个提交的收口**：编辑器补上图形 / 插入 / 格式 / 折叠，
界面去重复、收拢工具栏、设置页按 Codex 重排，并修掉「插入图片点了没反应」。

## A. 发布前守护（本机）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 版本一致性（四处） | ✅ `1.3.1` | `package.json` / `src-tauri/Cargo.toml` / `Cargo.lock` / `tauri.conf.json`；`pnpm test` 的版本身份守卫 |
| 前端 | ✅ 69 文件 / 374 用例 | `CI=true pnpm test`；`pnpm lint`、`pnpm build` 通过 |
| Rust | ✅ 47 个测试目标全绿 | `cargo test --offline` |
| E2E 主流程 | ✅ 1 条通过 | `CI=true pnpm test:e2e`（新增：插入菜单、格式条实测宽度、收藏橙色、设置页导航与搜索、大纲折叠、mermaid 渲染、插入图片必须弹选择框） |
| 性能基准 | ✅ 全部过线 | `pnpm bench`（1 万检索 p95 + 3 万图谱全量，阈值见脚本） |
| 真机验收（E1 脚本） | ✅ **13 条用例 / 自动步骤 215 步通过 / 0 步失败** | `docs/acceptance/20261018-235856/report.md`（含新增第 13 条：插入图片整链路——原生选文件 → `attachments/` 落盘 → 正文写入链接） |

## B. 产物与发布

> 由「推 `v1.3.1` 标签」触发 CI 三平台构建并创建 Release；**发布后逐项抽验**见 §C。

| 平台 | 产物 |
| --- | --- |
| macOS（arm64） | `.dmg` + `.app`（ad-hoc 签名） |
| Windows（x64） | NSIS `setup.exe` + `portable.zip`（未签名） |
| Linux（x64） | `.deb` / `.rpm` / `.AppImage` |
| 校验 | 每平台一份 `SHA256SUMS-<platform>.txt` + 汇总 `SHA256SUMS-all.txt` |

## C. 发布后抽验

…

## D. 已知限制（与 V1.3.0 相同，未因本版改变）

1. **未签名（Developer ID）/ 未公证**：`spctl --assess` 仍是 `rejected`；下载副本首次打开需放行一次；
2. **Windows / Linux 产物未签名**，且**尚未在真实设备上验收**（由 macOS 交叉编译；已做结构校验）；
3. **图（Mermaid）在导出里的表现**：预览与 PDF（打印视图）是渲染后的图；
   HTML / Word 导出保留的是**源码**（导出器不是浏览器，不执行 mermaid）——需要图时用 PDF 或截图；
4. 其余沿用 [ACCEPTANCE_V1.3.0.md](./ACCEPTANCE_V1.3.0.md) §D。

## E. 相关文档

- [CHANGELOG.md](./CHANGELOG.md)：V1.3.1 的用户可见变化
- [GUIDE.md](./GUIDE.md) / [GUIDE.en.md](./GUIDE.en.md) / [MANUAL.md](./MANUAL.md)：完全指南与逐控件手册（均按 V1.3.1 更新）
- [RELEASE_PROCESS.md](./RELEASE_PROCESS.md)：发版步骤 + `develop` / `main` 分支约定
