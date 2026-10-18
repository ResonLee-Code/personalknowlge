'use strict';

/**
 * E 节真机验收的用例定义（Track E1）——**唯一事实来源**。
 *
 * 步骤里能自动化的都自动化；**只能人眼判断的**一律 `manual`：
 * - 主观手感（拖拽跟不跟手）、原生面板（导出 / 选目录）、真实网络请求、
 *   以及"候选内容对不对"这类需要理解语义的观察；
 * - `manual` 不是偷懒，是**不替人下结论**（脚本说"通过"必须有机器证据）。
 *
 * ## 元素怎么找（2026-10-14 首次真跑后按真实 AX 树改写）
 *
 * 原版用例是照着"记忆里的界面"写的，首次真跑 6 条失败，绝大多数是**名字对不上**：
 * 侧边栏导航在 AX 里是 `AXLink`（不是 AXButton）、大纲按钮叫「收起大纲 / 显示大纲」、
 * 「更多操作」是 `AXPopUpButton`……这里已按实际树改好。
 *
 * 可用的驱动手段（都实测过）：
 * - `click`：优先 AXPress（对按钮 / 链接 / **输入框**都有效，能把焦点给输入框）；
 * - `paste`：走剪贴板 + ⌘V。**中文必须用它**——`type` 送中文会进系统输入法候选窗；
 * - `keys`：应用级快捷键（⌘N 新建、⌘P 快速切换、⌘E 聚焦编辑器、⌘S 保存、⌘K 全局搜索）；
 * - `check`：文件断言（落盘 / 备份 / history / 剪藏真的抓回来了）；
 * - `expectElement` / `expectAbsent`：**界面上真的有 / 真的没有**这个元素（成对用才叫断言）；
 * - `ensure`：**把界面推到目标状态**——目标在就什么都不做，不在就按一次给定控件再查，
 *   按了几次如实写进报告。为什么需要它见下面第 2 条实测教训。
 * - `hover`：真实鼠标移动（不用 AX）——给"鼠标移上去才展开"的控件用（贴边 mini 态）；
 * - `selectOption`：改**原生下拉**的值（设置页那些 `<select>`），改完读 `value` 复核。
 *
 * ⚠️ 两条"照记忆写用例"的实测教训（2026-10-15 第二轮真跑）：
 * 1. **role 要对**：带 `aria-pressed` 的按钮在 WebKit 里是 `AXCheckBox`（可读名字在
 *    `description`），不是 `AXButton`/`title`。查询写到"名字包含某子串"就会**命中别的元素**——
 *    旧用例的 `{ role: 'AXButton', title: '大纲' }` 一直命中的是右侧面板的「收起大纲」。
 * 2. **按下去 ≠ 界面会动**：导航刚发生时无障碍树里可能还留着上一页的同名元素，
 *    AXPress 返回成功、React 节点却已卸载，点击被静默丢弃。所以"切状态"的步骤要用 `ensure`
 *    （以界面状态为准、失败就再按一次），而不是 `click` + 断言。
 */

/** 应用名（与 Info.plist 的 CFBundleName 一致）。 */
const APP_NAME = 'Personal Knowledge OS';

/** 验收用的固定标题：每次跑都会新建一篇，便于断言。 */
const NOTE_TITLE = 'ACCEPT-NOTE-A';

/**
 * 把「大纲位置」摆回默认（与笔记列表共用一栏）。
 *
 * 为什么几条用例都要先跑它：`大纲位置` 是**工作区级偏好**，会被上一轮残留改掉；
 * 位置在「正文右侧」时，列表栏头部那个开关切的是**右侧那一列的显隐**、不是栏位切换——
 * 于是用例里"点开关 → 这一栏换成大纲"的断言会全错（2026-10-17 实测：一次失败把位置留在
 * side，后面两条用例跟着假失败）。用设置页改，改完 selectOption 会隔 2.5 秒复核一次。
 */
const ENSURE_OUTLINE_PLACEMENT_LIST = [
  { op: 'click', frame: { role: 'AXButton', description: '设置' } },
  { op: 'pause', ms: 900 },
  { op: 'click', frame: { role: 'AXButton', title: '编辑器' } },
  {
    op: 'selectOption',
    popup: { role: 'AXPopUpButton', description: '大纲位置' },
    option: '与笔记列表共用一栏',
    options: ['与笔记列表共用一栏', '钉在正文右侧'],
    label: '设置 → 编辑器 → 大纲位置',
  },
];

