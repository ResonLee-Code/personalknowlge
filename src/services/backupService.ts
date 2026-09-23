import type {
  BackupInfo,
  BackupValidation,
  ExportResult,
  RestoreReport,
} from '@/types';

import { call } from './api';

export const backupService = {
  create: (targetDir?: string, note?: string) =>
    call<BackupInfo>('backup_create', { input: { targetDir: targetDir ?? null, note: note ?? null } }),

  list: () => call<BackupInfo[]>('backup_list'),

  validate: (path: string) => call<BackupValidation>('backup_validate', { path }),

  restore: (archivePath: string, targetDir: string) =>
    call<RestoreReport>('backup_restore', { input: { archivePath, targetDir } }),

  exportMarkdown: (targetDir: string, includeAttachments = true) =>
    call<ExportResult>('export_markdown', { input: { targetDir, includeAttachments } }),

  directory: () => call<string>('backup_directory'),
};
