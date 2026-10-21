import type {
  ResolveSyncConflictInput,
  RestoreSyncHistoryInput,
  SyncConflictDetail,
  SyncConflictSummary,
  SyncHistoryEntry,
  SyncMergePreview,
  SyncPreview,
  SyncRunInput,
  SyncRunReport,
  SyncSettings,
  UpdateSyncSettingsInput,
} from '@/types';

import { call } from './api';

export const syncService = {
  settings: () => call<SyncSettings>('sync_settings_get'),
  updateSettings: (input: UpdateSyncSettingsInput) =>
    call<SyncSettings>('sync_settings_update', { input }),
  preview: (passphrase: string) => call<SyncPreview>('sync_preview', { passphrase }),
  run: (input: SyncRunInput) => call<SyncRunReport>('sync_run', { input }),
  conflicts: () => call<SyncConflictSummary[]>('sync_conflicts'),
  conflictDetail: (conflictId: string) =>
    call<SyncConflictDetail>('sync_conflict_detail', { conflictId }),
  conflictMergePreview: (conflictId: string) =>
    call<SyncMergePreview>('sync_conflict_merge_preview', { conflictId }),
  resolveConflict: (input: ResolveSyncConflictInput) =>
    call<void>('sync_conflict_resolve', { input }),
  history: () => call<SyncHistoryEntry[]>('sync_history'),
  restoreHistory: (input: RestoreSyncHistoryInput) =>
    call<string>('sync_history_restore', { input }),
};
