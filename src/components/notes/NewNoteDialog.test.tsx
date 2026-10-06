import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it } from 'vitest';

import { NewNoteDialog } from '@/components/notes/NewNoteDialog';
import { useUiStore } from '@/stores/uiStore';

describe('NewNoteDialog', () => {
  beforeEach(() => {
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
});
