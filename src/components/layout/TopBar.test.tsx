import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { TopBar } from '@/components/layout/TopBar';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { Workspace } from '@/types';

vi.mock('@/components/editor/SaveStatusBadge', () => ({
  SaveStatusBadge: () => <span>已保存</span>,
}));
vi.mock('@/components/notes/NoteActions', () => ({ NoteActions: () => null }));

const workspace: Workspace = {
  id: 'workspace_1',
  name: '测试知识库',
  rootPath: '/tmp/pks',
  createdAt: '2026-10-25T01:00:00+08:00',
  updatedAt: '2026-10-25T01:00:00+08:00',
};

describe('TopBar', () => {
  beforeEach(() => {
    useWorkspaceStore.setState({ workspace });
    useNotesStore.setState({ saveStatus: 'saved', detail: null });
    useUiStore.setState({
      theme: 'light',
      commandPaletteOpen: false,
      askPanelOpen: false,
    });
  });

  it('显示工作区并提供搜索、Ask、主题与设置入口', async () => {
    render(
      <MemoryRouter initialEntries={['/discover']}>
        <TopBar />
      </MemoryRouter>,
    );

    expect(screen.getByText('测试知识库')).toBeInTheDocument();
    expect(screen.getByText('/tmp/pks')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /搜索笔记内容/ }));
    expect(useUiStore.getState().commandPaletteOpen).toBe(true);
    await userEvent.click(screen.getByRole('button', { name: '问知识库' }));
    expect(useUiStore.getState().askPanelOpen).toBe(true);
    await userEvent.click(screen.getByTitle('切换为深色主题'));
    expect(useUiStore.getState().theme).toBe('dark');
    await userEvent.click(screen.getByTitle('设置'));
  });
});
