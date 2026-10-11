import { beforeEach, describe, expect, it, vi } from 'vitest';

import { toast } from 'sonner';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import { workspaceService } from '@/services/workspaceService';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

/**
 * 回归：`workspace_open` / `workspace_switch` 返回的 `warnings`（例如
 * 「跳过无法读取的文件 xx.md」）此前只是存进 store，**界面上从来不显示**——
 * 用户把文件放进工作区却什么提示都没有。这里钉住「必须说出来」。
 */
describe('打开/切换工作区时的提醒', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const result = (warnings: string[]) => ({
    workspace: {
      id: 'ws_1',
      name: '测试库',
      rootPath: '/tmp/ws',
      createdAt: '2026-10-11T10:00:00+08:00',
      updatedAt: '2026-10-11T10:00:00+08:00',
    },
    stats: {
      noteCount: 1,
      inboxCount: 1,
      trashCount: 0,
      categoryCount: 0,
      tagCount: 0,
      attachmentCount: 0,
      totalWords: 1,
      updatedLast7d: 1,
      updatedLast30d: 1,
      pinnedCount: 0,
      activity: [],
    },
    settings: {
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
      ai: {
        enabled: false,
        network: 'localOnly',
        networkConfirmedAt: null,
        allowFetchBody: false,
        domainAllowlist: [],
        domainDenylist: [],
        localModelEndpoint: null,
        localModelName: null,
        localModelCompose: false,
        localModelScore: false,
        budget: { maxSubquestions: 5, maxPages: 12, maxDurationSecs: 480 },
        auditMaxEntries: 500,
      },
    },
    indexStatus: { indexedNotes: 1, totalNotes: 1, stale: false, lastIndexedAt: null },
    warnings,
  });

  it('打开工作区时把后端提醒弹出来', async () => {
    vi.spyOn(workspaceService, 'open').mockResolvedValue(
      result(['跳过无法读取的文件 00_Inbox/乱码.md']) as never,
    );

    await useWorkspaceStore.getState().openWorkspace('/tmp/ws');

    expect(vi.mocked(toast.warning)).toHaveBeenCalledWith(
      expect.stringContaining('跳过无法读取的文件'),
    );
  });

  it('没有提醒时保持安静（不发噪声提示）', async () => {
    vi.spyOn(workspaceService, 'open').mockResolvedValue(result([]) as never);

    await useWorkspaceStore.getState().openWorkspace('/tmp/ws');

    expect(vi.mocked(toast.warning)).not.toHaveBeenCalled();
  });
});
