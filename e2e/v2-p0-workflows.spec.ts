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
