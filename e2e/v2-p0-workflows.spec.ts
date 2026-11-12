import { expect, test, type Page } from '@playwright/test';

async function createWorkspace(page: Page) {
  await page.addInitScript(() => window.localStorage.clear());
  await page.goto('/');
  await page.getByLabel('知识库名称').fill('V2 P0 工作流');
  await page.getByLabel('保存目录（绝对路径）').fill('/Users/you/Documents/V2P0Workflows');
  await page.getByRole('button', { name: '创建知识库' }).click();
}

async function createNote(page: Page, title: string, body: string) {
  await page.getByRole('button', { name: '新建笔记' }).first().click();
  await page.getByLabel('标题', { exact: true }).fill(title);
  await page.getByLabel('初始内容（可选）').fill(body);
  await page.getByRole('button', { name: '创建笔记' }).click();
  await expect(page.locator('.cm-content')).toBeVisible();
}

async function enableAi(page: Page) {
  await page.goto('/#/settings');
  const nav = page.getByRole('navigation', { name: '设置导航' });
  await nav.getByTestId('settings-nav-ai').click();
  await page.getByTestId('ai-enabled').click();
  await expect(page.getByTestId('ai-enabled')).toBeChecked();
}

async function enableNetwork(page: Page) {
  await enableAi(page);
  await page.getByTestId('ai-tab-retrieval').click();
  await page.getByTestId('ai-network').selectOption('allowNetwork');
  await page.getByTestId('ai-network-confirm-ok').click();
  await expect(page.getByTestId('ai-network')).toHaveValue('allowNetwork');
}

test('Ask 提问、展示可引用材料并经预览确认存为普通笔记', async ({ page }) => {
  await createWorkspace(page);
  await enableAi(page);

  await page.goto('/#/notes');
  await createNote(
    page,
    'SQLite 索引材料',
    '# SQLite 索引\n\nSQLite FTS5 使用倒排索引和双字短语检索。',
  );

  await page.getByRole('button', { name: '问知识库' }).click();
  await page.getByLabel('问题').fill('SQLite FTS5 使用什么索引？');
  await page.getByRole('button', { name: /当前笔记.*中提问/ }).click();

  await expect(page.getByText('有可引用材料')).toBeVisible();
  await expect(page.getByText('来源与段落')).toBeVisible();
  await page.getByRole('button', { name: /存为笔记/ }).click();

  const saveDialog = page.getByRole('dialog', { name: '把答案存为笔记' });
  await expect(saveDialog).toBeVisible();
  await expect(saveDialog).toContainText('预览模式不调用模型');
  await saveDialog.getByRole('button', { name: '确认写入' }).click();

  await expect(saveDialog).not.toBeVisible();
  await expect(page.locator('.cm-content')).toContainText('证据整理（浏览器预览模板）');
  await expect(page.getByRole('heading', { name: '问知识库' })).not.toBeVisible();
});

test('Research 编辑计划、保存模板、重跑并展示差异', async ({ page }) => {
  await createWorkspace(page);
  await enableAi(page);

  await page.goto('/#/notes');
  await createNote(
    page,
    'SQLite 全文检索索引',
    '# 索引\n\nSQLite FTS5 使用倒排索引，双字短语提升中文召回。',
  );
  await createNote(page, 'SQLite 查询性能', '# 性能\n\nFTS5 查询性能取决于索引分片与分页策略。');

  await page.goto('/#/research');
  await page.getByTestId('research-topic').fill('SQLite 全文检索性能');
  await page.getByTestId('research-start').click();

  await page.getByRole('button', { name: '编辑计划' }).click();
  await page.getByLabel('子问题（一行一个）').fill('索引结构如何影响召回？\n查询性能如何测量？');
  await page.getByLabel('时间窗（天）').fill('365');
  await page.getByLabel('语言').fill('zh-CN');
  await page.getByLabel('来源类型').fill('local');
  await page.getByRole('button', { name: '保存计划' }).click();
  await expect(page.getByTestId('research-plan')).toContainText('索引结构如何影响召回？');
  await expect(page.getByTestId('research-plan')).toContainText('时间窗 365 天');

  await page.getByLabel('保存为计划模板').fill('全文检索模板');
  await page.getByRole('button', { name: '保存模板' }).click();
  await expect(page.getByText('研究计划已保存为模板')).toBeVisible();
  await expect(page.getByLabel('计划模板（可选）')).toContainText('全文检索模板');

  await page.getByTestId('research-collect').click();
  await expect(page.getByTestId('research-sources')).toBeVisible();

  await page.getByRole('button', { name: '同课题重跑' }).click();
  await page.getByRole('button', { name: '与上次对比' }).click();
  await expect(page.getByRole('heading', { name: '与上次调研的差异' })).toBeVisible();
  await expect(page.getByText(/新增来源 \d+/)).toBeVisible();
  await expect(page.getByText(/消失来源 \d+/)).toBeVisible();
  await expect(page.getByText(/结论变化 \d+/)).toBeVisible();
});

