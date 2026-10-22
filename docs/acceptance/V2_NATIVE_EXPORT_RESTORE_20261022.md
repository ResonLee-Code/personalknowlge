# V2 原生导出、DOCX 回读与备份恢复自动化

## 现场

- 日期：2026-10-22
- 基线提交：`49de15fbb17cbc468157ef2c477e901c4cffef9a`
- 版本：`1.3.7`（V2 开发线）
- 平台：macOS 27 / arm64
- 应用：`Personal Knowledge OS.app`，ad-hoc 签名
- 数据目录：`/private/tmp/pkos-v2-49de15f-native-clean.WsegHg/data`
- 隔离工作区：`/private/tmp/pkos-v2-49de15f-native-clean.WsegHg/workspace`

## 结果

- 自动步骤：**321 通过 / 0 失败**
- 整体通过：用例 3、9
- 待人工：**11 条**（均含视觉判断、真实模型 / 网络或外部平台条件）
- 截图：27 张
- 用例 2：全部功能步骤通过，仅保留 Word / 浏览器最终排版观感

## 本轮新增自动化

1. Markdown / HTML / DOCX 三种格式均通过真实 macOS 保存面板落盘。
2. Markdown 校验 frontmatter、标题和唯一正文；HTML 校验 HTML 根元素、CSP 和正文；
   DOCX 校验 ZIP 签名、`word/document.xml` 与唯一正文。
3. 导出的 DOCX 经系统打开面板重新导入；断言回读标题、正文与工作区 Markdown 文件。
4. 创建并校验备份后，经原生目录面板恢复到空目录。
5. 对恢复库执行 SQLite `PRAGMA integrity_check`、唯一笔记查询和 Markdown 正文断言。
6. 从应用工作区切换器真实打开恢复库，核对标题与正文，再切回原库并校验数据库进程句柄。
7. 首页“待复核”卡片实际点击，断言跳转后的笔记标题、“可能已过期”和“已索引”。

## 验收器改进

- 原生面板确认不再遍历整棵文件对话框 AX 树，改为读取 sheet 状态并在原生确认按钮位置
  投递鼠标事件，避免大目录扫描超时。
- 导出 / 恢复产物只允许写入本次运行独占的 `os.tmpdir()/pkos-native-acceptance/...`；
  路径解析继续拒绝绝对路径和 `..`。
- 新增可选精确 AX 查询，避免“导出”命中带“原样导出”说明的格式按钮。
- 工作区切换器按当前数据库根路径定位，并从进程打开的 SQLite 句柄复核切换结果。

## 证据

原始报告：

- `/private/tmp/pkos-v2-full.LjUEwz/out/report.md`
- SHA-256 `a4c7dac5c12707b93a9704bef2302f7cd04ab40ac65f897042ddd6494ee0acda`

机器可读结果：

- `/private/tmp/pkos-v2-full.LjUEwz/out/report.json`
- SHA-256 `fe8fe95a47d7bda4cb78f63bcb07842b212d8e2d5e224fcf7a3f76d0375d69a9`

截图位于同目录，默认不进 Git。

## 保留人工

- 编辑器 `[[` 候选顺序与原生日期控件观感。
- 导出文件在 Word / 浏览器中的最终排版、分页和字体观感。
- 拖拽跟手、贴边大纲悬停动效与留白。
- 版本历史菜单、diff 和还原的视觉确认。
- 网页剪藏正文完整度。
- Research 真实 AI 开关与真实联网链路。
- 首次启动 Gatekeeper。
- 长标题按钮显隐、Mermaid 配色与图片尺寸等视觉判断。

## 结论

macOS 上可由本机自动驱动的导出、回读、备份、恢复、重新打开恢复库和待复核跳转均已通过。
这不替代真实模型质量、真实 WebDAV / S3、物理双设备、Developer ID / 公证或
Windows / Linux 真机验收；V2.0.x 仍不能宣布完成。
