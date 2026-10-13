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
 * - `check` / `expectElement`：文件断言与"界面上真的有这个元素"的断言。
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
    title: '拖拽分栏、大纲三态的手感',
    expect: '固定 → mini → 钉住 → 隐藏 → 再显示走全；拖分割线比例变化并落库',
    steps: [
      { op: 'app' },
      { op: 'keyCode', code: 53 },
      { op: 'keys', combo: 'cmd+p' },
      { op: 'paste', text: '目标笔记' },
      { op: 'keyCode', code: 36 },
      { op: 'click', frame: { role: 'AXButton', title: '大纲' } },
      { op: 'drag', from: { role: 'AXSplitter', title: '调整笔记列表宽度' }, to: { dx: 60, dy: 0 } },
      { op: 'shot', name: '04-outline-split.png' },
      { op: 'manual', note: '「跟手程度」与三态切换的主观手感，必须人手拖两下' },
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
