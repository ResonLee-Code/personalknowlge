import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { SettingsPage } from '@/features/settings/SettingsPage';
import { backupService } from '@/services/backupService';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { AppSettings } from '@/types';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

const settings: AppSettings = {
  autosaveEnabled: true,
  autosaveDelayMs: 1000,
  editorTheme: 'light',
  fontSize: 15,
  defaultNewLocation: 'inbox',
  backupDir: null,
  editorMode: 'split',
  editorSplitRatio: 0.5,
  sidebarWidth: 280,
  listWidth: 320,
  listSort: 'updatedDesc',
  lastOpenedNoteId: null,
  lastCategoryId: null,
};

/**
 * 回归：备份列表读取失败以前被 `.catch(() => setBackups([]))` 吞掉，
 * 界面显示成「暂无备份文件」——用户会以为自己的备份都没了。
 */
describe('SettingsPage · 备份列表', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useWorkspaceStore.setState({ settings, workspace: null });
  });

  it('读取备份列表失败时明确报错，而不是显示成「暂无备份文件」', async () => {
    vi.spyOn(backupService, 'list').mockRejectedValue({
      code: 'IO_ERROR',
      message: '备份目录不可读',
    });

    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    expect(await screen.findByTestId('backup-list-error')).toHaveTextContent('备份目录不可读');
    expect(screen.queryByText('暂无备份文件。')).not.toBeInTheDocument();
  });

  it('确实没有备份文件时才显示空态', async () => {
    vi.spyOn(backupService, 'list').mockResolvedValue([]);

    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    expect(await screen.findByText('暂无备份文件。')).toBeInTheDocument();
    expect(screen.queryByTestId('backup-list-error')).not.toBeInTheDocument();
  });
});
