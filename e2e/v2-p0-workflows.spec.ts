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

test('Ask 提问、展示可引用材料并经预览确认存为普通笔记', async ({ page }) => {
  await createWorkspace(page);

  await page.goto('/#/settings');
  const nav = page.getByRole('navigation', { name: '设置导航' });
  await nav.getByTestId('settings-nav-ai').click();
  await page.getByTestId('ai-enabled').click();
  await expect(page.getByTestId('ai-enabled')).toBeChecked();

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
