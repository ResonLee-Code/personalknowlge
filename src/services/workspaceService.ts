import type {
  AppSettings,
  IndexStatus,
  ScanReport,
  UpdateSettingsInput,
  WorkspaceOpenResult,
  WorkspaceStats,
  WorkspaceSummary,
} from '@/types';

import { call } from './api';

export const workspaceService = {
  create: (name: string, root: string) =>
    call<WorkspaceOpenResult>('workspace_create', { name, root }),

  open: (root: string) => call<WorkspaceOpenResult>('workspace_open', { root }),

  switch: (id: string) => call<WorkspaceOpenResult>('workspace_switch', { id }),

  close: () => call<void>('workspace_close'),

  current: () => call<WorkspaceOpenResult | null>('workspace_get_current'),

  list: () => call<WorkspaceSummary[]>('workspace_list'),

  stats: () => call<WorkspaceStats>('workspace_stats'),

  scan: () => call<ScanReport>('workspace_scan'),

  rebuildIndex: () => call<IndexStatus>('workspace_rebuild_index'),

  forget: (id: string) => call<void>('workspace_forget', { id }),

  activeId: () => call<string | null>('workspace_active_id'),

  rootPath: () => call<string>('workspace_root_path'),

  getSettings: () => call<AppSettings>('settings_get'),

  updateSettings: (input: UpdateSettingsInput) => call<AppSettings>('settings_update', { input }),
};
