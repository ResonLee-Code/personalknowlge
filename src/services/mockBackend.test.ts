import { beforeEach, describe, expect, it, vi } from 'vitest';

import { clipService } from '@/services/clipService';
import { linkService } from '@/services/linkService';
import { graphService } from '@/services/graphService';
import { noteService } from '@/services/noteService';
import { categoryService } from '@/services/categoryService';
import { searchService } from '@/services/searchService';
import { tagService } from '@/services/tagService';
import { templateService } from '@/services/templateService';
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

  it('批量移动 / 打标签 / 删除（浏览器预览后端）', async () => {
    await workspaceService.create('测试库', '/tmp/mock');
    const first = await noteService.create({ title: '批量一' });
    const second = await noteService.create({ title: '批量二' });
    const ids = [first.note.id, second.note.id, first.note.id];

    const category = await categoryService.create('批量目录', null);
    const moved = await noteService.moveMany(ids, category.id);
    expect(moved.succeeded).toBe(2);
    expect(moved.failures).toHaveLength(0);
    expect((await noteService.get(first.note.id)).note.categoryId).toBe(category.id);

    const tagged = await noteService.tagMany([first.note.id], ['整理'], 'add');
    expect(tagged.succeeded).toBe(1);
    expect((await noteService.get(first.note.id)).tags.map((tag) => tag.name)).toContain('整理');
    await noteService.tagMany([first.note.id], ['整理'], 'remove');
    expect((await noteService.get(first.note.id)).tags.map((tag) => tag.name)).not.toContain('整理');

    const deleted = await noteService.deleteMany(ids);
    expect(deleted.succeeded).toBe(2);
    expect(await noteService.listDeleted()).toHaveLength(2);

    const failure = await noteService.moveMany(['note_missing'], null);
    expect(failure.succeeded).toBe(0);
    expect(failure.failures[0]?.noteId).toBe('note_missing');
  });

  it('解析双向链接：反向链接、出链、未解析与计数', async () => {
    await workspaceService.create('测试库', '/tmp/mock');
    const target = await noteService.create({ title: '索引设计' });
    const source = await noteService.create({
      title: '查询优化',
      initialContent: '参考 [[索引设计|倒排索引]]，另见 [[还没写的笔记]]。',
    });

    const backlinks = await linkService.backlinks(target.note.id);
    expect(backlinks).toHaveLength(1);
    expect(backlinks[0]).toMatchObject({
      noteId: source.note.id,
      title: '查询优化',
    });
    expect(backlinks[0]?.context).toContain('[[索引设计|倒排索引]]');

    const outgoing = await linkService.outgoing(source.note.id);
    expect(outgoing).toHaveLength(2);
    expect(outgoing[0]).toMatchObject({
      targetTitle: '索引设计',
      targetNoteId: target.note.id,
      targetExists: true,
      linkType: 'wikilink',
    });
    expect(outgoing[1]).toMatchObject({ targetTitle: '还没写的笔记', targetExists: false });

    const counts = await linkService.counts(target.note.id);
    expect(counts).toEqual({ backlinks: 1, outgoing: 0, unresolved: 0 });
    const sourceCounts = await linkService.counts(source.note.id);
    expect(sourceCounts).toEqual({ backlinks: 0, outgoing: 2, unresolved: 1 });

    const unresolved = await linkService.unresolved();
    expect(unresolved).toHaveLength(1);
    expect(unresolved[0]).toMatchObject({
      sourceNoteId: source.note.id,
      sourceTitle: '查询优化',
      targetTitle: '还没写的笔记',
    });
  });

  it('忽略代码块内的 [[…]]，并支持锚点', async () => {
    await workspaceService.create('测试库', '/tmp/mock');
    const note = await noteService.create({
      title: '解析规则',
      initialContent: [
        '正文链接 [[目标笔记#小节]]',
        '',
        '```rust',
        '// [[代码块里的链接不算]]',
        '```',
        '',
        '行内代码 `[[也不算]]` 结束。',
      ].join('\n'),
    });

    const outgoing = await linkService.outgoing(note.note.id);
    expect(outgoing).toHaveLength(1);
    expect(outgoing[0]).toMatchObject({ targetTitle: '目标笔记', anchor: '小节' });
  });

  it('重建链接索引返回统计信息', async () => {
    await workspaceService.create('测试库', '/tmp/mock');
    await noteService.create({ title: 'A' });
    await noteService.create({ title: 'B', initialContent: '链接 [[A]] 与 [[缺失]]' });

    const report = await linkService.rebuild();
    expect(report.links).toBe(2);
    expect(report.resolved).toBe(1);
    expect(report.unresolved).toBe(1);
    // 创建工作区会预置一篇欢迎笔记，因此是 3 篇
    expect(report.scannedNotes).toBe(3);
  });

  it('按链接生成图谱节点与边，并支持聚焦与孤立节点开关', async () => {
    await workspaceService.create('测试库', '/tmp/mock');
    const alpha = await noteService.create({ title: '图谱甲' });
    const beta = await noteService.create({ title: '图谱乙' });
    const lonely = await noteService.create({ title: '图谱孤立' });
    await noteService.save({
      id: alpha.note.id,
      content: '指向 [[图谱乙]]',
      baseContentHash: alpha.note.contentHash,
    });

    const graph = await graphService.get();
    const find = (id: string) => graph.nodes.find((node) => node.id === id);
    expect(find(alpha.note.id)?.degree).toBe(1);
    expect(find(beta.note.id)?.degree).toBe(1);
    expect(find(lonely.note.id)?.orphan).toBe(true);
    expect(graph.edges).toEqual([
      { source: alpha.note.id, target: beta.note.id, linkType: 'wikilink', weight: 1 },
    ]);

    const focused = await graphService.get({ focusNoteId: alpha.note.id, hops: 1 });
    expect(focused.nodes.map((node) => node.id).sort()).toEqual(
      [alpha.note.id, beta.note.id].sort(),
    );

    const withoutOrphans = await graphService.get({ includeOrphans: false });
    expect(withoutOrphans.nodes.every((node) => !node.orphan)).toBe(true);
  });
});

