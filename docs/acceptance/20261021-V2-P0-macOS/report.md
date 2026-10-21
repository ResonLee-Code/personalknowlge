# E 节真机验收（自动跑）

- 开始时间：2026-10-21T16:16:10.618Z
- 结论：0 条通过、0 条失败、5 条待人工（自动步骤 111 步通过 / 0 步失败）
- 自动步骤：**111 步通过 / 0 步失败**（用例含人工确认时整体标「待人工」，机器证据看这里与每条的明细）
- 工作区：`/private/tmp/pkos-app-scale-smoke-65888`
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
| 1 | 编辑器输入、[[ 补全、原生下拉与日期控件（valid_until） | 🖐 待人工 | 截图 `01-editor-input.png`；✓ 00_Inbox/20261022-001128-ACCEPT-NOTE-A.md（刚生成） |
| 6 | 网页剪藏（真实网络抓取公开网页） | 🖐 待人工 | 截图 `06-web-clip.png`；✓ 00_Inbox/20261022-001150-Example-Domain.md（刚生成） |
| 7 | 研究助手：本地调研全链路 | 🖐 待人工 | 截图 `07-research-plan.png`；✓ 界面存在「研究入口显示 AI 总开关闸门错误」 |
| 11 | 界面梳理：侧边栏开关归位、区块折叠、条目行不溢出列表栏 | 🖐 待人工 | ✓ 「设置 → 编辑器 → 大纲位置」= 与笔记列表共用一栏；✓ 界面不存在「顶栏的「切换侧边栏」已删除」；✓ 界面存在「侧边栏先展开」；✓ 界面存在「收起后底部的「展开侧边栏」」；✓ 界面存在「收起态图标栏里的「首页」」；✓ 界面存在「收起态图标栏里的「回收站」」；✓ 界面存在「点图标栏的「首页」真的到了首页」；✓ 界面存在「这一栏先显示笔记列表」；截图 `11-sidebar-collapsed.png`；✓ 界面存在「展开回来了（开关还在原处）」；✓ 界面不存在「折叠后分类条目不在了」；✓ 界面存在「展开后分类条目回来了」；✓ 界面不存在「折叠后标签条目不在了」；截图 `11-sections.png`；✓ 界面存在「看条目行前：这一栏是列表」；✓ 长标题条目行仍在列表栏内（没有溢出）；截图 `11-long-title-row.png`；✓ 界面存在「长标题下大纲栏头仍在（有「返回笔记列表」）」；✓ 栏头里那颗返回按钮还是一行高；截图 `11-outline-header-long-title.png` |
| 12 | 编辑器新能力：图形渲染、插入菜单、大纲折叠 | 🖐 待人工 | ✓ 「设置 → 编辑器 → 大纲位置」= 与笔记列表共用一栏；✓ 界面存在「编辑器工具栏最左边的「＋ 插入内容」」；✓ 界面存在「预览里的流程图（节点文字「判断」）」；截图 `12-mermaid-preview.png`；✓ 界面存在「这一栏显示大纲」；✓ 界面存在「大纲里有「子标题一」」；✓ 界面不存在「收起后子标题不再显示」；截图 `12-outline-fold.png`；✓ 界面存在「展开后子标题回来了」 |
| 10 | 首次启动放行（Gatekeeper） | 🖐 人工 | 只对「从网络下载的包」有意义；本机构建无 quarantine，脚本无法覆盖 |

## 逐条明细

### 1. 编辑器输入、[[ 补全、原生下拉与日期控件（valid_until）（🖐 待人工）

- 期望：输入落盘、⌘S 立即写盘、[[ 弹出候选、日期控件弹出系统日历
- [✅ 通过] `app` app=Personal Knowledge OS pid=75781
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(182,200) · AXPress
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `click` click(1004,769) · AXPress
- [✅ 通过] `keys` cmd+e
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `keys` cmd+s
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `check` 文件断言通过
- [🖐 待人工] `manual` [[ 补全候选的内容与顺序、原生日期控件弹出系统日历，需人眼确认

### 6. 网页剪藏（真实网络抓取公开网页）（🖐 待人工）

- 期望：抓 https://example.com 生成笔记：标题 / source_type / source_ref / 正文
- [✅ 通过] `app` app=Personal Knowledge OS pid=75781
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(182,200) · AXPress
- [✅ 通过] `click` click(948,252) · AXPress
- [✅ 通过] `click` click(719,523) · AXPress
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `click` click(999,523) · AXPress
- [✅ 通过] `pause` 等待 8000 ms（等界面自己完成）
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `click` click(980,710) · AXPress
- [✅ 通过] `pause` 等待 2500 ms（等界面自己完成）
- [✅ 通过] `check` 文件断言通过
- [🖐 待人工] `manual` 机器已验「抓取 → 生成笔记 → 落盘（含 source_ref）」；正文抽取的完整度与元数据展示需人眼核对一次

### 7. 研究助手：本地调研全链路（🖐 待人工）

- 期望：五步走通并确认写入；未开 AI 时被闸门挡住
- [✅ 通过] `app` app=Personal Knowledge OS pid=75781
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(182,410) · AXPress
- [✅ 通过] `click` click(491,379) · AXPress
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `click` click(491,571) · AXPress
- [✅ 通过] `pause` 等待 800 ms（等界面自己完成）
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `expectElement` 界面上找到了：研究入口显示 AI 总开关闸门错误
- [🖐 待人工] `manual` 验收库的 AI 总开关默认关闭：本条机器断言的是**闸门挡住**（没有会话落盘）。打开 AI 后走完五步并确认写入，需要人眼确认报告六段与写入 diff

### 11. 界面梳理：侧边栏开关归位、区块折叠、条目行不溢出列表栏（🖐 待人工）

- 期望：顶栏没有重复的侧边栏开关；收起后「展开侧边栏」与展开态「收起侧边栏」同一高度；分类 / 标签可折叠；长标题行的按钮仍在列表栏内
- [✅ 通过] `app` app=Personal Knowledge OS pid=75781
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(1418,89) · AXPress
- [✅ 通过] `pause` 等待 900 ms（等界面自己完成）
- [✅ 通过] `click` click(419,277) · AXPress
- [✅ 通过] `selectOption` 本来就是「与笔记列表共用一栏」，未改动
- [✅ 通过] `click` click(182,274) · AXPress
- [✅ 通过] `pause` 等待 1000 ms（等界面自己完成）
- [✅ 通过] `expectAbsent` 界面上确实没有：顶栏的「切换侧边栏」已删除
- [✅ 通过] `ensure` 界面已在目标状态（侧边栏先展开），未按控件
- [✅ 通过] `click` click(182,274) · AXPress
- [✅ 通过] `pause` 等待 1200 ms（等界面自己完成）
- [✅ 通过] `click` click(274,842) · AXPress
- [✅ 通过] `expectElement` 界面上找到了：收起后底部的「展开侧边栏」
- [✅ 通过] `expectElement` 界面上找到了：收起态图标栏里的「首页」
- [✅ 通过] `expectElement` 界面上找到了：收起态图标栏里的「回收站」
- [✅ 通过] `click` click(88,181) · AXPress
- [✅ 通过] `expectElement` 界面上找到了：点图标栏的「首页」真的到了首页
- [✅ 通过] `click` click(88,215) · AXPress
- [✅ 通过] `ensure` 界面已在目标状态（这一栏先显示笔记列表），未按控件
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `click` click(88,842) · AXPress
- [✅ 通过] `expectElement` 界面上找到了：展开回来了（开关还在原处）
- [✅ 通过] `click` click(101,654) · AXPress
- [✅ 通过] `expectAbsent` 界面上确实没有：折叠后分类条目不在了
- [✅ 通过] `click` click(101,654) · AXPress
- [✅ 通过] `expectElement` 界面上找到了：展开后分类条目回来了
- [✅ 通过] `click` click(101,1082) · AXPress
- [✅ 通过] `expectAbsent` 界面上确实没有：折叠后标签条目不在了
- [✅ 通过] `click` click(101,792) · AXPress
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `click` click(482,211) · AXPress
- [✅ 通过] `pause` 等待 800 ms（等界面自己完成）
- [✅ 通过] `click` click(182,200) · AXPress
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `click` click(1004,769) · AXPress
- [✅ 通过] `pause` 等待 1500 ms（等界面自己完成）
- [✅ 通过] `drag` drag(551,489) → (471,489)
- [✅ 通过] `pause` 等待 600 ms（等界面自己完成）
- [✅ 通过] `ensure` 按了 1 次「返回笔记列表」达到目标状态（看条目行前：这一栏是列表）
- [✅ 通过] `expectInside` 内框 x=313..540，外框 x=317..536（在范围内）
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `click` click(522,138) · AXPress
- [✅ 通过] `pause` 等待 900 ms（等界面自己完成）
- [✅ 通过] `expectElement` 界面上找到了：长标题下大纲栏头仍在（有「返回笔记列表」）
- [✅ 通过] `expectSize` 实测 18×18（在范围内）
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `click` click(328,131) · AXPress
- [✅ 通过] `pause` 等待 600 ms（等界面自己完成）
- [🖐 待人工] `manual` 截断时右侧那两个按钮「是不是常显、间距顺不顺眼」要看截图/上手看一眼（AX 读不到透明度）

### 12. 编辑器新能力：图形渲染、插入菜单、大纲折叠（🖐 待人工）

- 期望：插入菜单在编辑 / 分屏可用；```mermaid 流程图在预览里真的渲染出图（WebKit 上验）；大纲能按节折叠
- [✅ 通过] `app` app=Personal Knowledge OS pid=75781
- [✅ 通过] `keyCode` keyCode(53)
- [✅ 通过] `click` click(1418,89) · AXPress
- [✅ 通过] `pause` 等待 900 ms（等界面自己完成）
- [✅ 通过] `click` click(419,277) · AXPress
- [✅ 通过] `selectOption` 本来就是「与笔记列表共用一栏」，未改动
- [✅ 通过] `click` click(182,-16) · AXPress
- [✅ 通过] `pause` 等待 1000 ms（等界面自己完成）
- [✅ 通过] `click` click(182,200) · AXPress
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `click` click(1004,769) · AXPress
- [✅ 通过] `pause` 等待 1800 ms（等界面自己完成）
- [✅ 通过] `expectElement` 界面上找到了：编辑器工具栏最左边的「＋ 插入内容」
- [✅ 通过] `keys` cmd+e
- [✅ 通过] `paste` 粘贴了一小段文本（不落进报告）
- [✅ 通过] `keys` cmd+s
- [✅ 通过] `pause` 等待 1200 ms（等界面自己完成）
- [✅ 通过] `click` click(755,138) · AXPress
- [✅ 通过] `pause` 等待 2500 ms（等界面自己完成）
- [✅ 通过] `expectElement` 界面上找到了：预览里的流程图（节点文字「判断」）
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `click` click(695,138) · AXPress
- [✅ 通过] `pause` 等待 800 ms（等界面自己完成）
- [✅ 通过] `ensure` 按了 1 次「工具栏大纲开关」达到目标状态（这一栏显示大纲）
- [✅ 通过] `pause` 等待 900 ms（等界面自己完成）
- [✅ 通过] `expectElement` 界面上找到了：大纲里有「子标题一」
- [✅ 通过] `click` click(323,160) · AXPress
- [✅ 通过] `expectAbsent` 界面上确实没有：收起后子标题不再显示
- [✅ 通过] `shot` 截图已存档
- [✅ 通过] `click` click(323,160) · AXPress
- [✅ 通过] `expectElement` 界面上找到了：展开后子标题回来了
- [🖐 待人工] `manual` 图形的**观感**（连线是否清楚、深色主题下配色是否舒服）需要人眼看一下截图
