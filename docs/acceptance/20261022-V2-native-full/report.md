# V2 macOS 原生完整验收复测

> 后续同日的导出 / 恢复自动化扩容已把完整套件提升到
> **321 步通过 / 0 失败 / 11 条待人工**，见
> [V2_NATIVE_EXPORT_RESTORE_20261022.md](../V2_NATIVE_EXPORT_RESTORE_20261022.md)。
> 本文件保留 2026-10-22 较早一轮 `0dd550c` 的原始证据，不覆盖历史结果。

## 现场

- 日期：2026-10-22
- 提交：`0dd550c`
- 版本：`1.3.7`（V2 开发线）
- 平台：macOS 27 / arm64
- 应用：`Personal Knowledge OS.app`，ad-hoc 签名
- 数据目录：`/private/tmp/pkos-v2-native-final.jEZFTK/data`
- 隔离工作区：`/private/tmp/pkos-v2-native-final.jEZFTK/workspace`
- 自动门禁：全部通过

## 结果

- 自动步骤：**238 通过 / 0 失败**
- 待人工：**13 条**
- 截图：24 张
- 用例 10 首次启动放行：人工，仅适用于从网络下载的包

本次在全新隔离工作区运行，不依赖旧 `/private/tmp/pkos-acceptance` 的人工夹具。

## 本轮修复

1. 用例 4 通过 UI 新建唯一目标笔记，不再依赖旧“目标笔记”。
2. 用例 9 自动写入唯一 `valid_until` 过期夹具，再断言待复核卡片和时效角标。
3. 用例 11 自动创建唯一分类与标签夹具，并使用 `ensureHidden` 处理折叠状态时序。
4. 验收器新增受限 `writeFile`：只允许当前已核验工作区内的相对路径，拒绝绝对路径和
   `..`，使用排他创建避免覆盖。
5. 修复外部文件扫描后的标签缓存：`filesChanged` 现在同时刷新笔记、统计、分类与标签，
   标签已入库但侧边栏仍显示“暂无标签”的问题不再复现。

## 证据

原始报告：

- `/private/tmp/pkos-v2-native-final.jEZFTK/acceptance-full/report.md`
- SHA-256 `984a90392568446dadc8a7ee1dedd24bf69adca183e7e0c3399b780704fae35c`

机器可读结果：

- `/private/tmp/pkos-v2-native-final.jEZFTK/acceptance-full/report.json`
- SHA-256 `ed6c15e38577da73de5f32267001860ea5063826a87d40cb770ba42dac21de66`

截图位于同目录，默认不进 Git。

## 保留人工

- 编辑器 `[[` 候选排序、原生日期控件观感。
- 导出 / 恢复原生目录选择与文件可打开性。
- 备份恢复结果人工打开确认。
- 拖拽跟手与贴边大纲悬停动效。
- 版本历史菜单 / diff / 还原的视觉确认。
- 网页剪藏正文完整度。
- Research 真实 AI 开关与真实联网链路。
- 待复核角标文案与跳转详情。
- 首次启动 Gatekeeper。
- 长标题按钮显隐与间距、Mermaid 配色、图片尺寸等视觉判断。

## 结论

自动化可覆盖的 macOS 原生流程全部通过，本轮没有功能失败。V2.0.x 仍不能宣布完成，
因为统一人工清单中的真实模型、远端同步、签名公证、Windows / Linux 真机与主观体验
尚待执行。