const CASES = [
  {
    id: 1,
    title: '编辑器输入、[[ 补全、原生下拉与日期控件（valid_until）',
    expect: '输入落盘、⌘S 立即写盘、[[ 弹出候选、日期控件弹出系统日历',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      // 用「点按钮」而不是 ⌘N：实测 ⌘N 在个别焦点状态下会被吞掉（点击走 AXPress 更稳）
      { op: 'click', frame: { role: 'AXButton', title: '新建笔记' } },
      { op: 'paste', text: NOTE_TITLE },
      { op: 'click', frame: { role: 'AXButton', title: '创建笔记' } },
      { op: 'keys', combo: 'cmd+e' },
      { op: 'paste', text: '验收：输入应当落盘。' },
      { op: 'keys', combo: 'cmd+s' },
      { op: 'shot', name: '01-editor-input.png' },
      {
        op: 'check',
        path: '00_Inbox',
        containsFile: NOTE_TITLE,
        grep: '验收：输入应当落盘。',
        // 必须断言**这一轮刚写的那篇**：只查"目录里有同名文件"会让上次跑的残留也过
        modifiedWithinMinutes: 2,
      },
      {
        op: 'manual',
        note: '[[ 补全候选的内容与顺序、原生日期控件弹出系统日历，需人眼确认',
      },
    ],
  },
  {
    id: 2,
    title: '导入对话框与导出（md / html / docx / pdf）',
    expect: '三种导出各得合法文件，导入 .docx 能回读正文',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      // 设置入口只有右上角那一个（2026-10-15 起侧边栏不再重复放一个）
      { op: 'click', frame: { role: 'AXButton', description: '设置' } },
      { op: 'shot', name: '02-export-dialog.png' },
      {
        op: 'manual',
        note: '导出走 macOS 原生保存面板：需要人眼选目录并确认三种格式的文件可打开；导入 .docx 需人眼核对正文',
      },
      { op: 'check', path: '00_Inbox', exists: true },
    ],
  },
  {
    id: 3,
    title: '备份创建 / 校验 / 恢复',
    expect: '创建出 zip、校验逐项通过、恢复到空目录得到可用知识库',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      { op: 'click', frame: { role: 'AXButton', description: '设置' } },
      // 设置页自 2026-10-16 起分节显示（左侧导航）：备份相关的行在「备份与导出」这一节里
      { op: 'pause', ms: 900 },
      { op: 'click', frame: { role: 'AXButton', title: '备份与导出' } },
      { op: 'click', frame: { role: 'AXButton', title: '立即备份' } },
      {
        op: 'check',
        path: '.knowledge/backups',
        containsFile: 'KnowledgeBackup_',
        modifiedWithinMinutes: 2,
      },
      // 校验是**就地**出结果的（不碰原生面板）→ 能机器断言；
      // 「恢复」要选目标目录（原生面板）→ 留给下面那步人工。
      { op: 'click', frame: { role: 'AXButton', title: '校验' } },
      { op: 'pause', ms: 2500 },
      { op: 'expectElement', query: { title: '清单文件存在' }, label: '备份校验：清单文件存在' },
      { op: 'expectElement', query: { title: 'Markdown 数量一致' }, label: '备份校验：Markdown 数量一致' },
      { op: 'shot', name: '03-backup-created.png' },
      {
        op: 'manual',
        note: '「恢复」需要选目录（原生面板）：由人选一个空目录，确认恢复出来的知识库能打开',
      },
      { op: 'shot', name: '03b-backup-restored.png' },
    ],
  },
  {
    id: 4,
    title: '大纲栏位切换、分栏拖拽与大纲三态的手感',
    expect:
      '默认与列表共用中栏（栏头有「返回笔记列表」）→ 挪到正文右侧（出现「收起大纲」）→ 挪回列表栏；拖分割线比例变化并落库',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      /**
       * 0. 先把**前提**摆好：`大纲位置` 是**工作区级偏好**，会被上一轮残留（甚至排查脚本）改掉。
       *    与其假定"它一定是默认值"，不如用设置页把它设回默认——顺带也真机验了设置页这一行。
       *    （"打开笔记时这一栏默认是列表"这条由浏览器预览的 E2E 从干净状态断言，见 `e2e/app.spec.ts` 步骤 18。）
       */
      ...ENSURE_OUTLINE_PLACEMENT_LIST,
      { op: 'keys', combo: 'cmd+p' },
      { op: 'paste', text: '目标笔记' },
      { op: 'keyCode', code: 36 },
      { op: 'pause', ms: 600 },
      /**
       * V1.3.0 布局调整后的三处「位置」都要有机器证据，不能只看截图：
       * - 「返回笔记列表」只存在于**列表栏里的大纲** → 它出现 = 大纲占着列表那一栏；
       * - 「改到笔记列表那一栏」只存在于**正文右侧的大纲** → 它出现 = 大纲已挪到右边；
       * 两个方向各点一次，证明位置不是"挪走就找不回来"。
       *
       * ⚠️ 两条实测教训（2026-10-15）：
       * 1. 查询要**精确到 role**：工具栏那个大纲按钮带 `aria-pressed`，WebKit 把它暴露成
       *    **AXCheckBox**，可读名字在 `description` 上（`AXCheckBox | | 显示大纲 | 0`），
       *    不是 AXButton/title。旧用例写的 `{ role: 'AXButton', title: '大纲' }` 之所以"通过"，
       *    是因为它按子串命中了**右侧面板的「收起大纲」**——点的是一个真实元素，
       *    但不是用例想点的那个（结论：**"步骤通过"不等于"点的就是它"**）。
       * 2. 导航刚发生时，无障碍树里可能还留着**上一页**的同名元素；这时 AXPress 返回成功、
       *    界面却纹丝不动（React 节点已卸载）。所以位置切换用 `ensure`：**以界面状态为准**，
       *    按了几次如实写进报告（第 2 次才成功也是证据，不是被抹掉的抖动）。
       */
      // 0. 归零：这一栏显示什么只是内存态，上一次跑的残留不该影响这一条的起点
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '多选' },
        press: { role: 'AXButton', description: '返回笔记列表' },
        pressLabel: '返回笔记列表',
        label: '这一栏先显示笔记列表',
      },
      // 2. 打开大纲：同一栏换成大纲（不是再挤出一列）
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '返回笔记列表' },
        press: { role: 'AXCheckBox', description: '大纲' },
        pressLabel: '工具栏大纲开关',
        label: '大纲显示在笔记列表那一栏（栏头有「返回笔记列表」）',
      },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '返回笔记列表' },
        label: '列表栏里的大纲',
      },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '钉在正文右侧' },
        label: '「钉在正文右侧」入口',
      },
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '收起大纲' },
        label: '此时正文右侧没有常驻大纲',
      },
      { op: 'shot', name: '04-outline-in-list.png' },
      // 每段动作前**先确认前提**（ensure 是幂等的：已经在目标状态就什么都不做）——
      // 2026-10-15 实测：跑验收时若有人动了电脑（前台被抢），这一栏的状态可能被重置，
      // 后面那段的按钮就"找不到"了；重确认一次比后面整段失败划算。
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '返回笔记列表' },
        press: { role: 'AXCheckBox', description: '大纲' },
        pressLabel: '工具栏大纲开关',
        label: '动作前再确认一次：大纲在列表栏里',
      },
      // 3. 挪到正文右侧：列表栏还给列表，大纲变成正文右边独立一列
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '返回笔记列表' },
        press: { role: 'AXCheckBox', description: '大纲' },
        pressLabel: '工具栏大纲开关',
        label: '挪到右侧前：这一栏确实显示着大纲',
      },
      // 普通点击挪位置（点不到就如实失败）：按钮在**列表栏里的大纲**栏头
      { op: 'click', frame: { role: 'AXButton', description: '钉在正文右侧' } },
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '收起大纲' },
        press: { role: 'AXCheckBox', description: '大纲' },
        pressLabel: '工具栏大纲开关',
        label: '正文右侧的大纲（面板里有「收起大纲」）',
      },
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '返回笔记列表' },
        label: '大纲已离开列表栏',
      },
      { op: 'shot', name: '04-outline-side.png' },
      // 3.5 贴边 mini 态与悬停展开：**悬停不是 AX 动作**（以前这条只能标「待人工」），
      //     改用 `hover`（真实鼠标移动）驱动——实测能让 WebKit 派发 mouseover 并展开悬浮大纲。
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '收起大纲' },
        // 用**列表栏头部那个开关**（不是「钉在正文右侧」）：面板若被藏起来（位置在右侧 +
        // outlineMode=hidden），只有它能重新显示这一列；位置在列表栏时它什么都不用做
        press: { role: 'AXCheckBox', description: '大纲' },
        pressLabel: '工具栏大纲开关',
        label: '动作前再确认一次：正文右侧有常驻大纲',
      },
      { op: 'click', frame: { role: 'AXButton', description: '收起大纲' } },
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '收起大纲' },
        label: '收起后正文右侧没有常驻面板',
      },
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '钉住大纲' },
        label: '收起后也没有展开的悬浮大纲',
      },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '鼠标移上去展开' },
        label: '贴边「大纲」条（竖排）',
      },
      { op: 'shot', name: '04-outline-mini.png' },
      { op: 'hover', target: { role: 'AXButton', description: '鼠标移上去展开' } },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '钉住大纲' },
        label: '悬停后展开的悬浮大纲（有「钉住大纲」）',
      },
      { op: 'shot', name: '04-outline-hover.png' },
      { op: 'click', frame: { role: 'AXButton', description: '钉住大纲' } },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '收起大纲' },
        label: '钉住后回到常驻态',
      },
      // 4. 再挪回列表栏（两个方向都有入口，不会"挪走找不回来"）
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '收起大纲' },
        press: { role: 'AXCheckBox', description: '大纲' },
        pressLabel: '工具栏大纲开关',
        label: '挪回列表栏前：正文右侧有常驻大纲',
      },
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '返回笔记列表' },
        press: { role: 'AXButton', description: '改到笔记列表那一栏' },
        pressLabel: '改到笔记列表那一栏',
        label: '大纲已从正文右侧挪回列表那一栏',
      },
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '收起大纲' },
        label: '正文右侧已不再常驻大纲',
      },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '返回笔记列表' },
        label: '挪回来之后，栏头仍有「返回笔记列表」',
      },
      { op: 'drag', from: { role: 'AXSplitter', title: '调整笔记列表宽度' }, to: { dx: 60, dy: 0 } },
      { op: 'shot', name: '04-outline-split.png' },
      {
        op: 'manual',
        note: '只剩主观手感需要人眼：拖拽分栏跟不跟手、贴边条悬停展开的动效观感，必须人手拖两下 / 移两下',
      },
    ],
  },
  {
    id: 5,
    title: '版本历史：命名、还原、对比',
    expect: '保存后生成 history 快照、对比面板显示增删、还原后正文回滚',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      { op: 'keys', combo: 'cmd+p' },
      { op: 'paste', text: '目标笔记' },
      { op: 'keyCode', code: 36 },
      { op: 'keys', combo: 'cmd+s' },
      { op: 'check', path: '.knowledge/history', exists: true },
      { op: 'shot', name: '05-history-dialog.png' },
      {
        op: 'manual',
        note: '「更多操作 → 历史版本…」的命名 / 还原 / 对比需要人眼看：菜单弹层与 diff 内容无法用 AX 断言',
      },
    ],
  },
  {
    id: 6,
    title: '网页剪藏（真实网络抓取公开网页）',
    expect: '抓 https://example.com 生成笔记：标题 / source_type / source_ref / 正文',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      { op: 'click', frame: { role: 'AXButton', title: '新建笔记' } },
      { op: 'click', frame: { role: 'AXButton', title: '从网页剪藏' } },
      { op: 'click', frame: { role: 'AXTextField', title: '要剪藏的网页地址' } },
      { op: 'paste', text: 'https://example.com' },
      { op: 'click', frame: { role: 'AXButton', title: '抓取' } },
      // 抓取要联网：抓完之前「剪藏为新笔记」是禁用的，必须等它回来
      { op: 'pause', ms: 8000 },
      { op: 'shot', name: '06-web-clip.png' },
      { op: 'click', frame: { role: 'AXButton', title: '剪藏为新笔记' } },
      { op: 'pause', ms: 2500 },
      {
        op: 'check',
        path: '00_Inbox',
        containsFile: 'Example',
        grep: 'example.com',
        modifiedWithinMinutes: 2,
      },
      {
        op: 'manual',
        note: '机器已验「抓取 → 生成笔记 → 落盘（含 source_ref）」；正文抽取的完整度与元数据展示需人眼核对一次',
      },
    ],
  },
  {
    id: 7,
    title: '研究助手：本地调研全链路',
    expect: '五步走通并确认写入；未开 AI 时被闸门挡住',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      { op: 'click', frame: { title: '研究助手' } },
      { op: 'click', frame: { role: 'AXTextField', title: '课题' } },
      { op: 'paste', text: 'Rust 异步运行时的调度策略' },
      { op: 'click', frame: { role: 'AXButton', title: '生成研究计划' } },
      { op: 'shot', name: '07-research-plan.png' },
      { op: 'check', path: '.knowledge/research', notExists: true },
      {
        op: 'manual',
        note: '验收库的 AI 总开关默认关闭：本条机器断言的是**闸门挡住**（没有会话落盘）。打开 AI 后走完五步并确认写入，需要人眼确认报告六段与写入 diff',
      },
    ],
  },
  {
    id: 8,
    title: '研究助手：联网检索',
    expect: '配置检索源 → 允许联网 → 首次确认 → 收集到本地 + 外部来源，注入页被丢弃',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      {
        op: 'manual',
        note: '需要先起本机桩检索服务（scripts/research/）并在界面配置检索源、打开联网档位并逐次确认',
      },
      { op: 'shot', name: '08-research-web.png' },
    ],
  },
  {
    id: 9,
    title: '首页「待复核」卡片点击跳转与时效角标',
    expect: '写一篇 valid_until 已过期的笔记 → 首页出现待复核卡片 → 点击跳转',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      /**
       * 前提：验收库里得有一篇**有效期已过**的笔记（`00_Inbox/…验收-待复核笔记.md`）。
       *
       * 为什么这条前提只能人工准备（2026-10-16 查清）：设有效期的控件是原生
       * `<input type="date">`——它在无障碍树里**完全不可见**（只有它的标签文字是 AXStaticText，
       * 输入框本身连个 role 都没有），AXPress 点不到、Tab 走过去再敲数字也写不进去（实测过两轮）。
       * 手工准备的那篇笔记里写清了来龙去脉；后面两条断言仍然是机器判定的。
       */
      {
        op: 'manual',
        note: '前提：库里要有一篇有效期已过的笔记（人工准备，见用例注释与 docs/ACCEPTANCE_V1.3.0.md）。缺了它下面两条断言会失败——那是"前提没摆好"，不是卡片坏了',
      },
      { op: 'click', frame: { title: '首页' } },
      { op: 'pause', ms: 1200 },
      { op: 'expectElement', query: { title: '待复核' }, label: '首页的「待复核」卡片' },
      // 时效角标（「已过期 N 天」）也在界面上断言一次，不只是看截图
      { op: 'expectElement', query: { title: '已过期' }, label: '时效角标「已过期」' },
      { op: 'shot', name: '09-review-due.png' },
      {
        op: 'manual',
        note: '角标文案与跳转后的详情行（⚠ 可能已过期 / ✓ 已索引）需人眼确认',
      },
    ],
  },
  {
    id: 11,
    title: '界面梳理：侧边栏开关归位、区块折叠、条目行不溢出列表栏',
    expect:
      '顶栏没有重复的侧边栏开关；收起后「展开侧边栏」与展开态「收起侧边栏」同一高度；分类 / 标签可折叠；长标题行的按钮仍在列表栏内',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      // 前提：「大纲位置」摆回默认——这条用例里"点开关切栏位"的前提靠它
      ...ENSURE_OUTLINE_PLACEMENT_LIST,
      { op: 'click', frame: { role: 'AXLink', title: '全部笔记' } },
      { op: 'pause', ms: 1000 },
      // 顶栏那个重复的「切换侧边栏」已删除
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '切换侧边栏' },
        label: '顶栏的「切换侧边栏」已删除',
      },
      // 侧边栏此刻是展开还是收起会影响后面每一步的坐标/元素：先摆成展开态
      // （侧边栏收起状态是内存态，上一条用例或别的操作都可能把它留在收起上）
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '收起侧边栏' },
        press: { role: 'AXButton', description: '展开侧边栏' },
        pressLabel: '展开侧边栏',
        label: '侧边栏先展开',
      },
      { op: 'click', frame: { role: 'AXLink', title: '全部笔记' } },
      { op: 'pause', ms: 1200 },
      // 侧边栏开关：收起后「展开」与展开态「收起」都在底栏同一行——
      // 断言的是"展开按钮确实在底栏那个位置"（坐标证据见报告里的 click(x,y)：两次 y 相同）
      { op: 'click', frame: { role: 'AXButton', description: '收起侧边栏' } },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '展开侧边栏' },
        label: '收起后底部的「展开侧边栏」',
      },
      // 收起不等于失去导航：图标栏里导航项都在（点一个真的能跳过去）
      {
        op: 'expectElement',
        query: { role: 'AXLink', description: '首页' },
        label: '收起态图标栏里的「首页」',
      },
      {
        op: 'expectElement',
        query: { role: 'AXLink', description: '回收站' },
        label: '收起态图标栏里的「回收站」',
      },
      { op: 'click', frame: { role: 'AXLink', description: '首页' } },
      {
        op: 'expectElement',
        query: { title: '最近编辑' },
        label: '点图标栏的「首页」真的到了首页',
      },
      { op: 'click', frame: { role: 'AXLink', description: '全部笔记' } },
      // 这一栏得先是**列表**：前面那条用例可能把大纲留在了这一栏（列表条目就不会渲染，
      // 后面"看条目行"的断言会找不到元素）。ensure 幂等：本来就在列表态就什么都不做。
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '多选' },
        press: { role: 'AXButton', description: '返回笔记列表' },
        pressLabel: '返回笔记列表',
        label: '这一栏先显示笔记列表',
      },
      { op: 'shot', name: '11-sidebar-collapsed.png' },
      { op: 'click', frame: { role: 'AXButton', description: '展开侧边栏' } },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '收起侧边栏' },
        label: '展开回来了（开关还在原处）',
      },
      // 分类 / 标签区块可折叠（折叠后条目不在了，展开后回来）
      { op: 'click', frame: { role: 'AXButton', description: '折叠分类' } },
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '技术' },
        label: '折叠后分类条目不在了',
      },
      { op: 'click', frame: { role: 'AXButton', description: '展开分类' } },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '技术' },
        label: '展开后分类条目回来了',
      },
      { op: 'click', frame: { role: 'AXButton', description: '折叠标签' } },
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '入门' },
        label: '折叠后标签条目不在了',
      },
      { op: 'click', frame: { role: 'AXButton', description: '展开标签' } },
      { op: 'shot', name: '11-sections.png' },
      /**
       * 长标题 + 窄列表：条目行**不能溢出**这一栏。
       *
       * 为什么用几何断言：AX 读不到透明度（"按钮是不是常显"它看不见），但读得到坐标；
       * 而 Radix ScrollArea 的内容层是 `display: table`，会按内容撑宽——溢出时行会比面板宽，
       * 行右侧的按钮就被裁到面板外（这正是用户报的现象）。外框取列表栏里的「按标题筛选」输入框
       * （它与条目行同宽、右边缘基本对齐）。
       */
      // 先切到「最近修改」再新建：新建的笔记才会排在最上面、落在可视区里
      // （排成「自定义顺序」时新笔记会排到最后，AX 树里根本看不到它）
      { op: 'click', frame: { role: 'AXButton', title: '最近修改' } },
      { op: 'pause', ms: 800 },
      { op: 'click', frame: { role: 'AXButton', title: '新建笔记' } },
      {
        op: 'paste',
        text: '一篇特别特别长的标题用来验证截断时右侧两个快捷按钮仍然可见（真机验收）',
      },
      { op: 'click', frame: { role: 'AXButton', title: '创建笔记' } },
      { op: 'pause', ms: 1500 },
      { op: 'drag', from: { role: 'AXSplitter', title: '调整笔记列表宽度' }, to: { dx: -80, dy: 0 } },
      { op: 'pause', ms: 600 },
      // 看条目行之前再确认一次这一栏是列表（列表条目只在列表态才渲染）
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '多选' },
        press: { role: 'AXButton', description: '返回笔记列表' },
        pressLabel: '返回笔记列表',
        label: '看条目行前：这一栏是列表',
      },
      {
        op: 'expectInside',
        inner: { role: 'AXButton', description: '特别特别长' },
        outer: { role: 'AXTextField', description: '按标题筛选' },
        label: '长标题条目行仍在列表栏内（没有溢出）',
      },
      { op: 'shot', name: '11-long-title-row.png' },
      // 栏头一行到底（2026-10-16）：标题再长，图标与「大纲」也不许被挤成两行——
      // 栏头被挤成两行时高度会翻倍，所以直接量高度最省事
      { op: 'click', frame: { role: 'AXCheckBox', description: '大纲' } },
      { op: 'pause', ms: 900 },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '返回笔记列表' },
        label: '长标题下大纲栏头仍在（有「返回笔记列表」）',
      },
      {
        op: 'expectSize',
        query: { role: 'AXButton', description: '返回笔记列表' },
        minWidth: 12,
        maxHeight: 24,
        label: '栏头里那颗返回按钮还是一行高',
      },
      { op: 'shot', name: '11-outline-header-long-title.png' },
      { op: 'click', frame: { role: 'AXButton', description: '返回笔记列表' } },
      { op: 'pause', ms: 600 },
      {
        op: 'manual',
        note: '截断时右侧那两个按钮「是不是常显、间距顺不顺眼」要看截图/上手看一眼（AX 读不到透明度）',
      },
    ],
  },
  {
    id: 12,
    title: '编辑器新能力：图形渲染、插入菜单、大纲折叠',
    expect:
      '插入菜单在编辑 / 分屏可用；```mermaid 流程图在预览里真的渲染出图（WebKit 上验）；大纲能按节折叠',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      // 前提：大纲位置摆回默认（后面要靠"点开关 → 这一栏换成大纲"）
      ...ENSURE_OUTLINE_PLACEMENT_LIST,
      { op: 'click', frame: { role: 'AXLink', title: '全部笔记' } },
      { op: 'pause', ms: 1000 },
      { op: 'click', frame: { role: 'AXButton', title: '新建笔记' } },
      { op: 'paste', text: '图形与折叠（真机验收）' },
      { op: 'click', frame: { role: 'AXButton', title: '创建笔记' } },
      { op: 'pause', ms: 1800 },
      // 插入菜单在编辑模式下可用（纯预览时置灰；那条断言在 E2E 里）
      // 注意 role：它是个 Radix 下拉，WebKit 暴露成 **AXPopUpButton**（不是 AXButton）
      {
        op: 'expectElement',
        query: { role: 'AXPopUpButton', description: '插入内容' },
        label: '编辑器工具栏最左边的「＋ 插入内容」',
      },
      // 正文：一段带层级的标题 + 一张 mermaid 流程图
      { op: 'keys', combo: 'cmd+e' },
      {
        op: 'paste',
        text: '# 一级标题\n\n## 子标题一\n\n## 子标题二\n\n```mermaid\nflowchart LR\n  A[开始] --> B{判断}\n  B -->|是| C[结束]\n```\n',
      },
      { op: 'keys', combo: 'cmd+s' },
      { op: 'pause', ms: 1200 },
      // 切到纯预览：编辑器不再显示源码，「判断」这两个字只可能来自**渲染出来的图**
      { op: 'click', frame: { role: 'AXButton', title: '预览' } },
      { op: 'pause', ms: 2500 },
      {
        op: 'expectElement',
        query: { title: '判断' },
        label: '预览里的流程图（节点文字「判断」）',
      },
      { op: 'shot', name: '12-mermaid-preview.png' },
      { op: 'click', frame: { role: 'AXButton', title: '分屏' } },
      { op: 'pause', ms: 800 },
      // 大纲折叠：这一栏切到大纲，收起「一级标题」那一节 → 两个子标题的条目消失
      // 用 ensure（幂等）：这一栏可能**已经是**大纲（比如上一轮跑完就停在那个状态），
      // 直接 click 会"没找到元素"——目标状态已满足时它什么都不做
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '返回笔记列表' },
        press: { role: 'AXCheckBox', description: '大纲' },
        pressLabel: '工具栏大纲开关',
        label: '这一栏显示大纲',
      },
      { op: 'pause', ms: 900 },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '子标题一' },
        label: '大纲里有「子标题一」',
      },
      { op: 'click', frame: { role: 'AXButton', description: '折叠 一级标题' } },
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '子标题一' },
        label: '收起后子标题不再显示',
      },
      { op: 'shot', name: '12-outline-fold.png' },
      { op: 'click', frame: { role: 'AXButton', description: '展开 一级标题' } },
      {
        op: 'expectElement',
        query: { role: 'AXButton', description: '子标题二' },
        label: '展开后子标题回来了',
      },
      {
        op: 'manual',
        note: '图形的**观感**（连线是否清楚、深色主题下配色是否舒服）需要人眼看一下截图',
      },
    ],
  },
  {
    id: 13,
    title: '插入图片：原生文件选择框 → 落盘 attachments → 插入正文',
    expect:
      '点「插入内容 → 图片…」弹出系统文件面板；选中图片后附件落到 attachments/，正文写入 Markdown 图片链接',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      { op: 'click', frame: { role: 'AXLink', title: '全部笔记' } },
      { op: 'pause', ms: 1000 },
      { op: 'click', frame: { role: 'AXButton', title: '新建笔记' } },
      { op: 'paste', text: '插入图片（真机验收）' },
      { op: 'click', frame: { role: 'AXButton', title: '创建笔记' } },
      { op: 'pause', ms: 1800 },
      { op: 'keys', combo: 'cmd+e' },
      { op: 'pause', ms: 400 },
      /**
       * 回归背景（2026-10-18）：上一轮重写工具栏时把隐藏的 `<input type="file">` 删掉了，
       * 点「图片…」什么都不发生（用户实测报的正是这个）。这条用例从**菜单**点起，
       * 一直到"附件真的落盘"与"正文真的写了链接"，断的是整条链路。
       */
      // Radix 菜单只认 pointerdown：AXPress 打不开 → 先聚焦触发按钮，再用回车展开
      // （这也是键盘用户的真实路径；菜单项此时会作为 AXMenuItem 出现）
      { op: 'focus', query: { role: 'AXPopUpButton', description: '插入内容' }, label: '「＋ 插入内容」' },
      { op: 'pause', ms: 400 },
      { op: 'keyCode', code: 36 },
      { op: 'pause', ms: 900 },
      {
        op: 'expectElement',
        query: { role: 'AXMenuItem', title: '图片…' },
        label: '插入菜单里的「图片…」',
      },
      { op: 'click', frame: { role: 'AXMenuItem', title: '图片…' } },
      { op: 'pause', ms: 1800 },
      // 系统「打开」面板：⌘⇧G 输入路径 → 回车定位 → 回车打开
      { op: 'keyCode', code: 5, modifiers: ['shift', 'cmd'] },
      { op: 'pause', ms: 900 },
      { op: 'paste', text: '/tmp/pkos-check/sample.png' },
      { op: 'pause', ms: 600 },
      { op: 'keyCode', code: 36 },
      { op: 'pause', ms: 900 },
      { op: 'keyCode', code: 36 },
      { op: 'pause', ms: 2500 },
      { op: 'keys', combo: 'cmd+s' },
      { op: 'pause', ms: 1200 },
      // 附件落盘（必须是**这一轮刚生成**的）
      {
        op: 'check',
        path: 'attachments',
        containsFile: '.png',
        modifiedWithinMinutes: 2,
      },
      // 正文里写入了图片链接（Markdown 语法 + attachments 路径）
      {
        op: 'check',
        path: '00_Inbox',
        containsFile: '插入图片（真机验收）',
        grep: 'attachments/',
        modifiedWithinMinutes: 2,
      },
      { op: 'shot', name: '13-insert-image.png' },
      {
        op: 'manual',
        note: '图片在预览里是否正常显示、尺寸是否合适，需要人眼看一下截图',
      },
    ],
  },
];

/** 第 10 条（首次启动放行）**只能**在干净机器上人工验，脚本明确不覆盖。 */
const MANUAL_ONLY = [
  {
    id: 10,
    title: '首次启动放行（Gatekeeper）',
    reason: '只对「从网络下载的包」有意义；本机构建无 quarantine，脚本无法覆盖',
  },
];

module.exports = { APP_NAME, CASES, MANUAL_ONLY };
