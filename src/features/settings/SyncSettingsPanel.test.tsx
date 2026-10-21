import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { SyncSettingsPanel } from '@/features/settings/SyncSettingsPanel';
import { syncService } from '@/services/syncService';
import type { SyncPreview, SyncSettings } from '@/types';

vi.mock('@/services/syncService', () => ({
  syncService: {
    settings: vi.fn(),
    updateSettings: vi.fn(),
    preview: vi.fn(),
    run: vi.fn(),
    conflicts: vi.fn(),
    conflictDetail: vi.fn(),
    conflictMergePreview: vi.fn(),
    resolveConflict: vi.fn(),
    history: vi.fn(),
    restoreHistory: vi.fn(),
  },
}));

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn(), warning: vi.fn() },
}));

const configured: SyncSettings = {
  enabled: true,
  provider: 'webdav',
  endpoint: 'https://dav.example.test/pkos',
  bucket: '',
  region: 'us-east-1',
  usernameSet: true,
  accessKeySet: false,
  syncNotes: true,
  syncAttachments: true,
  syncIndex: false,
  deviceId: 'device-a',
  pendingQueue: 0,
};

const preview: SyncPreview = {
  uploads: [
    {
      kind: 'upload',
      path: '00_Inbox/知识管理.md',
      sizeBytes: 2048,
      reason: '本地文件已更新',
    },
  ],
  downloads: [],
  localDeletes: [],
  remoteDeletes: [],
  conflicts: [],
  totalBytes: 2048,
  pendingQueue: 0,
  warnings: [],
};

describe('SyncSettingsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(syncService.settings).mockResolvedValue(configured);
    vi.mocked(syncService.conflicts).mockResolvedValue([]);
    vi.mocked(syncService.history).mockResolvedValue([]);
    vi.mocked(syncService.preview).mockResolvedValue(preview);
    vi.mocked(syncService.run).mockResolvedValue({
      preview,
      uploaded: 1,
      downloaded: 0,
      localDeleted: 0,
      remoteDeleted: 0,
      conflictsCreated: 0,
      queued: 0,
      durationMs: 12,
    });
  });

  it('dry-run 前禁用确认同步，预检后才允许写入', async () => {
    render(<SyncSettingsPanel />);
    await screen.findByTestId('sync-settings');
    const passphrase = screen.getByLabelText('同步口令');
    await userEvent.type(passphrase, 'correct horse battery staple');

    const confirm = screen.getByRole('button', { name: '确认并同步' });
    expect(confirm).toBeDisabled();
    await userEvent.click(screen.getByRole('button', { name: 'dry-run' }));

    expect(await screen.findByTestId('sync-preview')).toBeInTheDocument();
    expect(screen.getByText('00_Inbox/知识管理.md')).toBeInTheDocument();
    expect(confirm).toBeEnabled();
    await userEvent.click(confirm);
    await waitFor(() =>
      expect(syncService.run).toHaveBeenCalledWith({
        passphrase: 'correct horse battery staple',
        confirm: true,
      }),
    );
  });

  it('明确展示待重试队列且不回显已保存密码', async () => {
    vi.mocked(syncService.settings).mockResolvedValue({ ...configured, pendingQueue: 3 });
    render(<SyncSettingsPanel />);
    expect(await screen.findByText('待重试 3 项')).toBeInTheDocument();
    expect(screen.getByLabelText('密码（只写不读）')).toHaveValue('');
    expect(screen.queryByDisplayValue(/secret/i)).not.toBeInTheDocument();
  });
});
