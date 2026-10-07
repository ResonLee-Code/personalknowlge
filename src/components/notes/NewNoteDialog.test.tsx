import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { NewNoteDialog } from '@/components/notes/NewNoteDialog';
import { templateService } from '@/services/templateService';
import { useUiStore } from '@/stores/uiStore';

vi.mock('@/services/templateService', () => ({
  templateService: { list: vi.fn(), render: vi.fn() },
}));

const listTemplates = vi.mocked(templateService.list);
const renderTemplate = vi.mocked(templateService.render);

describe('NewNoteDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
