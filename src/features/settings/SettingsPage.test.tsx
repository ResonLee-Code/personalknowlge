import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
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
  outlinePlacement: 'list',
  autoOutlineOnSelect: false,
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
    // 设置页现在分节显示（左侧导航，2026-10-16）：备份相关的行在「备份与导出」那一节里
    await userEvent.click(screen.getByTestId('settings-nav-backup'));

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
    // 设置页现在分节显示（左侧导航，2026-10-16）：备份相关的行在「备份与导出」那一节里
    await userEvent.click(screen.getByTestId('settings-nav-backup'));

    // 空态文案现在是「暂无备份文件。点上方『立即备份』创建后…」（B2 加了入口说明），
    // 这条测试只关心「确实显示了空态、且不是错误态」。
    expect(await screen.findByTestId('backup-empty')).toHaveTextContent('暂无备份文件');
    expect(screen.queryByTestId('backup-list-error')).not.toBeInTheDocument();
  });

  /**
   * Track B2（2026-10-11 真机验收）：**空态要说清「校验 / 恢复」从哪来**。
   *
   * 「校验 / 恢复」是逐条渲染在每张备份卡片下方的按钮——没有备份时它们根本不存在。
   * 验收时你和我都因此找不到这两个入口（各自花了几分钟），所以空态要主动说明。
   */
  it('空态里说明「创建备份后可校验 / 恢复」（这两个按钮只在有备份时出现）', async () => {
    vi.spyOn(backupService, 'list').mockResolvedValue([]);

    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );
    // 设置页现在分节显示（左侧导航，2026-10-16）：备份相关的行在「备份与导出」那一节里
    await userEvent.click(screen.getByTestId('settings-nav-backup'));

    const empty = await screen.findByTestId('backup-empty');
    expect(empty).toHaveTextContent('暂无备份文件');
    expect(empty).toHaveTextContent(/校验/);
    expect(empty).toHaveTextContent(/恢复/);
  });
});
