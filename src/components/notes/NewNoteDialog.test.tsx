import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { NewNoteDialog } from '@/components/notes/NewNoteDialog';
import * as appServiceModule from '@/services/appService';
import { noteService } from '@/services/noteService';
import { templateService } from '@/services/templateService';
import { toast } from 'sonner';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';

vi.mock('@/services/templateService', () => ({
  templateService: { list: vi.fn(), render: vi.fn() },
}));

// 运行时可切换（默认浏览器预览；需要走桌面导入流程的用例自己打开）
const runtimeFlag = vi.hoisted(() => ({ tauri: false }));

vi.mock('@/services/api', async () => {
  const actual = await vi.importActual<typeof import('@/services/api')>('@/services/api');
  return { ...actual, isTauriRuntime: () => runtimeFlag.tauri };
});

vi.mock('@/services/appService', async () => {
  const actual = await vi.importActual<typeof import('@/services/appService')>('@/services/appService');
  return { ...actual, pickMarkdownFiles: vi.fn(), pickDirectory: vi.fn() };
});

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

const listTemplates = vi.mocked(templateService.list);
const renderTemplate = vi.mocked(templateService.render);

describe('NewNoteDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runtimeFlag.tauri = false;
    listTemplates.mockResolvedValue([]);
    useUiStore.setState({ newNoteDialogOpen: false });
  });

  it('提供手动创建与导入外部文件两种方式', async () => {
    useUiStore.setState({ newNoteDialogOpen: true });
    render(
      <MemoryRouter>
        <NewNoteDialog />
      </MemoryRouter>,
    );

    // 默认是手动创建
    expect(screen.getByLabelText('标题')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '创建笔记' })).toBeInTheDocument();

    // 切到导入页签
    await userEvent.click(screen.getByRole('button', { name: /导入外部文件/ }));
    expect(screen.getByRole('button', { name: '选择文件' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '选择文件夹' })).toBeInTheDocument();
    // 未选择文件时导入按钮不可用
    expect(screen.getByRole('button', { name: /导入到知识库/ })).toBeDisabled();
  });

  // 回归：导入成功的路径会直接跳到第一篇笔记，结果面板不会展示——
  // 「源文件不是 UTF-8，已按 GBK 解码」这类提醒必须在这条路径上也说出来，
  // 否则用户只知道「导入成功」，不知道自己的文件被换过编码。
  it('导入时把编码提醒带出来（成功路径用 toast，不静默）', async () => {
    runtimeFlag.tauri = true;
    vi.mocked(appServiceModule.pickMarkdownFiles).mockResolvedValue(['/tmp/gbk-note.txt']);
    vi.spyOn(noteService, 'importFiles').mockResolvedValue({
      imported: [{ sourcePath: '/tmp/gbk-note.txt', noteId: 'note_gbk', title: 'gbk-note', filePath: '00_Inbox/gbk-note.md' }],
      skipped: [],
      total: 1,
      failed: 0,
      attachments: 0,
      mode: 'plain',
      warnings: ['gbk-note.txt 不是 UTF-8，已按 GBK 解码'],
    });
    useNotesStore.setState({ refresh: vi.fn() } as never);

    useUiStore.setState({ newNoteDialogOpen: true });
    render(
      <MemoryRouter>
        <NewNoteDialog />
      </MemoryRouter>,
    );
    await userEvent.click(screen.getByRole('button', { name: /导入外部文件/ }));
    await userEvent.click(screen.getByRole('button', { name: '选择文件' }));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: /导入到知识库/ })).toBeEnabled(),
    );
    await userEvent.click(screen.getByRole('button', { name: /导入到知识库/ }));

    await waitFor(() =>
      expect(vi.mocked(toast.warning)).toHaveBeenCalledWith(
        expect.stringContaining('不是 UTF-8'),
      ),
    );
  });

  it('浏览器预览模式提示需在桌面应用中使用导入', async () => {
    useUiStore.setState({ newNoteDialogOpen: true });
    render(
      <MemoryRouter>
        <NewNoteDialog />
      </MemoryRouter>,
    );

    await userEvent.click(screen.getByRole('button', { name: /导入外部文件/ }));
    expect(screen.getByText(/浏览器预览模式无法读取本地文件/)).toBeInTheDocument();
  });

  it('导入页签提供格式选择（自动识别 / Obsidian / Notion）', async () => {
    useUiStore.setState({ newNoteDialogOpen: true });
    render(
      <MemoryRouter>
        <NewNoteDialog />
      </MemoryRouter>,
    );

    await userEvent.click(screen.getByRole('button', { name: /导入外部文件/ }));
    expect(screen.getByText('导入格式')).toBeInTheDocument();
    expect(screen.getByRole('option', { name: '自动识别（推荐）' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Obsidian 库' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Notion 导出' })).toBeInTheDocument();
  });

  it('有模板时可以选择模板并把渲染结果填进初始内容', async () => {
    listTemplates.mockResolvedValue([
      { name: '会议记录', fileName: '会议记录.md', title: '会议记录', bytes: 120 },
    ]);
    renderTemplate.mockResolvedValue({
      name: '会议记录',
      content: '# 周会\n\n- 时间：2026-10-07 09:00\n',
    });
    useUiStore.setState({ newNoteDialogOpen: true });

    render(
      <MemoryRouter>
        <NewNoteDialog />
      </MemoryRouter>,
    );

    await userEvent.type(screen.getByLabelText('标题'), '周会');
    await userEvent.selectOptions(await screen.findByLabelText('模板'), '会议记录');

    await vi.waitFor(() =>
      expect(screen.getByLabelText('初始内容（可选）')).toHaveValue(
        '# 周会\n\n- 时间：2026-10-07 09:00\n',
      ),
    );
    expect(renderTemplate).toHaveBeenCalledWith({ name: '会议记录', title: '周会' });
  });
});

