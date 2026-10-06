import { expect, test } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';

const SHOTS = path.resolve(process.cwd(), 'docs/screenshots');

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
  await page.screenshot({ path: path.join(SHOTS, '02-dashboard.png') });

  // 4. 新建笔记并进入编辑器
  await page.getByRole('button', { name: '新建笔记' }).first().click();
  await page.getByLabel('标题').fill('SQLite 全文搜索学习笔记');
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
  const handleBox = await page
    .getByRole('separator', { name: '调整笔记列表宽度' })
    .boundingBox();
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
  await page.getByRole('button', { name: /SQLite 全文搜索学习笔记/ }).first().click();
  await expect(page.locator('.cm-content')).toBeVisible();
  await page.getByRole('button', { name: '更多操作' }).click();
  await page.getByRole('menuitem', { name: /导出（Markdown/ }).click();
  await expect(page.getByText('Markdown (.md)')).toBeVisible();
  await expect(page.getByText('Word (.docx)')).toBeVisible();
  await expect(page.getByText('PDF（打印视图）')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '13-export-dialog.png') });
  await page.getByRole('button', { name: '完成' }).click();

  // 14. 设置页布局：内容占满并居中（回归：右侧曾出现大片空白）
  await page.goto('/#/settings');
  await expect(page.getByRole('heading', { name: '设置', level: 1 })).toBeVisible();
  const mainBox = await page.locator('main').boundingBox();
  const contentBox = await page.locator('main div.mx-auto').first().boundingBox();
  if (!mainBox || !contentBox) throw new Error('未找到主区域或内容容器');
  expect(contentBox.width).toBeGreaterThan(mainBox.width * 0.5);
  const leftGap = contentBox.x - mainBox.x;
  const rightGap = mainBox.x + mainBox.width - (contentBox.x + contentBox.width);
  expect(Math.abs(leftGap - rightGap)).toBeLessThan(8);
  await page.screenshot({ path: path.join(SHOTS, '14-settings-layout.png') });

  await page.goto('/#/trash');
  await expect(page.getByText('回收站是空的')).toBeVisible();

  expect(errors, `控制台错误：${errors.join('\n')}`).toHaveLength(0);
});