test('长大纲可独立滚动、点击后段章节后仍能返回笔记列表', async ({ page }) => {
  await createWorkspace(page);
  await page.goto('/#/notes');
  const body = Array.from(
    { length: 40 },
    (_, index) => `## 章节 ${index + 1}\n\n这是第 ${index + 1} 节正文内容。`,
  ).join('\n\n');
  await createNote(page, '长文档大纲测试', body);

  await page.getByTestId('outline-toggle').click();
  await expect(page.getByTestId('list-pane-outline')).toBeVisible();
  await expect(page.getByTestId('outline-back')).toBeVisible();
  await expect(page.getByTestId('outline-back')).toContainText('返回列表');

  const scroll = page.getByTestId('outline-scroll');
  expect(await scroll.evaluate((element) => element.scrollHeight > element.clientHeight)).toBe(
    true,
  );

  await scroll.evaluate((element) => {
    element.scrollTop = element.scrollHeight;
  });
  const last = page.getByTestId('outline-item').last();
  await last.click();
  await expect(page.getByTestId('outline-back')).toBeVisible();

  await scroll.evaluate((element) => {
    element.scrollTop = 0;
  });
  await expect(page.getByTestId('outline-item').first()).toBeVisible();

  await page.getByTestId('outline-back').click();
  await expect(page.getByTestId('note-list-panel')).toBeVisible();
  await expect(page.getByTestId('list-pane-outline')).not.toBeVisible();
});

test('长标题悬停时横向滚动并显示覆盖式操作按钮', async ({ page }) => {
  await createWorkspace(page);
  await page.goto('/#/notes');
  const longTitle = '查看当前项目文件中的文档，构建一个可本地化运行的桌面程序和完整知识库工作流';
  await createNote(page, longTitle, '# 长标题测试\n\n用于验证列表项悬停效果。');

  const row = page.getByTestId('note-list-item').first();
  const title = row.getByTestId('note-list-item-title');
  const titleWindow = row.getByTestId('note-list-item-title-scroll');
  const titleContent = row.getByTestId('note-list-item-title-content');
  const actions = row.getByTestId('note-row-actions');

  expect(await title.evaluate((element) => element.className)).not.toContain('truncate');
  expect(await titleWindow.evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(
    true,
  );

  await row.hover();
  await page.waitForTimeout(450);
  const transform = await titleContent.evaluate((element) => getComputedStyle(element).transform);
  expect(transform).not.toBe('none');
  expect(transform).not.toContain('1, 0, 0, 1, 0, 0');
  await expect(actions).toHaveCSS('opacity', '1');
});

test('内容发现：URL 抓取、候选勾选、深抓汇总并确认入库', async ({ page }) => {
  await createWorkspace(page);
  await enableNetwork(page);

  await page.goto('/#/discover');
  await expect(page.getByRole('heading', { name: '内容发现', exact: true })).toBeVisible();
  await page.getByTestId('content-discovery-url').fill('https://example.com/knowledge-article');
  await page.getByTestId('content-discovery-note').fill('用于日常知识积累');
  await page.getByTestId('content-discovery-create').click();
  await expect(page.getByTestId('content-discovery-network-confirm')).toBeVisible();
  await page.getByTestId('content-discovery-network-confirm-ok').click();

  await expect(page.getByTestId('content-discovery-detail')).toContainText('剪藏示例');
  const candidates = page.getByTestId('content-candidate');
  await expect(candidates).toHaveCount(2);
  await candidates.first().click();
  await page.getByTestId('content-discovery-crawl').click();
  await page.getByTestId('content-discovery-network-confirm-ok').click();
  await expect(page.getByText('已抓取页面')).toBeVisible();
  await page.screenshot({ path: 'docs/screenshots/54-content-discovery.png' });

  await page.getByTestId('content-discovery-save-open').click();
  await page.getByTestId('content-discovery-save').click();
  await expect(page.getByRole('link', { name: '打开笔记' })).toBeVisible();
  await page.getByRole('link', { name: '打开笔记' }).click();
  await expect(page.locator('.cm-content')).toContainText('内容摘录');
});