describe('NewNoteDialog · 从网页剪藏（C1）', () => {
  it('抓取后展示标题/站点/预览，并可剪藏为新笔记（含来源元数据）', async () => {
    const { clipService } = await import('@/services/clipService');
    const clip = vi.spyOn(clipService, 'clip').mockResolvedValue({
      url: 'example.com/a',
      finalUrl: 'https://example.com/a',
      title: '一篇好文章',
      site: 'example.com',
      markdown: '# 一篇好文章\n\n正文第一段。',
      wordCount: 12,
      fetchedAt: '2026-10-10T12:00:00+08:00',
      bytes: 100,
    });
    const { noteService } = await import('@/services/noteService');
    const create = vi.spyOn(useNotesStore.getState(), 'create').mockResolvedValue({
      note: { id: 'note_clip', title: '一篇好文章' },
      content: '',
      categoryPath: '',
      tags: [],
    } as never);
    const setSource = vi.spyOn(noteService, 'setSource').mockResolvedValue({} as never);

    useUiStore.setState({ newNoteDialogOpen: true });
    render(
      <MemoryRouter>
        <NewNoteDialog />
      </MemoryRouter>,
    );

    await userEvent.click(screen.getByRole('button', { name: /从网页剪藏/ }));
    await userEvent.type(screen.getByTestId('clip-url'), 'example.com/a');
    await userEvent.click(screen.getByTestId('clip-fetch'));

    await waitFor(() => expect(clip).toHaveBeenCalledWith('example.com/a'));
    expect(await screen.findByTestId('clip-meta')).toHaveTextContent('example.com');
    expect(screen.getByTestId('clip-preview')).toHaveTextContent('一篇好文章');

    await userEvent.click(screen.getByTestId('clip-create'));
    await waitFor(() => expect(create).toHaveBeenCalled());
    const payload = create.mock.calls[0]![0];
    expect(payload.title).toBe('一篇好文章');
    expect(payload.initialContent).toContain('> 来源：https://example.com/a');
    expect(payload.initialContent).toContain('# 一篇好文章');
    await waitFor(() =>
      expect(setSource).toHaveBeenCalledWith({
        id: 'note_clip',
        sourceType: 'web',
        sourceRef: 'https://example.com/a',
      }),
    );
    create.mockRestore();
  });

  it('抓取失败时给出错误提示且无法创建', async () => {
    const { clipService } = await import('@/services/clipService');
    vi.spyOn(clipService, 'clip').mockRejectedValue({ code: 'INVALID_INPUT', message: '抓取失败' });

    useUiStore.setState({ newNoteDialogOpen: true });
    render(
      <MemoryRouter>
        <NewNoteDialog />
      </MemoryRouter>,
    );

    await userEvent.click(screen.getByRole('button', { name: /从网页剪藏/ }));
    await userEvent.type(screen.getByTestId('clip-url'), 'bad.example');
    await userEvent.click(screen.getByTestId('clip-fetch'));

    await waitFor(() => expect(screen.queryByTestId('clip-preview')).not.toBeInTheDocument());
    expect(screen.getByTestId('clip-create')).toBeDisabled();
  });

});
