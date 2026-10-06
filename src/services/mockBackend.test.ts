import { beforeEach, describe, expect, it } from 'vitest';

import { noteService } from '@/services/noteService';
import { categoryService } from '@/services/categoryService';
import { searchService } from '@/services/searchService';
import { tagService } from '@/services/tagService';
import { workspaceService } from '@/services/workspaceService';

// 浏览器预览后端（localStorage 内存实现）的端到端验证。
// 同时覆盖 api.ts 的错误归一化，因为失败的命令会抛出 AppError 结构。
describe('浏览器预览后端', () => {
  beforeEach(async () => {
    localStorage.clear();
    await workspaceService.forget('ws_mock').catch(() => undefined);
  });

  it('创建工作区并返回统计信息', async () => {
    const result = await workspaceService.create('测试库', '/tmp/mock');
    expect(result.workspace.name).toBe('测试库');
    expect(result.stats.noteCount).toBeGreaterThanOrEqual(1);
    expect(result.settings.autosaveEnabled).toBe(true);
  });

  it('未打开工作区时列表命令返回 NOT_INITIALIZED', async () => {
    await expect(noteService.list({})).rejects.toMatchObject({ code: 'NOT_INITIALIZED' });
  });

  it('支持笔记的创建、保存、搜索、标签与删除恢复', async () => {
    await workspaceService.create('测试库', '/tmp/mock');

    const detail = await noteService.create({
      title: '全文搜索设计',
      initialContent: 'SQLite FTS5 中文检索方案',
      tags: ['搜索'],
    });
    expect(detail.note.title).toBe('全文搜索设计');
    expect(detail.tags.map((tag) => tag.name)).toContain('搜索');

    const saved = await noteService.save({
      id: detail.note.id,
      content: '更新后的内容：向量检索与倒排索引',
      baseContentHash: detail.note.contentHash,
    });
    expect(saved.indexUpdated).toBe(true);

    const search = await searchService.search({ keyword: '倒排索引' });
    expect(search.hits).toHaveLength(1);
    expect(search.hits[0]?.snippet.length).toBeGreaterThan(0);

    const tags = await tagService.list();
    expect(tags.map((tag) => tag.name)).toContain('搜索');

    await noteService.remove(detail.note.id);
    expect(await noteService.listDeleted()).toHaveLength(1);
    await noteService.restore(detail.note.id);
    expect(await noteService.listDeleted()).toHaveLength(0);
  });

  it('检测保存冲突', async () => {
    await workspaceService.create('测试库', '/tmp/mock');
    const detail = await noteService.create({ title: '冲突', initialContent: '原始' });
    await expect(
      noteService.save({
        id: detail.note.id,
        content: '新内容',
        baseContentHash: 'outdated-hash',
      }),
    ).rejects.toMatchObject({ code: 'NOTE_CONFLICT' });
  });

  it('分类支持创建、重命名与删除策略', async () => {
    await workspaceService.create('测试库', '/tmp/mock');
    const category = await categoryService.create('数据库', null);
    expect(category.dirName).toContain('数据库');

    await categoryService.rename(category.id, '数据库设计');
    const list = await categoryService.list();
    expect(list.some((item) => item.name === '数据库设计')).toBe(true);

    const report = await categoryService.remove(category.id, 'moveToInbox');
    expect(report.deletedCategories).toBe(1);
  });

  it('未实现的命令返回统一错误结构', async () => {
    await workspaceService.create('测试库', '/tmp/mock');
    await expect(
      import('@/services/api').then(({ call }) => call('not_implemented_command')),
    ).rejects.toMatchObject({ code: 'INTERNAL_ERROR' });
  });

  it('浏览器预览模式下导入本地文件会给出明确提示', async () => {
    await workspaceService.create('测试库', '/tmp/mock');
    await expect(
      noteService.importFiles({ paths: ['/tmp/note.md'], categoryId: null }),
    ).rejects.toMatchObject({
      code: 'INVALID_INPUT',
      message: expect.stringContaining('桌面应用'),
    });
  });

  it('浏览器预览模式下导出会给出明确提示', async () => {
    await workspaceService.create('测试库', '/tmp/mock');
    const note = await noteService.create({ title: '导出测试', initialContent: '正文' });
    await expect(
      noteService.exportNote({ id: note.note.id, format: 'docx', targetPath: '/tmp/a.docx' }),
    ).rejects.toMatchObject({
      code: 'INVALID_INPUT',
      message: expect.stringContaining('桌面应用'),
    });
  });
});