describe('浏览器预览后端的版本历史（与 Rust 端行为对齐）', () => {
  beforeEach(async () => {
    localStorage.clear();
    await workspaceService.create('历史库', '/tmp/mock-history');
  });

  it('保存留档 → 列表 → 还原，且还原会更新内容哈希', async () => {
    const created = await noteService.create({
      title: '历史笔记',
      initialContent: '第一版正文',
      categoryId: null,
    });
    const id = created.note.id;
    const beforeHash = created.note.contentHash;

    await noteService.save({ id, content: '第二版正文', baseContentHash: beforeHash ?? null });
    const entries = await noteService.historyList(id);
    expect(entries).toHaveLength(1);

    const snapshot = await noteService.historyRead(id, entries[0]!.entryId);
    expect(snapshot.body).toContain('第一版正文');

    const restored = await noteService.historyRestore(id, entries[0]!.entryId);
    expect(restored.content).toContain('第一版正文');
    const detail = await noteService.get(id);
    expect(detail.content).toContain('第一版正文');
    expect(detail.note.contentHash).not.toBe(beforeHash);
  });

  it('内容没变时不重复留档（避免自动保存刷出重复快照）', async () => {
    const created = await noteService.create({
      title: '重复保存',
      initialContent: '正文',
      categoryId: null,
    });
    const id = created.note.id;
    await noteService.save({ id, content: '改过了', baseContentHash: null });
    await noteService.save({ id, content: '改过了', baseContentHash: null });
    await noteService.save({ id, content: '改过了', baseContentHash: null });
    expect(await noteService.historyList(id)).toHaveLength(1);
  });
});

