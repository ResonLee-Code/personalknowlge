# E 节真机验收（自动跑）

- 开始时间：2026-10-15T00:52:43.668Z
- 结论：0 条通过、0 条失败、9 条待人工（自动步骤 95 步通过 / 0 步失败）
- 自动步骤：**95 步通过 / 0 步失败**（用例含人工确认时整体标「待人工」，机器证据看这里与每条的明细）
- 工作区：`/private/tmp/pkos-acceptance`
- 应用：Personal Knowledge OS
- 模式：real（真机执行）
- 截图：同名 PNG 就在本目录（**默认不进仓库**，见 `scripts/acceptance/README.md`；要留证请 `git add -f`）

## 现场准备（自动）

> 每轮真跑都会先做这一步：把应用切到前台，并确认**目标工作区**已打开。
> 现场不对就不出结论表——不能把「现场没准备好」写成「功能有问题」。

- [✅ 通过] `activate` 已把应用切到前台
- [✅ 通过] `cleanup` 按 Esc 清掉可能残留的弹层
- [✅ 通过] `scene` 应用已在主界面：直接开始用例（若当前打开的不是目标工作区，文件断言会如实报错）

## 结论表

| # | 项目 | 结果 | 证据 |
| --- | --- | --- | --- |
| 1 | 编辑器输入、[[ 补全、原生下拉与日期控件（valid_until） | 🖐 待人工 | 截图 `01-editor-input.png`；✓ 00_Inbox/20261015-084627-ACCEPT-NOTE-A.md（刚生成） |
| 2 | 导入对话框与导出（md / html / docx / pdf） | 🖐 待人工 | 截图 `02-export-dialog.png`；✓ 00_Inbox 存在 |
| 3 | 备份创建 / 校验 / 恢复 | 🖐 待人工 | ✓ .knowledge/backups/KnowledgeBackup_20261015-084646.zip（刚生成）；✓ 界面存在「备份校验：清单文件存在」；✓ 界面存在「备份校验：Markdown 数量一致」；截图 `03-backup-created.png`；截图 `03b-backup-restored.png` |
| 4 | 大纲栏位切换、分栏拖拽与大纲三态的手感 | 🖐 待人工 | ✓ 「设置 → 编辑器 → 大纲位置」= 与笔记列表共用一栏；✓ 界面存在「这一栏先显示笔记列表」；✓ 界面存在「大纲显示在笔记列表那一栏（栏头有「返回笔记列表」）」；✓ 界面存在「列表栏里的大纲」；✓ 界面存在「「钉在正文右侧」入口」；✓ 界面不存在「此时正文右侧没有常驻大纲」；截图 `04-outline-in-list.png`；✓ 界面存在「正文右侧的大纲（面板里有「收起大纲」）」；✓ 界面不存在「大纲已离开列表栏」；截图 `04-outline-side.png`；✓ 界面不存在「收起后正文右侧没有常驻面板」；✓ 界面不存在「收起后也没有展开的悬浮大纲」；✓ 界面存在「贴边「大纲」条（竖排）」；截图 `04-outline-mini.png`；✓ 界面存在「悬停后展开的悬浮大纲（有「钉住大纲」）」；截图 `04-outline-hover.png`；✓ 界面存在「钉住后回到常驻态」；✓ 界面存在「大纲已从正文右侧挪回列表那一栏」；✓ 界面不存在「正文右侧已不再常驻大纲」；✓ 界面存在「挪回来之后，栏头仍有「返回笔记列表」」；截图 `04-outline-split.png` |
| 5 | 版本历史：命名、还原、对比 | 🖐 待人工 | ✓ .knowledge/history 存在；截图 `05-history-dialog.png` |
| 6 | 网页剪藏（真实网络抓取公开网页） | 🖐 待人工 | 截图 `06-web-clip.png`；✓ 00_Inbox/20261015-085213-Example-Domain.md（刚生成） |
| 7 | 研究助手：本地调研全链路 | 🖐 待人工 | 截图 `07-research-plan.png`；✓ .knowledge/research 不存在（符合预期） |
| 8 | 研究助手：联网检索 | 🖐 待人工 | 截图 `08-research-web.png` |
| 9 | 首页「待复核」卡片点击跳转与时效角标 | 🖐 待人工 | ✓ 界面存在「首页的「待复核」卡片」；✓ 界面存在「时效角标「已过期」」；截图 `09-review-due.png` |
| 10 | 首次启动放行（Gatekeeper） | 🖐 人工 | 只对「从网络下载的包」有意义；本机构建无 quarantine，脚本无法覆盖 |

## 逐条明细

### 1. 编辑器输入、[[ 补全、原生下拉与日期控件（valid_until）（🖐 待人工）

- 期望：输入落盘、⌘S 立即写盘、[[ 弹出候选、日期控件弹出系统日历
- [✅ 通过] `app` app=Personal Knowledge OS pid=31293
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(152,200) · AXPress
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `click` click(1004,725) · CGEvent（没找到元素，按坐标点）
- [✅ 通过] `keys` cmd+e
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `keys` cmd+s
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `check` 文件断言通过
- [🖐 待人工] `manual` [[ 补全候选的内容与顺序、原生日期控件弹出系统日历，需人眼确认

### 2. 导入对话框与导出（md / html / docx / pdf）（🖐 待人工）

- 期望：三种导出各得合法文件，导入 .docx 能回读正文
- [✅ 通过] `app` app=Personal Knowledge OS pid=31293
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(1418,89) · AXPress
- [✅ 通过] `shot` 截图已存档
- [🖐 待人工] `manual` 导出走 macOS 原生保存面板：需要人眼选目录并确认三种格式的文件可打开；导入 .docx 需人眼核对正文
- [✅ 通过] `check` 文件断言通过

### 3. 备份创建 / 校验 / 恢复（🖐 待人工）

- 期望：创建出 zip、校验逐项通过、恢复到空目录得到可用知识库
- [✅ 通过] `app` app=Personal Knowledge OS pid=31293
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(1418,89) · AXPress
- [✅ 通过] `click` click(789,867) · AXPress
- [✅ 通过] `check` 文件断言通过
- [✅ 通过] `click` click(318,1014) · AXPress
- [✅ 通过] `pause` 等待 2500 ms（等界面自己完成）
- [✅ 通过] `expectElement` 界面上找到了：备份校验：清单文件存在
- [✅ 通过] `expectElement` 界面上找到了：备份校验：Markdown 数量一致
- [✅ 通过] `shot` 截图已存档
- [🖐 待人工] `manual` 「恢复」需要选目录（原生面板）：由人选一个空目录，确认恢复出来的知识库能打开
- [✅ 通过] `shot` 截图已存档

### 4. 大纲栏位切换、分栏拖拽与大纲三态的手感（🖐 待人工）

- 期望：默认与列表共用中栏（栏头有「返回笔记列表」）→ 挪到正文右侧（出现「收起大纲」）→ 挪回列表栏；拖分割线比例变化并落库
- [✅ 通过] `app` app=Personal Knowledge OS pid=31293
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(152,614) · AXPress
- [✅ 通过] `selectOption` 本来就是「与笔记列表共用一栏」，未改动
- [✅ 通过] `keys` cmd+p
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `keyCode` keyCode(36)
- [✅ 通过] `pause` 等待 600 ms（等界面自己完成）
- [✅ 通过] `ensure` 按了 1 次「返回笔记列表」达到目标状态（这一栏先显示笔记列表）
- [✅ 通过] `ensure` 按了 1 次「工具栏大纲开关」达到目标状态（大纲显示在笔记列表那一栏（栏头有「返回笔记列表」））
- [✅ 通过] `expectElement` 界面上找到了：列表栏里的大纲
- [✅ 通过] `expectElement` 界面上找到了：「钉在正文右侧」入口
- [✅ 通过] `expectAbsent` 界面上确实没有：此时正文右侧没有常驻大纲
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `ensure` 按了 1 次「钉在正文右侧」达到目标状态（正文右侧的大纲（面板里有「收起大纲」））
- [✅ 通过] `expectAbsent` 界面上确实没有：大纲已离开列表栏
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `click` click(1425,324) · AXPress
- [✅ 通过] `expectAbsent` 界面上确实没有：收起后正文右侧没有常驻面板
- [✅ 通过] `expectAbsent` 界面上确实没有：收起后也没有展开的悬浮大纲
- [✅ 通过] `expectElement` 界面上找到了：贴边「大纲」条（竖排）
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `hover` hover(1440,516)
- [✅ 通过] `expectElement` 界面上找到了：悬停后展开的悬浮大纲（有「钉住大纲」）
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `click` click(1425,324) · AXPress
- [✅ 通过] `expectElement` 界面上找到了：钉住后回到常驻态
- [✅ 通过] `ensure` 按了 1 次「改到笔记列表那一栏」达到目标状态（大纲已从正文右侧挪回列表那一栏）
- [✅ 通过] `expectAbsent` 界面上确实没有：正文右侧已不再常驻大纲
- [✅ 通过] `expectElement` 界面上找到了：挪回来之后，栏头仍有「返回笔记列表」
- [✅ 通过] `drag` drag(811,489) → (871,489)
- [✅ 通过] `shot` 截图已存档
- [🖐 待人工] `manual` 只剩主观手感需要人眼：拖拽分栏跟不跟手、贴边条悬停展开的动效观感，必须人手拖两下 / 移两下

### 5. 版本历史：命名、还原、对比（🖐 待人工）

- 期望：保存后生成 history 快照、对比面板显示增删、还原后正文回滚
- [✅ 通过] `app` app=Personal Knowledge OS pid=31293
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `keys` cmd+p
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `keyCode` keyCode(36)
- [✅ 通过] `keys` cmd+s
- [✅ 通过] `check` 文件断言通过
- [✅ 通过] `shot` 截图已存档
- [🖐 待人工] `manual` 「更多操作 → 历史版本…」的命名 / 还原 / 对比需要人眼看：菜单弹层与 diff 内容无法用 AX 断言

### 6. 网页剪藏（真实网络抓取公开网页）（🖐 待人工）

- 期望：抓 https://example.com 生成笔记：标题 / source_type / source_ref / 正文
- [✅ 通过] `app` app=Personal Knowledge OS pid=31293
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(152,200) · AXPress
- [✅ 通过] `click` click(948,296) · AXPress
- [✅ 通过] `click` click(719,523) · AXPress
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `click` click(999,523) · CGEvent（没找到元素，按坐标点）
- [✅ 通过] `pause` 等待 8000 ms（等界面自己完成）
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `click` click(980,710) · AXPress
- [✅ 通过] `pause` 等待 2500 ms（等界面自己完成）
- [✅ 通过] `check` 文件断言通过
- [🖐 待人工] `manual` 机器已验「抓取 → 生成笔记 → 落盘（含 source_ref）」；正文抽取的完整度与元数据展示需人眼核对一次

### 7. 研究助手：本地调研全链路（🖐 待人工）

- 期望：五步走通并确认写入；未开 AI 时被闸门挡住
- [✅ 通过] `app` app=Personal Knowledge OS pid=31293
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(152,410) · AXPress
- [✅ 通过] `click` click(431,379) · AXPress
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `click` click(431,499) · CGEvent（没找到元素，按坐标点）
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `check` 文件断言通过
- [🖐 待人工] `manual` 验收库的 AI 总开关默认关闭：本条机器断言的是**闸门挡住**（没有会话落盘）。打开 AI 后走完五步并确认写入，需要人眼确认报告六段与写入 diff

### 8. 研究助手：联网检索（🖐 待人工）

- 期望：配置检索源 → 允许联网 → 首次确认 → 收集到本地 + 外部来源，注入页被丢弃
- [✅ 通过] `app` app=Personal Knowledge OS pid=31293
- [✅ 通过] `keyCode` keyCode(53)
- [🖐 待人工] `manual` 需要先起本机桩检索服务（scripts/research/）并在界面配置检索源、打开联网档位并逐次确认
- [✅ 通过] `shot` 截图已存档

### 9. 首页「待复核」卡片点击跳转与时效角标（🖐 待人工）

- 期望：写一篇 valid_until 已过期的笔记 → 首页出现待复核卡片 → 点击跳转
- [✅ 通过] `app` app=Personal Knowledge OS pid=31293
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(152,240) · AXPress
- [✅ 通过] `expectElement` 界面上找到了：首页的「待复核」卡片
- [✅ 通过] `expectElement` 界面上找到了：时效角标「已过期」
- [✅ 通过] `shot` 截图已存档
- [🖐 待人工] `manual` 角标文案与跳转后的详情行（⚠ 可能已过期 / ✓ 已索引）需人眼确认
