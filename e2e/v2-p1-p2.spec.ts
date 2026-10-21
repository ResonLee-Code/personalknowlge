import { expect, test, type Page } from '@playwright/test';
import { mkdir } from 'node:fs/promises';
import path from 'node:path';

const SHOTS = path.resolve(process.cwd(), 'docs/screenshots');

test.beforeAll(async () => {
  await mkdir(SHOTS, { recursive: true });
});

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => window.localStorage.clear());
  await page.goto('/');
  await page.getByLabel('知识库名称').fill('V2 P1 P2');
  await page.getByLabel('保存目录（绝对路径）').fill('/Users/you/Documents/V2P1P2');
  await page.getByRole('button', { name: '创建知识库' }).click();
});

async function createNote(page: Page, title: string, body: string) {
  await page.getByRole('button', { name: '新建笔记' }).first().click();
  await page.getByLabel('标题', { exact: true }).fill(title);
  await page.getByLabel('初始内容（可选）').fill(body);
  await page.getByRole('button', { name: '创建笔记' }).click();
  await expect(page.locator('.cm-content')).toBeVisible();
}

test('Writing 逐段接受 → diff → 确认写入', async ({ page }) => {
  await createNote(
    page,
    '写作验收',
    '# 写作验收\n\n本地优先是核心边界。引用必须可以回到原文区间。',
  );
  await page.getByRole('button', { name: '写作', exact: true }).click();
  await expect(page.getByTestId('writing-panel')).toBeVisible();
  await page.getByRole('button', { name: '生成草稿' }).click();
  await expect(page.getByTestId('writing-segment-W1')).toBeVisible();
  await page.getByRole('button', { name: '预览 diff' }).click();
  await expect(page.getByTestId('writing-diff')).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '52-writing-diff.png') });
  await page.getByRole('button', { name: /确认写入/ }).click();
  await expect(page.locator('.cm-content')).toContainText('写作来源');
});

test('Graph 洞察展示可解释近邻与只读重复预览', async ({ page }) => {
  await createNote(page, '索引设计', '# 索引设计\n\n全文检索使用倒排索引和分块检索。');
  await page.getByRole('button', { name: '新建笔记' }).first().click();
  await page.getByLabel('标题', { exact: true }).fill('索引设计旧版');
  await page.getByLabel('初始内容（可选）').fill('# 索引设计\n\n全文检索使用倒排索引和分块检索。');
  await page.getByRole('button', { name: '创建笔记' }).click();
  await page.goto('/#/graph');
  await page.getByRole('button', { name: '洞察' }).click();
  await expect(page.getByTestId('graph-insights')).toBeVisible();
  await expect(page.getByText('本地词项 + 证据', { exact: true })).toBeVisible();
  await expect(page.getByRole('heading', { name: '语义近邻' })).toBeVisible();
  await page.screenshot({ path: path.join(SHOTS, '51-graph-insights.png') });
  await page.getByRole('button', { name: /索引设计旧版.*2 篇/ }).click();
  await expect(page.getByTestId('duplicate-preview')).toBeVisible();
  await page.getByRole('button', { name: '生成引用摘要' }).click();
  await expect(page.getByText('摘要预览（写入前可取消）')).toBeVisible();
  await page.getByRole('button', { name: '确认新建摘要笔记' }).click();
  await expect(page.locator('.cm-content')).toContainText('写作来源');
});

test('Sync 设置能保存非秘密配置且密码不回显', async ({ page }) => {
  await page.goto('/#/settings');
  const nav = page.getByRole('navigation', { name: '设置导航' });
  await nav.getByTestId('settings-nav-sync').click();
  await expect(page.getByTestId('sync-settings')).toBeVisible();
  await page.getByLabel('Endpoint').fill('https://dav.example.test/pkos');
  await page.getByLabel(/用户名/).fill('sync-user');
  await page.getByLabel('密码（只写不读）').fill('sync-password');
  await page.getByRole('button', { name: '保存同步设置' }).click();
  await expect(page.getByText('同步设置已保存在应用数据目录')).toBeVisible();
  await expect(page.getByLabel('密码（只写不读）')).toHaveValue('');
  await expect(page.getByLabel(/用户名/)).toHaveValue('');
  await page.screenshot({ path: path.join(SHOTS, '53-sync-settings.png') });
});
