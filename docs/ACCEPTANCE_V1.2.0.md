# V1.2.0 验收清单（macOS 为主，Windows 对照）

> 生成时间：2026-10-10。产物见 [CHANGELOG.md](./CHANGELOG.md)「当前构建」。
> 标注说明：**✅ 已自动验证**（本机可执行，含证据）；**⬜ 需真机**（必须人工在图形界面操作，
> 括号内为无法自动执行的原因）。

## A. 产物完整性（✅ 已自动验证）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| macOS dmg 可挂载、包含 `.app` 与 Applications 链接 | ✅ | `hdiutil attach` 成功，目录含 `Personal Knowledge OS.app` + `Applications` |
| app 版本与包名 | ✅ | `CFBundleShortVersionString = 1.2.0`，`CFBundleIdentifier = com.personalknowledge.os` |
| 二进制架构与体积 | ✅ | `Mach-O 64-bit executable arm64`，7,787,328 bytes |
| 最低系统版本 | ✅ | `LSMinimumSystemVersion = 10.13` |
| 签名状态 | ✅（未签名） | `codesign --verify` 返回「code has no resources but signature indicates they must be present」→ 首次启动需右键打开/放行 |
| dmg 校验值 | ✅ | sha256 `86d8db02e290777db7596e06a208449494ec232b04149bda5e2f049fc47efb1b`，4,142,417 bytes |
| Windows 安装包格式 | ✅ | `file` 识别为 `PE32 executable (GUI) Intel 80386, for MS Windows, Nullsoft Installer self-extracting archive` |
| Windows 产物校验值 | ✅ | setup.exe sha256 `d75b97e535c7ed972122d7dcd981d005d6e39dc3e4823d2a27b09ba5154ad606`；portable.zip sha256 `ea25ed29e5e7e33692cd2c16ab5956d9a4e96aad6ceaa3aa1b86c3187ed47392` |

## B. 升级与迁移（✅ 已自动验证）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 旧库（迁移 005，无 rowid 映射表）打开后自动迁移到 schema 6 | ✅ | `cargo test --offline --test upgrade_flow` |
| 迁移后搜索索引自动重建（已索引 == 总数） | ✅ | 同一测试断言 `indexed_notes == total_notes` |
| 迁移后检索不退化（原有笔记仍可搜到） | ✅ | 同一测试搜索「倒排索引」命中原笔记 |
| FTS 记录与映射表一一对应 | ✅ | 同一测试断言 `fts_rows == map_rows` |

> ⚠️ 升级提示：迁移 006 会**清空既有 FTS 索引**（旧索引没有 rowid 映射），
> 下次打开知识库时自动重建一次（1 万篇实测约 4 秒）。用户数据（Markdown / 数据库）不受影响。

## C. 性能（✅ 已自动验证，1 万篇合成数据）

| 项目 | 结果 | 证据 |
| --- | --- | --- |
| 首次导入 1 万篇（带文件监听） | ✅ **4.16 s** | `BENCH_NOTES=10000 cargo test --offline --test search_bench -- --ignored --nocapture` |
| 首次导入 1 万篇（停监听，纯采纳） | ✅ **3.96 s** | 同上（`BENCH_STOP_WATCHER=1`） |
| FTS 全量重建 1 万篇 | ✅ 4.2 s | 同上 |
| 检索延迟（含摘要高亮） | ✅ 平均 18.2 ms / p95 44.2 ms | 同上（120 次查询） |
| 召回（纯英文 / 混合中英文 / 中文短语） | ✅ 100% / 100% / 100% | 同上 |
| 库体积（1 万篇） | ✅ 32.3 MB（3.3 KB/篇） | 同上 |

## D. 功能验收（⬜ 需真机，macOS）

> 以下项目需要在图形界面手动操作，我无法在当前环境执行（无图形会话，且应用为 GUI 程序）。

