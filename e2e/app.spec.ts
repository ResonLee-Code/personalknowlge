import { expect, test, type Locator, type Page } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';

const SHOTS = path.resolve(process.cwd(), 'docs/screenshots');

/**
 * 点击提示条里的「撤销」。
 *
 * 提示条自带进出场动画，在机器忙碌时 Playwright 的「元素稳定」检查会一直重试直到超时；
 * 这里直接派发 click 事件（React 的合成事件照样会触发），避免与动画赛跑。
 */
async function clickUndo(page: Page) {
  const undo = page.getByRole('button', { name: '撤销' }).last();
  await expect(undo).toBeVisible();
  await undo.dispatchEvent('click');
}

// 每次运行都从干净的内存后端开始
test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => window.localStorage.clear());
});

test.beforeAll(async () => {
  await mkdir(SHOTS, { recursive: true });
});

test('核心流程：创建工作区 → 新建笔记 → 搜索 → 设置', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(String(error)));
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text());
  });

  // 1. 欢迎页
  await page.goto('/');
  await expect(page.getByRole('heading', { name: '本地优先的个人知识库' })).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '01-welcome.png') });

  // 2. 创建工作区
  await page.getByLabel('知识库名称').fill('我的知识库');
  await page.getByLabel('保存目录（绝对路径）').fill('/Users/you/Documents/MyKnowledge');
  await page.getByRole('button', { name: '创建知识库' }).click();

  // 3. 首页
  await expect(page.getByText('最近编辑')).toBeVisible();
  await expect(page.getByText('Inbox 待整理')).toBeVisible();
  // 统计明细（V1.2.1）：字数 / 近期更新 / 置顶，全部来自本地数据
  await expect(page.getByTestId('workspace-stats')).toBeVisible();
  await expect(page.getByTestId('workspace-stats')).toContainText('正文字数');
  await expect(page.getByTestId('workspace-stats')).toContainText('置顶笔记');
  await expect(page.getByTestId('writing-activity')).toContainText('近 14 天写作活动');
  await page.screenshot({ path: path.join(SHOTS, '02-dashboard.png') });

  // 4. 新建笔记并进入编辑器
  await page.getByRole('button', { name: '新建笔记' }).first().click();
  await page.getByLabel('标题', { exact: true }).fill('SQLite 全文搜索学习笔记');
  await page.getByLabel('标签（逗号分隔）').fill('sqlite, search');
  await page
    .getByLabel('初始内容（可选）')
    .fill('# 全文搜索\n\nSQLite FTS5 支持中文双字短语检索。\n\n- 索引策略\n- 查询策略');
  await page.getByRole('button', { name: '创建笔记' }).click();

  await expect(page.locator('.cm-content')).toBeVisible();
  await expect(page.getByText('SQLite 全文搜索学习笔记').first()).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '03-editor.png') });

  // 5. 编辑内容触发自动保存
  await page.locator('.cm-content').click();
  await page.keyboard.press('End');
  await page.keyboard.type('\n\n补充：中文检索需要双字扩展。');
  await expect(page.getByText('已保存').first()).toBeVisible({ timeout: 15_000 });

  // 5b. 编辑器快捷键：⌘S 立即保存（把自动保存延迟调大，证明是快捷键而不是自动保存）、⌘E 聚焦编辑器
  const editorUrl = page.url();
  await page.goto('/#/settings');
  // 选项范围是 500–3000ms：调到最大值后，⌘S 的效果才和自动保存区分得开
  await page.getByLabel('自动保存延迟').selectOption('3000');
  await page.goto(editorUrl);
  await page.locator('.cm-content').click();
  await page.keyboard.press('End');
  await page.keyboard.type('\n\n⌘S 应当立即保存。');
  await expect(page.getByText('未保存').first()).toBeVisible();
  await page.keyboard.press('Meta+s');
  // 断言窗口（1500ms）小于自动保存延迟（3000ms）：这一条只能由快捷键满足
  await expect(page.getByText('已保存').first()).toBeVisible({ timeout: 1500 });

  // ⌘E：先点到别处，再用快捷键把焦点送回编辑器
  await page.getByTestId('note-list-item').first().click();
  await page.keyboard.press('Meta+e');
  await expect(page.locator('.cm-content')).toBeFocused();

  // 5c. 防抖窗口内切走笔记不能丢改动（切走前必须先落盘）
  await page.locator('.cm-content').click();
  await page.keyboard.press('End');
  await page.keyboard.type('\n\n切走前的这行字必须落盘。');
  // 立刻点到另一篇笔记：此刻 3000ms 的自动保存防抖还没到
  await page.getByTestId('note-list-item').filter({ hasText: '欢迎使用' }).first().click();
  await expect(page.locator('.cm-content')).toContainText('欢迎使用');
  // 切回来：刚才那行字必须还在（说明切换前已经落盘）
  await page.getByTestId('note-list-item').filter({ hasText: 'SQLite 全文搜索学习笔记' }).first().click();
  await expect(page.locator('.cm-content')).toContainText('切走前的这行字必须落盘');

  // 还原自动保存延迟，避免影响后续步骤的等待逻辑
  await page.goto('/#/settings');
  await page.getByLabel('自动保存延迟').selectOption('1000');
  await page.goto(editorUrl);

  // 6. 全局搜索面板（⌘/Ctrl + K）
  await page.keyboard.press('Meta+k');
  await expect(page.getByPlaceholder(/输入关键词开始搜索|输入以搜索标题与正文/)).toBeVisible();
  await page.keyboard.type('中文检索');
  await expect(page.getByText('SQLite 全文搜索学习笔记').first()).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '04-search-palette.png') });
  await page.keyboard.press('Escape');

  // 7. 搜索页面
  await page.goto('/#/search?q=中文检索');
  await expect(page.getByText(/共 \d+ 条结果/)).toBeVisible();
  await expect(page.locator('.search-highlight').first()).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '05-search-page.png') });

  // 8. 分类与标签管理
  await page.goto('/#/categories');
  await expect(page.getByRole('heading', { name: '分类管理', level: 1 })).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '06-categories.png') });

  // 嵌套分类显示**完整**目录路径（后端 dirName 只是单级目录名，直接显示会让人误以为在根目录）
  const techRow = page.locator('li').filter({ hasText: '01_技术' }).first();
  await techRow.getByTitle('分类操作').click();
  await page.getByRole('menuitem', { name: '新建子分类' }).click();
  await page.getByLabel('分类名称').fill('数据库');
  await page.getByRole('button', { name: '创建' }).click();
  await expect(page.getByText(/01_技术\/\d+_数据库/)).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '06b-categories-nested.png') });

  await page.goto('/#/tags');
  await expect(page.getByRole('heading', { name: '标签', level: 1 })).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '07-tags.png') });

  // 9. 设置页面 + 深色主题
  await page.goto('/#/settings');
  await expect(page.getByRole('heading', { name: '设置', level: 1 })).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '08-settings.png') });

  await page.getByRole('button', { name: '切换为深色主题' }).click();
  await expect(page.locator('html')).toHaveClass(/dark/);
  await page.screenshot({ path: path.join(SHOTS, '09-dark-theme.png') });

  // 10. 笔记列表 + 回收站流程
  await page.goto('/#/notes');
  await expect(page.getByRole('heading', { name: '全部笔记' })).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '10-note-list-dark.png') });

  // 11. 列表宽度改为拖拽把手，底部不再出现无法解释的滑块
  await expect(page.locator('input[type="range"]')).toHaveCount(0);

  // 分栏可拖拽：向右拖动列表把手应加宽列表
  const listPanel = page.getByTestId('note-list-panel');
  const beforeBox = await listPanel.boundingBox();
  const handleBox = await page.getByRole('separator', { name: '调整笔记列表宽度' }).boundingBox();
  if (!beforeBox || !handleBox) throw new Error('未找到列表或分栏把手');
  await page.mouse.move(handleBox.x + handleBox.width / 2, handleBox.y + handleBox.height / 2);
  await page.mouse.down();
  await page.mouse.move(handleBox.x + 80, handleBox.y + handleBox.height / 2, { steps: 8 });
  await page.mouse.up();
  const afterBox = await listPanel.boundingBox();
  expect(afterBox!.width).toBeGreaterThan(beforeBox.width + 40);

  // 按分类筛选时才出现「清除筛选」入口
  await page.getByRole('button', { name: /^技术/ }).click();
  await expect(page.getByRole('button', { name: /清除筛选，查看全部笔记/ })).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '11-clear-filter.png') });
  await page.getByRole('button', { name: /清除筛选，查看全部笔记/ }).click();
  await expect(page.getByRole('heading', { name: '全部笔记' })).toBeVisible();

  // 12. 新建笔记对话框：手动创建 / 导入外部文件
  await page.getByRole('button', { name: '新建笔记' }).first().click();
  await expect(page.getByRole('button', { name: /导入外部文件/ })).toBeVisible();
  await page.getByRole('button', { name: /导入外部文件/ }).click();
  // Playwright 的 name 默认是子串匹配，这里用 exact 区分「选择文件 / 选择文件夹」
  await expect(page.getByRole('button', { name: '选择文件', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: '选择文件夹', exact: true })).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '12-import-dialog.png') });
  await page.getByRole('button', { name: '取消' }).click();

  // 13. 笔记导出入口（四种格式）
  await page.goto('/#/notes');
  await page
    .getByRole('button', { name: /SQLite 全文搜索学习笔记/ })
    .first()
    .click();
  await expect(page.locator('.cm-content')).toBeVisible();
  await page.getByRole('button', { name: '更多操作' }).click();
  await page.getByRole('menuitem', { name: /导出（Markdown/ }).click();
  await expect(page.getByText('Markdown (.md)')).toBeVisible();
  await expect(page.getByText('Word (.docx)')).toBeVisible();
  await expect(page.getByText('PDF（打印视图）')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '13-export-dialog.png') });
  await page.getByRole('button', { name: '完成' }).click();

  // 14. 设置页布局（2026-10-16 改版）：左侧分组导航 + 搜索，右侧只显示当前分节
  await page.goto('/#/settings');
  await expect(page.getByRole('heading', { name: '设置', level: 1 })).toBeVisible();
  const nav = page.getByRole('navigation', { name: '设置导航' });
  await expect(nav).toBeVisible();
  await expect(nav.getByTestId('settings-nav-editor')).toHaveAttribute('aria-current', 'true');
  // 右侧内容在导航右边（两者不重叠），且宽度占得住
  const navBox = await nav.boundingBox();
  const contentBox = await page.locator('main div.mx-auto').first().boundingBox();
  if (!navBox || !contentBox) throw new Error('未找到导航或内容容器');
  expect(contentBox.x).toBeGreaterThan(navBox.x + navBox.width - 1);
  expect(contentBox.width).toBeGreaterThan(600);
  // 分节切换：点「备份与导出」→ 出现备份相关行，编辑器行消失
  await nav.getByTestId('settings-nav-backup').click();
  await expect(page.getByText('创建备份')).toBeVisible();
  await expect(page.getByText('自动保存延迟')).toHaveCount(0);
  // 搜索：输入「字号」→ 只剩命中的行（跨分节都能找到）
  await page.getByLabel('搜索设置').fill('字号');
  await expect(page.getByText('字号', { exact: true })).toBeVisible();
  await expect(page.getByText('创建备份')).toHaveCount(0);
  await page.getByLabel('搜索设置').fill('');
  await nav.getByTestId('settings-nav-editor').click();
  await page.screenshot({ path: path.join(SHOTS, '14-settings-layout.png') });

  // 15. 分屏比例拖拽：编辑栏宽度应跟随指针（回归：首次进入时比例换算错误导致跳变）
  await page.goto('/#/notes');
  await page
    .getByRole('button', { name: /SQLite 全文搜索学习笔记/ })
    .first()
    .click();
  await expect(page.locator('.cm-content')).toBeVisible();
  const editorPane = page.getByTestId('editor-pane');
  const paneBefore = await editorPane.boundingBox();
  const splitHandle = await page
    .getByRole('separator', { name: '调整编辑 / 预览比例' })
    .boundingBox();
  if (!paneBefore || !splitHandle) throw new Error('未找到编辑栏或分屏把手');
  const splitX = splitHandle.x + splitHandle.width / 2;
  const splitY = splitHandle.y + 60;
  await page.mouse.move(splitX, splitY);
  await page.mouse.down();
  await page.mouse.move(splitX + 60, splitY, { steps: 8 });
  await page.mouse.up();
  const paneAfter = await editorPane.boundingBox();
  const grown = paneAfter!.width - paneBefore.width;
  expect(grown).toBeGreaterThan(30);
  expect(grown).toBeLessThan(90);

  // 16. 切换笔记不应重置编辑模式（回归：切换后回到预览）
  await page.getByRole('button', { name: '编辑', exact: true }).click();
  await expect(page.locator('.cm-content')).toBeVisible();
  await page
    .getByRole('button', { name: /欢迎使用 Personal Knowledge OS/ })
    .first()
    .click();
  await expect(page.locator('.cm-content')).toBeVisible();

  // 分屏模式下切换笔记同样保持：编辑区与预览区都应存在
  await page.getByRole('button', { name: '分屏', exact: true }).click();
  await expect(page.getByTestId('editor-pane')).toBeVisible();
  await expect(page.getByTestId('preview-pane')).toBeVisible();
  await page
    .getByRole('button', { name: /SQLite 全文搜索学习笔记/ })
    .first()
    .click();
  await expect(page.getByTestId('editor-pane')).toBeVisible();
  await expect(page.getByTestId('preview-pane')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '15-edit-mode-kept.png') });

  // 17. 双向链接：反向链接面板 + 未解析链接一键创建（V1.1 M1-3）
  await page.goto('/#/notes');
  await page.getByRole('button', { name: '新建笔记' }).first().click();
  await page.getByLabel('标题', { exact: true }).fill('索引设计笔记');
  await page
    .getByLabel('初始内容（可选）')
    .fill('参考 [[SQLite 全文搜索学习笔记]]，另见 [[尚未存在的笔记]]。');
  await page.getByRole('button', { name: '创建笔记' }).click();
  await expect(page.locator('.cm-content')).toBeVisible();

  // 本篇有一处未解析链接，可一键创建；元信息栏显示引用计数
  await expect(page.getByTestId('unresolved-item')).toHaveCount(1);
  await expect(page.getByTestId('backlink-panel')).toContainText('被 0 篇引用');
  await page.screenshot({ path: path.join(SHOTS, '16-backlink-panel.png') });

  // 打开被引用的笔记，应看到来自「索引设计笔记」的反链与上下文
  await page
    .getByRole('button', { name: /SQLite 全文搜索学习笔记/ })
    .first()
    .click();
  await expect(page.getByTestId('backlink-item').first()).toContainText('索引设计笔记');
  await expect(page.getByTestId('backlink-item').first()).toContainText(
    '参考 [[SQLite 全文搜索学习笔记]]',
  );
  await page.screenshot({ path: path.join(SHOTS, '17-backlink-from-source.png') });

  // 18. 编辑器大纲（V1.1 M2-1；V1.3.0 起默认与笔记列表共用一栏）：解析标题层级并支持点击跳转
  // 默认状态下这一栏是笔记列表，大纲没有占位（这是布局调整要解决的问题）
  await expect(page.getByTestId('note-list-panel')).toBeVisible();
  await expect(page.getByTestId('outline-panel')).toHaveCount(0);
  // 打开大纲：它出现在同一栏位（列表让位），而不是再挤出一列
  await page.getByRole('button', { name: '显示大纲' }).click();
  await expect(page.getByTestId('list-pane-outline')).toBeVisible();
  await expect(page.getByTestId('outline-panel')).toBeVisible();
  await expect(page.getByTestId('note-list-panel')).toHaveCount(0);
  await expect(page.getByTestId('outline-note-title')).toContainText('SQLite 全文搜索学习笔记');
  await expect(page.getByTestId('outline-item').first()).toContainText('全文搜索');
  await page.getByTestId('outline-item').first().click();
  await page.screenshot({ path: path.join(SHOTS, '18-outline.png') });
  // 「〈〈 返回列表」回到笔记列表；列表栏头部那个开关是同一个开关
  await page.getByTestId('outline-back').click();
  await expect(page.getByTestId('note-list-panel')).toBeVisible();
  await expect(page.getByTestId('outline-panel')).toHaveCount(0);
  // 再打开一次，确认开关可反复；收起走「〈〈 返回列表」——
  // 这一栏显示大纲时栏头归大纲，开关本身也随列表栏一起让位
  await page.getByRole('button', { name: '显示大纲' }).click();
  await expect(page.getByTestId('outline-panel')).toBeVisible();
  await page.getByTestId('outline-back').click();
  await expect(page.getByTestId('outline-panel')).toHaveCount(0);

  // 19. ⌘/Ctrl + P 快速切换笔记（按标题模糊匹配）
  await page.keyboard.press('Meta+p');
  await expect(page.getByLabel('按标题跳转')).toBeVisible();
  await page.keyboard.type('索引设计');
  await expect(page.getByTestId('quick-switch-item').first()).toContainText('索引设计笔记');
  await page.screenshot({ path: path.join(SHOTS, '19-quick-switcher.png') });
  await page.keyboard.press('Enter');
  await expect(page.locator('input[placeholder="笔记标题"]')).toHaveValue('索引设计笔记');

  // 20. 预览里的双向链接 chip（V1.1 M2-2）：已解析可跳转、未解析可创建
  await expect(page.getByTestId('wikilink-chip')).toHaveCount(2);
  await expect(page.getByTestId('wikilink-chip').first()).toHaveText('SQLite 全文搜索学习笔记');
  await expect(page.getByTestId('wikilink-chip').nth(1)).toHaveText('尚未存在的笔记');
  await page.screenshot({ path: path.join(SHOTS, '20-wikilink-chips.png') });
  await page.getByTestId('wikilink-chip').first().click();
  await expect(page.locator('input[placeholder="笔记标题"]')).toHaveValue(
    'SQLite 全文搜索学习笔记',
  );

  // 21. 知识图谱（V1.1 M3）：节点/连接统计 + 力导向画布
  await page.goto('/#/graph');
  await expect(page.getByRole('heading', { name: '知识图谱' })).toBeVisible();
  await expect(page.getByText(/3 个节点/)).toBeVisible();
  await expect(page.getByText(/1 条连接/)).toBeVisible();
  await expect(page.getByTestId('graph-canvas')).toBeVisible();
  // 等布局收敛（收敛后提示语会从「正在计算布局」切换）。
  // 收敛依赖力导向迭代的 tick 数，机器忙碌时会明显变慢，因此这里给足 25 秒。
  await expect(page.getByText(/拖拽节点可固定位置/)).toBeVisible({ timeout: 25_000 });
  await page.screenshot({ path: path.join(SHOTS, '21-graph.png') });

  // 22. 子文档层级 / 右键菜单 / 大纲预览（V1.2.1，对标语雀）
  await page.goto('/#/notes');
  const parentRow = page
    .getByTestId('note-list-item')
    .filter({ hasText: 'SQLite 全文搜索学习笔记' })
    .first();
  await parentRow.getByRole('button', { name: '笔记操作' }).click();
  await page.getByTestId('note-menu-create-child').click();
  await expect(page.getByRole('dialog')).toContainText('新建子文档');
  await page.getByLabel('子文档标题').fill('索引策略');
  await page.getByRole('button', { name: '创建子文档' }).click();
  // 创建后直接打开子文档
  await expect(page.locator('input[placeholder="笔记标题"]')).toHaveValue('索引策略');

  // 列表显示父子层级：父行带折叠箭头，子行以层级缩进呈现（行内不再有元信息文字）
  await expect(parentRow.getByTestId('note-tree-toggle')).toHaveAttribute('aria-expanded', 'true');
  const childRow = page.getByTestId('note-list-item').filter({ hasText: '索引策略' }).first();
  await expect(childRow).toBeVisible();
  await expect(childRow).toHaveAttribute('data-depth', '1');
  await page.screenshot({ path: path.join(SHOTS, '22-note-tree.png') });

  await parentRow.getByTestId('note-tree-toggle').click();
  await expect(childRow).toHaveCount(0);
  await parentRow.getByTestId('note-tree-toggle').click();
  await expect(childRow).toBeVisible();

  // 右键菜单（⋮ 与右键同一套）：截图留档，顺便确认菜单结构
  await parentRow.click({ button: 'right' });
  await expect(page.getByTestId('note-menu-create-child')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '23-context-menu.png') });
  await page.keyboard.press('Escape');

  // 大纲与笔记列表共用一栏（V1.3.0 布局调整）：点开某篇后按开关把这一栏切到大纲，点标题跳转
  await parentRow.click();
  await expect(page.getByTestId('note-list-panel')).toBeVisible();
  await page.getByRole('button', { name: '显示大纲' }).click();
  await expect(page.getByTestId('outline-panel')).toBeVisible();
  await expect(page.getByTestId('outline-item').first()).toContainText('全文搜索');
  await page.screenshot({ path: path.join(SHOTS, '24-outline-in-list.png') });
  await page.getByTestId('outline-item').first().click();
  await expect(page.locator('input[placeholder="笔记标题"]')).toHaveValue(
    'SQLite 全文搜索学习笔记',
  );
  // 回到列表，后面的层级操作（右键菜单）还在这一栏里做
  await page.getByTestId('outline-back').click();
  await expect(page.getByTestId('note-list-panel')).toBeVisible();

  // 子文档可升级为顶层文档
  await childRow.click({ button: 'right' });
  await page.getByTestId('note-menu-promote').click();
  await expect(parentRow.getByTestId('note-tree-toggle')).toHaveCount(0);

  // 23. 拖拽建立层级 / 同层排序（V1.2.1，对标语雀的拖拽语义）
  await page.goto('/#/notes');
  const dragSource = () =>
    page.getByTestId('note-list-item').filter({ hasText: '索引策略' }).first();
  const nestTarget = () =>
    page.getByTestId('note-list-item').filter({ hasText: 'SQLite 全文搜索学习笔记' }).first();
  const nestBox = await nestTarget().boundingBox();
  if (!nestBox) throw new Error('未找到拖拽目标行');
  // 拖到行中间 = 成为子文档（蓝色框）
  await dragSource().dragTo(nestTarget(), {
    targetPosition: { x: nestBox.width / 2, y: nestBox.height / 2 },
  });
  await expect(nestTarget().getByTestId('note-tree-toggle')).toHaveAttribute('aria-expanded', 'true');
  await expect(dragSource()).toHaveAttribute('data-depth', '1');
  await page.screenshot({ path: path.join(SHOTS, '25-drag-nest.png') });

  // 拖到「第一行」的上边缘 = 同层插入到最前（会自动切到「自定义顺序」）
  await page.getByRole('button', { name: '自定义顺序' }).click();
  const firstRow = page.getByTestId('note-list-item').first();
  await expect(firstRow).not.toContainText('索引策略');
  const firstBox = await firstRow.boundingBox();
  if (!firstBox) throw new Error('未找到第一个条目');
  await dragSource().dragTo(firstRow, { targetPosition: { x: firstBox.width / 2, y: 2 } });
  await expect
    .poll(async () => (await page.getByTestId('note-list-item').first().innerText()))
    .toContain('索引策略');
  await page.screenshot({ path: path.join(SHOTS, '26-drag-reorder.png') });

  // 24. 「不在目录中」筛选 + 删除可撤销（V1.2.1）
  await page.goto('/#/notes');
  const nestBox2 = await nestTarget().boundingBox();
  if (!nestBox2) throw new Error('未找到嵌套目标');
  await dragSource().dragTo(nestTarget(), {
    targetPosition: { x: nestBox2.width / 2, y: nestBox2.height / 2 },
  });
  await expect(dragSource()).toHaveAttribute('data-depth', '1');

  // 只看顶层文档：折叠在父文档下的子文档不再出现
  await page.getByTestId('root-only-toggle').click();
  await expect(dragSource()).toHaveCount(0);
  await expect(nestTarget()).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '27-root-only-filter.png') });
  await page.getByTestId('root-only-toggle').click();
  await expect(dragSource()).toBeVisible();

  // 删除后可随即撤销
  await dragSource().click({ button: 'right' });
  await page.getByTestId('note-menu-delete').click();
  await expect(dragSource()).toHaveCount(0);
  await clickUndo(page);
  await expect(dragSource()).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '28-delete-undo.png') });

  // 25. 置顶（对标语雀「置顶文档」）：置顶后排在列表最前，取消后回到原顺序
  await page.goto('/#/notes');
  const orderBeforePin = await page.getByTestId('note-list-item').allInnerTexts();
  // 选一篇顶层笔记（子文档必须跟在父文档下，不会上浮到列表最前）
  const pinTarget = page
    .getByTestId('note-list-item')
    .filter({ hasText: '欢迎使用 Personal Knowledge OS' })
    .first();
  await pinTarget.click({ button: 'right' });
  await page.getByTestId('note-menu-pin').click();
  await expect
    .poll(async () => (await page.getByTestId('note-list-item').allInnerTexts())[0])
    .toContain('欢迎使用');
  await expect(page.getByTestId('note-list-item').first().getByTestId('note-pinned-marker')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '29-pin.png') });

  await page.getByTestId('note-list-item').first().click({ button: 'right' });
  await page.getByTestId('note-menu-pin').click();
  await expect
    .poll(async () => (await page.getByTestId('note-list-item').allInnerTexts()).join('|'))
    .toBe(orderBeforePin.join('|'));

  // 26. 折叠全部 / 批量复制到分类（V1.2.1 P1 收尾）
  await page.goto('/#/notes');
  await expect(dragSource()).toBeVisible();
  await page.getByTestId('toggle-all-fold').click();
  await expect(dragSource()).toHaveCount(0);
  await page.getByTestId('toggle-all-fold').click();
  await expect(dragSource()).toBeVisible();

  await page.getByRole('button', { name: '多选' }).click();
  await page.getByLabel('选择 索引设计笔记').check();
  await page.getByLabel('选择 欢迎使用 Personal Knowledge OS').check();
  await page.getByTestId('batch-copy').click();
  await expect(page.getByRole('dialog')).toContainText('批量复制到分类');
  await page.getByLabel('目标分类').selectOption({ label: '技术' });
  await page.getByRole('button', { name: /复制 2 篇/ }).click();
  // 复制后总数从 4 变成 6，且原作仍在
  await expect(page.getByTestId('note-list-panel').getByText('6 篇')).toBeVisible();
  await expect(page.getByText('索引设计笔记').first()).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '30-batch-copy.png') });

  // 单篇「复制到分类」（C4）：副本落到指定分类，原作保留
  // （批量操作结束后多选模式已自动退出，这里直接操作条目）
  const rowsBeforeSingleCopy = await page.getByTestId('note-list-item').count();
  await page
    .getByTestId('note-list-item')
    .filter({ hasText: '欢迎使用 Personal Knowledge OS' })
    .first()
    .click({ button: 'right' });
  await page.getByTestId('note-menu-copy-to').click();
  await expect(page.getByRole('dialog')).toContainText('复制到分类');
  await page.getByLabel('目标分类').selectOption({ label: '技术' });
  await page.getByTestId('copy-note-confirm').click();
  await expect
    .poll(async () => page.getByTestId('note-list-item').count())
    .toBe(rowsBeforeSingleCopy + 1);
  await expect(page.getByTestId('note-list-item').filter({ hasText: '副本' }).first()).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '30b-single-copy.png') });

  // 27. 大纲「栏位切换 / 右侧常驻 / mini 态」与「另存为模板」入口（V1.2.1；V1.3.0 起可在两处位置间挪）
  await page.goto('/#/notes');
  await page.getByRole('button', { name: /SQLite 全文搜索学习笔记/ }).first().click();
  await expect(page.locator('.cm-content')).toBeVisible();
  // 默认在「笔记列表那一栏」，不占正文宽度
  await expect(page.getByTestId('note-list-panel')).toBeVisible();
  await page.getByRole('button', { name: '显示大纲' }).click();
  await expect(page.getByTestId('list-pane-outline')).toBeVisible();
  // 挪到正文右侧（宽屏更顺手）：列表栏让回给列表，大纲变成正文右边独立一列
  await page.getByTestId('outline-move-to-side').click();
  await expect(page.getByTestId('note-list-panel')).toBeVisible();
  await expect(page.getByTestId('outline-panel')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '30c-outline-to-side.png') });

  // 收起为贴边 mini 态：面板消失、留下贴边条；鼠标移上去临时展开
  await page.getByTestId('outline-collapse').click();
  await expect(page.getByTestId('outline-panel')).toHaveCount(0);
  await expect(page.getByTestId('outline-mini')).toBeVisible();
  await page.getByTestId('outline-mini').hover();
  await expect(page.getByTestId('outline-overlay')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '31-outline-mini.png') });

  // 钉住后固定展示，不再依赖悬停
  await page.getByTestId('outline-pin').click();
  await expect(page.getByTestId('outline-panel')).toBeVisible();
  await expect(page.getByTestId('outline-mini')).toHaveCount(0);
  // 也能从右侧列挪回列表栏（避免"挪走了找不回来"），最后恢复默认位置
  await page.getByTestId('outline-move-to-list').click();
  await expect(page.getByTestId('list-pane-outline')).toBeVisible();
  await expect(page.getByTestId('note-list-panel')).toHaveCount(0);

  // 另存为模板入口（浏览器预览模式没有工作区目录，这里验证入口与提示）
  await page.getByRole('button', { name: '更多操作' }).click();
  await page.getByRole('menuitem', { name: /另存为模板/ }).click();
  await expect(page.getByRole('dialog')).toContainText('另存为模板');
  await expect(page.getByRole('dialog')).toContainText('浏览器预览模式');
  await page.screenshot({ path: path.join(SHOTS, '32-save-template.png') });
  await page.getByRole('button', { name: '取消' }).click();
  // 收尾：这一栏正显示大纲（栏头归大纲），用「〈〈 返回列表」回到列表
  await page.getByTestId('outline-back').click();
  await expect(page.getByTestId('note-list-panel')).toBeVisible();

  // 28. 历史版本：留档 → 对比 → 还原（V1.2.1 P2）
  await page.goto('/#/notes');
  await page.getByRole('button', { name: /SQLite 全文搜索学习笔记/ }).first().click();
  await expect(page.locator('.cm-content')).toBeVisible();
  await page.locator('.cm-content').click();
  await page.keyboard.press('End');
  await page.keyboard.type('\n\n历史版本测试行。');
  await expect(page.getByText('已保存').first()).toBeVisible({ timeout: 15_000 });

  await page.getByRole('button', { name: '更多操作' }).click();
  await page.getByTestId('note-menu-history').click();
  await expect(page.getByRole('dialog')).toContainText('历史版本');
  await expect(page.getByTestId('history-entry').first()).toBeVisible();
  // 默认对比「最新快照 → 当前版本」，应有新增行（刚写进去的那行）
  await expect(page.getByTestId('diff-row-add').first()).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '33-history-dialog.png') });

  // 还原到最新快照（= 写入测试行之前的内容），确认编辑器里那行被回滚
  page.once('dialog', (dialog) => void dialog.accept());
  await page.getByTestId('history-restore').click();
  await expect(page.getByText(/已还原到该历史版本/)).toBeVisible({ timeout: 10_000 });
  await page.screenshot({ path: path.join(SHOTS, '34-history-restored.png') });
  await expect(page.locator('.cm-content')).not.toContainText('历史版本测试行');

  // 命名版本（对标语雀「保存为版本」）：给当前内容打版本名，再清除
  await page.getByRole('button', { name: '更多操作' }).click();
  await page.getByTestId('note-menu-history').click();
  await page.getByTestId('history-version-name').fill('v1 验收版');
  await page.getByTestId('history-save-version').click();
  await expect(page.getByTestId('history-entry-label').first()).toContainText('v1 验收版');
  await page.screenshot({ path: path.join(SHOTS, '34b-history-named-version.png') });

  await page.getByTestId('history-label-edit').first().click();
  await page.getByTestId('history-label-input').fill('');
  await page.getByTestId('history-label-save').click();
  await expect(page.getByTestId('history-entry-label')).toHaveCount(0);
  await page.getByRole('button', { name: '关闭' }).last().click();

  // 29. 删除有子文档的笔记：默认保留子文档，可勾选一起删除（V1.2.1 P2）
  await page.goto('/#/notes');
  const parentRowForDelete = page
    .getByTestId('note-list-item')
    .filter({ hasText: 'SQLite 全文搜索学习笔记' })
    .first();
  await expect(parentRowForDelete.getByTestId('note-tree-toggle')).toBeVisible();
  await parentRowForDelete.click({ button: 'right' });
  await page.getByTestId('note-menu-delete').click();
  await expect(page.getByRole('dialog')).toContainText('删除有子文档的笔记');
  await expect(page.getByTestId('delete-with-children')).not.toBeChecked();
  await page.screenshot({ path: path.join(SHOTS, '35-delete-parent.png') });
  await page.getByTestId('delete-confirm').click();
  // 父文档进回收站，子文档保留并回到顶层展示
  await expect(parentRowForDelete).toHaveCount(0);
  await expect(dragSource()).toBeVisible();
  // 用 toast 的「撤销」恢复
  await clickUndo(page);
  await expect(parentRowForDelete).toBeVisible();

  // 30. 目录管理页（V1.2.1 P2，对标语雀「目录管理」）
  await page.goto('/#/manage');
  await expect(page.getByRole('heading', { name: '目录管理' })).toBeVisible();
  await expect(page.getByTestId('manage-row').first()).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '36-manage.png') });

  // 筛选：切到「不在目录中」时，子文档行应从表里消失
  const managedRows = () => page.getByTestId('manage-row');
  const rowsBefore = await managedRows().count();
  await page.getByTestId('manage-root-only').check();
  await expect
    .poll(async () => managedRows().count())
    .toBeLessThan(rowsBefore);
  await page.getByTestId('manage-root-only').uncheck();

  // 批量：全选后打开移动对话框，取消退出
  await page.getByTestId('manage-select-all').check();
  await expect(page.getByTestId('manage-batch-toolbar')).toContainText('已选');
  await page.getByTestId('manage-move').click();
  await expect(page.getByRole('dialog')).toContainText('批量移动分类');
  await page.getByRole('button', { name: '取消' }).click();

  // 31. 网页剪藏（C1）：抓取 → 预览 → 剪藏为新笔记（含来源元数据）
  await page.goto('/#/notes');
  await page.getByRole('button', { name: '新建笔记' }).first().click();
  await page.getByRole('button', { name: /从网页剪藏/ }).click();
  await page.getByTestId('clip-url').fill('example.com/articles/fts5');
  await page.getByTestId('clip-fetch').click();
  await expect(page.getByTestId('clip-meta')).toContainText('example.com');
  await expect(page.getByTestId('clip-preview')).toContainText('剪藏示例');
  await page.screenshot({ path: path.join(SHOTS, '37-web-clip.png') });
  await page.getByTestId('clip-create').click();

  // 剪藏后直接打开新笔记：标题、正文抬头与来源标签都在
  await expect(page.locator('input[placeholder="笔记标题"]')).toHaveValue(/剪藏示例/);
  await expect(page.locator('.cm-content')).toContainText('> 来源：https://example.com/articles/fts5');
  await expect(page.getByText('网页剪藏')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '38-web-clip-note.png') });

  // 32. 设置 → AI（D1）：默认最保守口径；联网档位需先确认说明
  await page.goto('/#/settings');
  // 设置页现在是分节显示（2026-10-16）：AI 相关行在「AI 能力」这一节里
  await page.getByTestId('settings-nav-ai').click();
  await expect(page.getByTestId('settings-ai-section')).toBeVisible();
  await expect(page.getByTestId('ai-network')).toHaveValue('localOnly');
  await expect(page.getByTestId('ai-enabled')).not.toBeChecked();

  await page.getByTestId('ai-enabled').click();
  await expect(page.getByTestId('ai-enabled')).toBeChecked();
  await page.getByTestId('ai-network').selectOption('allowNetwork');
  await expect(page.getByTestId('ai-network-confirm')).toBeVisible();
  await expect(page.getByTestId('ai-network-confirm')).toContainText('不会上传整篇知识库');
  await page.screenshot({ path: path.join(SHOTS, '39-ai-policy.png') });
  await page.getByTestId('ai-network-confirm-ok').click();
  await expect(page.getByTestId('ai-network')).toHaveValue('allowNetwork');
  await page.getByTestId('ai-fetch-body').click();
  await expect(page.getByTestId('ai-fetch-body')).toBeChecked();
  // 检索源地址（D3）：应用不预置第三方服务，这里填一个返回 JSON 的地址模板
  await page.getByTestId('ai-search-endpoint').fill('https://searx.example/search?q={query}&format=json');
  await page.getByTestId('ai-search-endpoint').blur();
  await expect(page.getByTestId('ai-search-endpoint')).toHaveValue(/searx\.example/);
  // 本地模型（D5）：默认不启用（空地址 + 两个开关关闭），浏览器预览不调用本地模型
  await expect(page.getByTestId('ai-local-endpoint')).toHaveValue('');
  await expect(page.getByTestId('ai-local-compose')).not.toBeChecked();
  await expect(page.getByTestId('ai-local-score')).not.toBeChecked();

  // 33. 研究助手（D2）：本地调研 → 六段报告 → 确认写入
  // 先准备三篇本地材料（两条指向「方案」，一条指向「限制」→ 单一来源进待验证）
  const materials: Array<[string, string]> = [
    ['Rust 异步调度方案对比', '主流方案是对比 tokio 与 async-std 的调度实现。'],
    ['Rust 异步调度实现要点', '实现要点在于任务唤醒与本地队列的批量窃取。'],
    ['Rust 异步调度的限制', '限制条件：阻塞调用会拖垮运行时，注意别占用调度线程。'],
  ];
  for (const [title, content] of materials) {
    await page.goto('/#/notes');
    await page.getByRole('button', { name: '新建笔记' }).first().click();
    await page.getByLabel('标题', { exact: true }).fill(title);
    await page.getByLabel('初始内容（可选）').fill(content);
    await page.getByRole('button', { name: '创建笔记' }).click();
    await expect(page.locator('.cm-content')).toBeVisible();
  }

  await page.goto('/#/research');
  await expect(page.getByRole('heading', { name: '研究助手' })).toBeVisible();
  await page.getByTestId('research-topic').fill('Rust 异步调度方案');
  await page.getByTestId('research-start').click();
  await expect(page.getByTestId('research-plan')).toContainText('核心概念与定义');
  await page.screenshot({ path: path.join(SHOTS, '40-research-plan.png') });

  await page.getByTestId('research-collect').click();
  // 联网档位已打开：收集前必须出现一次说明（§9.7「首次发起研究」）
  await expect(page.getByTestId('research-network-confirm')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '40b-research-network.png') });
  await page.getByTestId('research-network-ok').click();
  await expect(page.getByTestId('research-sources')).toBeVisible();
  await expect(page.getByTestId('research-sources')).toContainText('docs.rs');
  await page.getByTestId('research-verify').click();
  await page.getByTestId('research-compose').click();
  const report = page.getByTestId('research-report');
  await expect(report).toContainText('资料截至');
  await expect(report).toContainText('调研报告：Rust 异步调度方案');
  // D5：决策层打分（规则档）与来源可信度都要可见
  await expect(report).toContainText('打分 rule');
  await expect(page.getByTestId('research-sources')).toContainText('可信度');
  await expect(page.getByTestId('research-warnings')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '41-research-report.png') });

  await page.getByTestId('research-proposal').click();
  await expect(page.getByTestId('research-proposal')).toContainText('Inbox');
  await page.getByTestId('research-write-open').click();
  await expect(page.getByTestId('research-write-diff')).toContainText('资料截至');
  await page.screenshot({ path: path.join(SHOTS, '42-research-write.png') });
  await page.getByTestId('research-write-mode-create').click();
  await page.getByTestId('research-write-confirm').click();
  await expect(page.getByTestId('research-written')).toContainText('已写入知识库');
  await page.screenshot({ path: path.join(SHOTS, '43-research-written.png') });

  // 写回的报告带上 N1 元数据（来源类型 / 可靠性 / 复核时间）
  await page.getByTestId('research-written-open').click();
  await expect(page.locator('.cm-content')).toContainText('资料截至');
  await expect(page.getByText('调研报告', { exact: true }).first()).toBeVisible();

  // 34. 待复核提醒（D4）：把这篇报告的复核时间改成过去 → 首页出现「待复核」卡片
  // 用本地日期拼 `YYYY-MM-DD`：toISOString 走 UTC，在东八区会差一天
  const overdue = new Date();
  overdue.setDate(overdue.getDate() - 5);
  const pad = (value: number) => String(value).padStart(2, '0');
  const overdueDate = `${overdue.getFullYear()}-${pad(overdue.getMonth() + 1)}-${pad(overdue.getDate())}`;
  await page.getByTitle('点击编辑来源与时效').click();
  await expect(page.getByRole('dialog')).toContainText('来源与时效');
  await page.getByLabel('有效期至').fill(overdueDate);
  await page.getByRole('button', { name: '保存' }).click();

  await page.goto('/#/');
  const reviewCard = page.getByTestId('review-due-card');
  await expect(reviewCard).toBeVisible();
  await expect(reviewCard).toContainText('待复核');
  await expect(reviewCard).toContainText('已过期 5 天');
  await expect(reviewCard).toContainText('调研报告：Rust 异步调度方案');
  await page.screenshot({ path: path.join(SHOTS, '44-review-due.png') });

  // 点标题跳到对应笔记（时效字段已写入 frontmatter）
  await reviewCard.getByRole('button', { name: '调研报告：Rust 异步调度方案' }).click();
  await expect(page.locator('.cm-content')).toContainText('资料截至');

  await page.goto('/#/trash');
  await expect(page.getByText('回收站是空的')).toBeVisible();

  // 35. 界面梳理（2026-10-15）：侧边栏开关同高、大纲开关进列表栏头部、长标题行的快捷按钮、
  //     分类 / 标签可折叠、侧边栏不再重复「设置」
  await page.goto('/#/notes');
  // 顶栏那个「切换侧边栏」删掉了：侧边栏自己有收起 / 展开，且两者停在**同一高度**
  await expect(page.getByRole('button', { name: '切换侧边栏' })).toHaveCount(0);
  const collapseBox = await page.getByRole('button', { name: '收起侧边栏' }).boundingBox();
  await page.getByRole('button', { name: '收起侧边栏' }).click();
  const expandBox = await page.getByRole('button', { name: '展开侧边栏' }).boundingBox();
  expect(Math.abs(expandBox!.y - collapseBox!.y)).toBeLessThan(4);
  // 收起后不是"空了"：导航项以图标栏形式留在那里（选中项高亮、悬停出名字）
  const railLinks = page.locator('nav[aria-label="主导航"] a');
  await expect(railLinks).toHaveCount(11);
  const railBackground = async (locator: Locator) =>
    locator.evaluate((element) => getComputedStyle(element).backgroundColor);
  const activeRail = page.locator('nav[aria-label="主导航"] a[aria-current="page"]');
  await expect(activeRail).toHaveCount(1);
  const inactiveRail = page.locator('nav[aria-label="主导航"] a[aria-label="回收站"]');
  expect(await railBackground(activeRail)).not.toBe(await railBackground(inactiveRail));
  await inactiveRail.hover();
  await expect(page.getByRole('tooltip')).toContainText('回收站');
  await page.screenshot({ path: path.join(SHOTS, '45-sidebar-rail.png') });
  await page.getByRole('button', { name: '展开侧边栏' }).click();
  await expect(page.getByTestId('sidebar')).toBeVisible();

  // 分类 / 标签区块可折叠（折叠后条目消失、aria-expanded 跟着变）
  await page.getByTestId('categories-toggle').click();
  await expect(page.getByTestId('categories-toggle')).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByRole('button', { name: /^技术/ })).toHaveCount(0);
  await page.getByTestId('categories-toggle').click();
  await expect(page.getByRole('button', { name: /^技术/ })).toBeVisible();
  await page.getByTestId('tags-toggle').click();
  await expect(page.getByTestId('tags-toggle')).toHaveAttribute('aria-expanded', 'false');
  await expect(page.getByRole('button', { name: /^入门/ })).toHaveCount(0);
  await page.getByTestId('tags-toggle').click();
  await page.screenshot({ path: path.join(SHOTS, '45-sections.png') });

  // 侧边栏不再有「设置」；右上角那个是唯一入口
  await expect(page.getByRole('link', { name: '设置' })).toHaveCount(0);
  await page.getByRole('button', { name: '设置' }).click();
  await expect(page.getByRole('heading', { name: '设置', level: 1 })).toBeVisible();
  await page.goto('/#/notes');

  // 列表栏头部：计数贴在标题右边、大纲开关在最右；没选中笔记时开关置灰（那会儿没有大纲可看）
  const listHeading = await page
    .getByTestId('note-list-panel')
    .getByRole('heading', { level: 1 })
    .boundingBox();
  const countBox = await page.getByTestId('note-list-count').boundingBox();
  const toggleBox = await page.getByTestId('outline-toggle').boundingBox();
  expect(countBox!.x).toBeGreaterThan(listHeading!.x);
  expect(Math.abs(countBox!.y - listHeading!.y)).toBeLessThan(6);
  expect(toggleBox!.x).toBeGreaterThan(countBox!.x);
  await expect(page.getByTestId('outline-toggle')).toBeDisabled();

  // 长标题被截断时，右侧两个快捷按钮**不靠 hover** 也可见：
  // 否则标题右边会留一块空白（按钮本来就占着位置只是透明），看着像坏了
  await page.getByRole('button', { name: '新建笔记' }).first().click();
  await page
    .getByLabel('标题', { exact: true })
    .fill('一篇标题特别特别长的笔记用来验证截断时右侧两个快捷按钮仍然可见');
  await page.getByRole('button', { name: '创建笔记' }).click();
  await expect(page.locator('.cm-content')).toBeVisible();
  const longRow = page.getByTestId('note-list-item').filter({ hasText: '特别特别长' }).first();
  const quickActions = longRow.getByTestId('note-quick-child').locator('..');
  // 长标题也一样：默认隐藏，鼠标移到条目上才出现（2026-10-16 用户口径）
  await expect(quickActions).toHaveCSS('opacity', '0');
  await longRow.hover();
  await expect(quickActions).toHaveCSS('opacity', '1');
  await expect(longRow.getByTestId('note-quick-child')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '46-long-title-row.png') });

  // 36. 编辑器工具栏（2026-10-16）：插入菜单、文本格式条、收藏橙色、保存状态只留顶栏一处
  await expect(page.getByText('已同步')).toHaveCount(1); // 顶栏那一处；编辑器工具栏不再重复
  const editorToolbar = page.locator('main');
  await expect(page.getByTestId('insert-menu')).toBeVisible();
  await expect(page.getByTestId('format-bar')).toBeVisible();
  // 收藏：点一下变橙色实心，再点回到未收藏
  const favorite = page.getByTestId('favorite-toggle');
  await favorite.click();
  await expect(favorite).toHaveAttribute('aria-pressed', 'true');
  // Tailwind 4 的颜色是 oklch，computed style 会报成 lab(...)：这里断言"真的填了色"而不是某个具体色值
  await expect(favorite.locator('svg')).toHaveClass(/fill-amber-400/);
  expect(
    await favorite.locator('svg').evaluate((element) => getComputedStyle(element).fill),
  ).not.toBe('none');
  await favorite.click();
  await expect(favorite).toHaveAttribute('aria-pressed', 'false');
  // 插入菜单：插一个表格（插在光标所在行之后，另起一块）
  await page.locator('.cm-content').click();
  await page.keyboard.press('End');
  await page.getByTestId('insert-menu').click();
  await page.getByRole('menuitem', { name: '表格' }).click();
  await expect(page.locator('.cm-content')).toContainText('| 列 1 |');
  // 格式条：选中一段文字后点「加粗」
  await page.locator('.cm-content').press('Home');
  for (let index = 0; index < 4; index += 1) await page.keyboard.press('Shift+ArrowRight');
  await page.getByRole('button', { name: '加粗' }).click();
  await expect(page.locator('.cm-content')).toContainText('**');
  await page.screenshot({ path: path.join(SHOTS, '48-insert-and-format.png') });
  // 纯预览模式下插入按钮禁用（没有光标可插）
  await page.getByRole('button', { name: '预览' }).click();
  await expect(page.getByTestId('insert-menu')).toBeDisabled();
  await expect(page.getByTestId('format-bar')).toHaveCount(0);
  await page.getByRole('button', { name: '分屏' }).click();

  // 大纲折叠（2026-10-16）：先写一段带层级的标题，再看这一栏的大纲
  await page.locator('.cm-content').click();
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await page.keyboard.type('# 一级标题\n## 子标题一\n## 子标题二\n# 另一个一级');
  await page.getByRole('button', { name: '显示大纲' }).first().click();
  await expect(page.getByTestId('list-pane-outline')).toBeVisible();
  const outlineCount = () => page.getByTestId('outline-item').count();
  await expect.poll(outlineCount).toBe(4);
  await page.getByTestId('outline-fold').first().click();
  // 「一级标题」收起来后，它的两个子标题被藏起来（4 → 2）
  await expect.poll(outlineCount).toBe(2);
  await page.getByTestId('outline-fold').first().click();
  await expect.poll(outlineCount).toBe(4);
  await page.screenshot({ path: path.join(SHOTS, '49-outline-fold.png') });
  await page.getByTestId('outline-back').click();

  // 37. 可视化图形（2026-10-16）：```mermaid 真的渲染成图（流程图 + 时序图）
  await page.locator('.cm-content').click();
  await page.keyboard.press('End');
  await page.keyboard.type(
    '\n```mermaid\nflowchart LR\n  A[开始] --> B{判断}\n  B -->|是| C[结束]\n```\n',
  );
  await page.getByRole('button', { name: '预览' }).click();
  await expect(page.locator('[data-testid="mermaid-diagram"]').first()).toBeVisible();
  await expect(page.locator('[data-testid="mermaid-diagram"]').first().locator('svg')).toBeVisible();
  await expect(page.locator('[data-testid="mermaid-diagram"]').first()).toContainText('判断');
  await expect(page.locator('[data-testid="mermaid-error"]')).toHaveCount(0);
  await page.screenshot({ path: path.join(SHOTS, '50-mermaid.png') });
  await page.getByRole('button', { name: '分屏' }).click();
  void editorToolbar;

  // 选中笔记后开关可用：点它 = 这一栏换成大纲（不多占一列）
  await expect(page.getByTestId('outline-toggle')).toBeEnabled();
  await page.getByTestId('outline-toggle').click();
  await expect(page.getByTestId('list-pane-outline')).toBeVisible();
  // 标题很长时栏头也**只有一行**：图标与「大纲」不被挤成两行，由标题自己截断
  const headerBox = await page.getByTestId('outline-panel').locator('header').boundingBox();
  const labelBox = await page.getByTestId('outline-label').boundingBox();
  expect(headerBox!.height).toBeLessThan(40);
  expect(labelBox!.height).toBeLessThan(22);
  await expect(page.getByTestId('outline-note-title')).toHaveCSS('text-overflow', 'ellipsis');
  await page.screenshot({ path: path.join(SHOTS, '47-outline-header-long-title.png') });
  await page.getByTestId('outline-back').click();
  await expect(page.getByTestId('note-list-panel')).toBeVisible();

  expect(errors, `控制台错误：${errors.join('\n')}`).toHaveLength(0);
});
