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
      { op: 'click', frame: { title: '设置' } },
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
      { op: 'click', frame: { title: '设置' } },
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
      // 1. 默认：大纲不占列表栏
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '返回笔记列表' },
        label: '默认状态：大纲不在列表栏里',
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
      // 3. 挪到正文右侧：列表栏还给列表，大纲变成正文右边独立一列
      {
        op: 'ensure',
        until: { role: 'AXButton', description: '收起大纲' },
        press: { role: 'AXButton', description: '钉在正文右侧' },
        pressLabel: '钉在正文右侧',
        label: '正文右侧的大纲（面板里有「收起大纲」）',
      },
      {
        op: 'expectAbsent',
        query: { role: 'AXButton', description: '返回笔记列表' },
        label: '大纲已离开列表栏',
      },
      { op: 'shot', name: '04-outline-side.png' },
      // 4. 再挪回列表栏（两个方向都有入口，不会"挪走找不回来"）
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
        note: 'mini 态与悬停展开（悬停无法用 AX 触发）、以及「拖拽跟不跟手」是主观手感，必须人手拖两下',
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
      { op: 'click', frame: { title: '首页' } },
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