| # | 验收项 | 操作 | 预期 |
| --- | --- | --- | --- |
| D1 | 安装与首次启动 | 双击 dmg → 拖入「应用程序」→ 右键「打开」放行 | 正常启动；「关于」页显示 1.2.0 |
| D2 | 新建知识库 | 首次启动 → 填名称与目录 → 创建 | 生成 `.knowledge`/`00_Inbox`/`attachments`/`.trash` 与默认分类；预置欢迎笔记与示例模板 |
| D3 | 打开 V1.1.x 旧库 | 用旧版本建库并写入若干笔记后，用 1.2.0 打开 | 自动迁移到 schema 6；状态栏索引数在数秒内追平；笔记与标签完整 |
| D4 | 记录与自动保存 | 新建笔记 → 输入 → 等待 1s → 退出重启 | 内容保留；状态栏显示「已保存」 |
| D5 | 图片粘贴 | 在编辑器粘贴截图 | 写入 `attachments/YYYY-MM/`，正文插入相对链接，预览内联显示 |
| D6 | 双向链接 | 输入 `[[` 选标题；预览点击 chip；查看反链面板 | 补全生效、chip 可跳转、反链与未解析链接正确 |
| D7 | 类型化关系 | 关系面板添加 SUPERSEDES / CONTRADICTS | 元信息栏出现「已取代 / 已被取代 / N 处冲突」角标；frontmatter 写入 `relations:` |
| D8 | 来源与时效 | 更多操作 → 来源与时效 → 设有效期至昨天 | 元信息栏出现「可能已过期」；frontmatter 出现 `valid_until` |
| D9 | 图谱 | `/graph` 页查看、切社区着色、调粒度、导出 PNG/SVG | 力导向布局正常；导出文件落在 `attachments/YYYY-MM/` |
| D10 | 批量与拖拽 | 多选移动/打标签/删除；拖动笔记到分类 | 操作生效且列表/分类计数刷新 |
| D11 | 导入 Obsidian 库 | 新建 → 导入外部文件 → 选 vault 目录（自动识别） | 目录结构保留、`![[图片]]` 变标准图片链接、附件落到 `attachments/` |
| D12 | 导入 Notion 导出 | 同上，选 Notion 导出目录 | 文件名与标题去掉哈希后缀；`%20` 链接解码 |
| D13 | 导出 | 单篇导出 md/html/docx/pdf | 文件可打开；PDF 走打印视图另存 |
| D14 | 备份与恢复 | 设置 → 创建备份 → 校验 → 恢复 | 备份包含笔记与附件；校验通过 |
| D15 | 外部编辑联动 | 用其他编辑器改 md 并保存 | 应用自动同步；内容冲突时弹出冲突对话框 |
| D16 | 附件管理 | `/attachments` 查看引用并删除 | 引用笔记可点击跳转；删除后文件消失、引用提示失效风险 |
| D17 | 卸载/清理 | 删除应用；检查知识库目录 | 知识库目录与笔记完整保留（不随应用删除） |

## E. Windows 对照验收（⬜ 需 Windows 真机）

| # | 验收项 | 预期 |
| --- | --- | --- |
| E1 | 安装 `..._1.2.0_x64-setup.exe` | SmartScreen 提示（未签名）后放行；安装到当前用户目录 |
| E2 | 启动与 WebView2 | 正常启动；若缺 WebView2 有明确提示 |
| E3 | 文件读写与中文检索 | 建库、写中文笔记、搜索命中 |
| E4 | 快捷键与导出 | Ctrl+S/K/P 生效；导出 md/docx/pdf 成功 |
| E5 | 卸载 | 卸载后知识库目录保留 |

> ⚠️ Windows 产物由 macOS **交叉编译**（Tauri 官方标注 experimental）且未签名，
> 必须按上表在真实 Windows 上走一遍才能对外发布。

## F. 已知限制（本版）

1. 未做代码签名与公证（macOS 需右键打开，Windows 需放行 SmartScreen）；
2. Windows 安装包为交叉编译产物，未做真机验收；
3. 知识图谱在超过 2000 节点 / 5000 边时按连接度截断并提示筛选；
4. 中文分词策略经 1 万篇基准验证（体积/延迟/召回均达标），本版**不引入外部分词依赖**；
5. 第二代 AI 能力仅有接口骨架（`DecisionModel` / `Retriever` / `GenerativeModel`），不含模型调用。
