import type { AppInfo } from '@/types';

import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { openPath as openWithSystem } from '@tauri-apps/plugin-opener';

import { call } from './api';

export const appService = {
  info: () => call<AppInfo>('app_info'),

  logPath: () => call<string | null>('app_log_path'),

  ready: () => call<boolean>('app_ready'),
};

/** 在系统文件管理器中打开路径（需要 opener 插件）。 */
export async function openPath(path: string): Promise<void> {
  await openWithSystem(path);
}

/** 选择目录（需要 dialog 插件）。 */
export async function pickDirectory(title: string): Promise<string | null> {
  const selected = await openDialog({ directory: true, multiple: false, title });
  if (!selected || Array.isArray(selected)) return null;
  return selected;
}