describe('浏览器预览后端的来源与时效（N1 对齐，C1 剪藏依赖它）', () => {
  beforeEach(async () => {
    localStorage.clear();
    await workspaceService.create('来源库', '/tmp/mock-source');
  });

  it('note_set_source 写入来源字段并可读回；空串按清除处理', async () => {
    const created = await noteService.create({ title: '来源笔记', categoryId: null });
    const id = created.note.id;

    const updated = await noteService.setSource({
      id,
      sourceType: 'web',
      sourceRef: 'https://example.com/a',
      sourceReliability: 4,
      validUntil: '2027-01-01',
    });
    expect(updated.note.sourceType).toBe('web');
    expect(updated.note.sourceRef).toBe('https://example.com/a');
    expect(updated.note.sourceReliability).toBe(4);
    expect(updated.note.validUntil).toBe('2027-01-01');

    const cleared = await noteService.setSource({ id, sourceType: '  ', sourceRef: '' });
    expect(cleared.note.sourceType).toBeNull();
    expect(cleared.note.sourceRef).toBeNull();
  });

  it('note_clip_url 返回可用的剪藏结果（浏览器预览端为模拟实现）', async () => {
    const page = await clipService.clip('example.com/post');
    expect(page.finalUrl).toBe('https://example.com/post');
    expect(page.title).toContain('example.com');
    expect(page.markdown).toContain('# ');
    expect(page.wordCount).toBeGreaterThan(0);
    await expect(clipService.clip('   ')).rejects.toMatchObject({ code: 'INVALID_INPUT' });
  });

  it('note_due_for_review 只返回已过期与窗口内到期的笔记（D4）', async () => {
    await workspaceService.create('待复核库', '/tmp/mock-review');
    const iso = (offsetDays: number) => {
      const date = new Date();
      date.setDate(date.getDate() + offsetDays);
      return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(
        date.getDate(),
      ).padStart(2, '0')}`;
    };

    const overdue = await noteService.create({ title: '过期的手册' });
    await noteService.setSource({ id: overdue.note.id, sourceType: 'official', validUntil: iso(-7) });
    const soon = await noteService.create({ title: '快要复核' });
    await noteService.setSource({ id: soon.note.id, sourceType: 'official', validUntil: iso(3) });
    const far = await noteService.create({ title: '很久以后' });
    await noteService.setSource({ id: far.note.id, sourceType: 'official', validUntil: iso(200) });
    await noteService.create({ title: '没有时效字段' });

    const due = await noteService.dueForReview(14);
    expect(due.map((item) => item.noteId)).toEqual([overdue.note.id, soon.note.id]);
    expect(due[0]!.overdueDays).toBe(7);
    expect(due[1]!.overdueDays).toBe(-3);

    expect((await noteService.dueForReview(365)).length).toBe(3);
    expect((await noteService.dueForReview(0)).length).toBe(1);
  });
});

// 与 Rust 后端同规则：分类不存在时必须报错，而不是把笔记挂到一个悬空分类上。
// （浏览器预览曾被用来「验证界面」，校验不一致会让预览里的行为与桌面端分叉。）
describe('浏览器预览后端 · 分类引用校验（与 Rust 对齐）', () => {
  it('新建 / 单篇移动 / 批量移动 指向不存在的分类都会被拒绝', async () => {
    await workspaceService.create('分类引用校验库', '/tmp/mock-category-ref');

    await expect(
      noteService.create({ title: '不该被创建', categoryId: 'cat_不存在' }),
    ).rejects.toMatchObject({ code: 'CATEGORY_NOT_FOUND' });

    const note = await noteService.create({ title: '移动目标' });
    await expect(
      noteService.move(note.note.id, 'cat_不存在'),
    ).rejects.toMatchObject({ code: 'CATEGORY_NOT_FOUND' });

    const report = await noteService.moveMany([note.note.id], 'cat_不存在');
    expect(report.succeeded).toBe(0);
    expect(report.failures[0]?.message).toContain('分类不存在');

    // 失败后归属不变（没有留下悬空分类）
    const reloaded = await noteService.get(note.note.id);
    expect(reloaded.note.categoryId).toBeNull();
  });
});

// 标签规范化与后端一致：去空 / 折叠空白 / 大小写不敏感唯一（FR-T01）。
// 预览端曾把 ' React ' 与 'react' 当成两个标签，界面上会出现桌面端不会有的重复标签。
describe('浏览器预览后端 · 标签规范化（与 Rust 对齐）', () => {
  it('设置标签时会去空、折叠空白并按大小写不敏感去重', async () => {
    await workspaceService.create('标签规范化库', '/tmp/mock-tag-normalize');
    const note = await noteService.create({ title: '标签测试' });

    const updated = await noteService.setTags(note.note.id, [' React ', 'react', '  ', '前端  框架']);
    expect(updated.tags.map((tag) => tag.name)).toEqual(['React', '前端 框架']);

    // 标签表里也只应有两行（不会再出现 'react' 的重复行）
    const all = await tagService.list();
    expect(all.filter((tag) => tag.name.toLowerCase() === 'react')).toHaveLength(1);
    expect(all.some((tag) => tag.name.trim() === '')).toBe(false);
  });

  it('批量打标签同样规范化；重命名撞名（不区分大小写）被拒绝', async () => {
    await workspaceService.create('标签批量库', '/tmp/mock-tag-batch');
    const note = await noteService.create({ title: '批量标签测试' });

    await noteService.tagMany([note.note.id], [' Alpha ', 'alpha'], 'add');
    const registered = await tagService.list();
    expect(registered.filter((tag) => tag.name.toLowerCase() === 'alpha')).toHaveLength(1);

    const beta = await tagService.create('Beta');
    await expect(tagService.rename(beta.id, 'ALPHA')).rejects.toMatchObject({
      code: 'CATEGORY_NAME_CONFLICT',
    });
    await expect(tagService.rename(beta.id, '   ')).rejects.toMatchObject({
      code: 'INVALID_INPUT',
    });
  });
});

// 重命名 / 永久删除的预览端行为要与 Rust 对齐：
// 重命名会改写文件名并重连指向旧标题的链接；永久删除后列表不可见、详情取不到；
// 对不存在的笔记做删除 / 永久删除必须报错（Rust 走 `note_or_err`），而不是静默成功。
describe('浏览器预览后端 · 重命名与永久删除（与 Rust 对齐）', () => {
  it('重命名会规范化标题、改写文件名并重连指向旧标题的链接', async () => {
    await workspaceService.create('重命名库', '/tmp/mock-rename');
    const target = await noteService.create({ title: '旧标题', initialContent: '目标正文' });
    const source = await noteService.create({
      title: '来源笔记',
      initialContent:
        '见 [[旧标题]]、[[旧标题#小节|别名]]，行内代码 `[[旧标题]]` 不改，另有 [[别的笔记]]。',
    });
    const originalPath = target.note.filePath;
    expect(originalPath).toMatch(/^00_Inbox\/\d{8}-\d{6}-旧标题\.md$/);

    const renamed = await noteService.rename(target.note.id, '  新   标题  ');
    expect(renamed.note.title).toBe('新 标题');
    expect(renamed.note.filePath).not.toBe(originalPath);
    expect(renamed.note.filePath).toMatch(/^00_Inbox\/\d{8}-\d{6}-新-标题\.md$/);

    const reloaded = await noteService.get(source.note.id);
    expect(reloaded.content).toContain('[[新 标题]]');
    expect(reloaded.content).toContain('[[新 标题#小节|别名]]');
    expect(reloaded.content).toContain('`[[旧标题]]`');
    expect(reloaded.content).toContain('[[别的笔记]]');

    // renameFile=false 时只改标题，文件名保持不动
    const keepFile = await noteService.rename(target.note.id, '只改标题', false);
    expect(keepFile.note.filePath).toBe(renamed.note.filePath);

    await expect(noteService.rename(target.note.id, '   ')).rejects.toMatchObject({
      code: 'INVALID_INPUT',
    });
    await expect(noteService.rename('note_不存在', '随便')).rejects.toMatchObject({
      code: 'NOTE_NOT_FOUND',
    });
  });

  it('永久删除后列表不可见、详情取不到；删除不存在的笔记报错', async () => {
    await workspaceService.create('永久删除库', '/tmp/mock-purge');
    const note = await noteService.create({ title: '待永久删除', initialContent: '正文' });
    await noteService.remove(note.note.id);
    expect(await noteService.listDeleted()).toHaveLength(1);

    await noteService.purge(note.note.id);
    expect(await noteService.listDeleted()).toHaveLength(0);
    await expect(noteService.get(note.note.id)).rejects.toMatchObject({
      code: 'NOTE_NOT_FOUND',
    });

    await expect(noteService.purge(note.note.id)).rejects.toMatchObject({
      code: 'NOTE_NOT_FOUND',
    });
    await expect(noteService.remove('note_不存在')).rejects.toMatchObject({
      code: 'NOTE_NOT_FOUND',
    });
  });

  it('模板另存为在预览端明确报「不支持」，而不是假装成功', async () => {
    await workspaceService.create('模板库', '/tmp/mock-template');
    await expect(
      templateService.save({ name: '我的模板', content: '# 正文' }),
    ).rejects.toMatchObject({ code: 'INVALID_INPUT' });
  });
});

// 相对路径要跟桌面端一致：笔记文件放在分类目录下（无分类回落到 00_Inbox），
// 移动分类会连同文件路径一起改；同一目录同一秒内撞名时追加序号，不共用路径。
describe('浏览器预览后端 · 相对路径与分类目录（与 Rust 对齐）', () => {
  it('新建按分类目录落位，移动分类会改路径，移回 Inbox 恢复', async () => {
    await workspaceService.create('目录库', '/tmp/mock-directory');
    const tech = await categoryService.create('技术文档', null);
    const database = await categoryService.create('数据库', tech.id);

    const note = await noteService.create({ title: '索引设计', categoryId: tech.id });
    expect(note.note.filePath.startsWith(`${tech.dirName}/`)).toBe(true);

    const nested = await noteService.create({ title: 'B+ 树', categoryId: database.id });
    expect(nested.note.filePath.startsWith(`${tech.dirName}/${database.dirName}/`)).toBe(true);

    const moved = await noteService.move(note.note.id, database.id);
    expect(moved.note.filePath.startsWith(`${tech.dirName}/${database.dirName}/`)).toBe(true);
    // 移动只换目录，文件名（时间戳 + 标题）保持不变
    expect(moved.note.filePath.split('/').pop()).toBe(note.note.filePath.split('/').pop());

    const inbox = await noteService.move(note.note.id, null);
    expect(inbox.note.filePath.startsWith('00_Inbox/')).toBe(true);
  });

  it('同一目录同一秒内撞名时追加序号，不会两条笔记共用一个路径', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-10-11T10:00:00Z'));
    try {
      await workspaceService.create('撞名库', '/tmp/mock-collide');
      const first = await noteService.create({ title: '重名笔记' });
      const second = await noteService.create({ title: '重名笔记' });
      expect(second.note.filePath).not.toBe(first.note.filePath);
      expect(second.note.filePath).toMatch(/-1\.md$/);
    } finally {
      vi.useRealTimers();
    }
  });

});

// 列表排序必须与后端 `NoteSort::order_by()` 同口径：置顶最前、次级键 `id`、
// 标题用 SQLite 的**码点序**（COLLATE NOCASE）而不是中文拼音序——否则预览里的顺序
// 与桌面端不一致，用户会以为是应用在随机排序。
describe('浏览器预览后端 · 列表排序（与 Rust 对齐）', () => {
  it('标题排序用码点序（不是拼音序），并用 id 兜底', async () => {
    await workspaceService.create('排序库', '/tmp/mock-sort');
    // 「波」(U+6CE2) 在码点上小于「阿」(U+963F)，但拼音序相反——用来区分两种口径
    const wave = await noteService.create({ title: '波' });
    const ah = await noteService.create({ title: '阿' });
    const beta = await noteService.create({ title: 'Beta' });
    const alpha = await noteService.create({ title: 'alpha' });

    const sorted = await noteService.list({ sort: 'titleAsc' });
    // 只比对本用例建的笔记：工作区初始化会放一篇「欢迎使用 Personal Knowledge OS」
    const titles = sorted.items
      .map((item) => item.title)
      .filter((title) => ['alpha', 'Beta', '波', '阿'].includes(title));
    // ASCII 大小写折叠：alpha < Beta；其余按码点：Beta(Z 前无) → 'B'(0x42) < 波(0x6CE2) < 阿(0x963F)
    expect(titles).toEqual(['alpha', 'Beta', '波', '阿']);
    expect(
      sorted.items
        .filter((item) => ['alpha', 'Beta', '波', '阿'].includes(item.title))
        .map((item) => item.id),
    ).toEqual([alpha.note.id, beta.note.id, wave.note.id, ah.note.id]);
  });

  it('置顶在最前，自定义顺序同层内按 order→updatedAt→id 兜底', async () => {
    await workspaceService.create('排序库2', '/tmp/mock-sort2');
    const first = await noteService.create({ title: '一号' });
    const second = await noteService.create({ title: '二号' });
    const third = await noteService.create({ title: '三号' });

    // 手动顺序：三号 → 一号 → 二号
    await noteService.reorder({ ids: [third.note.id, first.note.id, second.note.id] });
    // 把二号置顶：即便它排在最后，也必须跑到最前
    await noteService.setPinned(second.note.id, true);

    const only = (items: Array<{ title: string }>) =>
      items.map((item) => item.title).filter((title) => ['一号', '二号', '三号'].includes(title));

    const manual = await noteService.list({ sort: 'manual' });
    expect(only(manual.items)).toEqual(['二号', '三号', '一号']);

    const updated = await noteService.list({ sort: 'updatedDesc' });
    expect(updated.items[0]?.title).toBe('二号');
  });
});
