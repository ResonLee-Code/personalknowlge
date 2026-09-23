import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

import type { AppError } from '@/types';

import { handleMockInvoke, isMockRuntime, subscribeMockEvent } from './mock';

/** 是否运行在 Tauri 运行时中。 */
export function isTauriRuntime(): boolean {
  return !isMockRuntime();
}

function normalizeError(error: unknown): AppError {
  if (typeof error === 'string') {
    try {
      const parsed = JSON.parse(error) as AppError;
      if (parsed && typeof parsed.code === 'string') return parsed;
    } catch {
      return { code: 'INTERNAL_ERROR', message: error };
    }
  }
  if (error && typeof error === 'object') {
    const candidate = error as Partial<AppError>;
    if (typeof candidate.code === 'string' && typeof candidate.message === 'string') {
      return { code: candidate.code, message: candidate.message, details: candidate.details };
    }
  }
  return { code: 'INTERNAL_ERROR', message: '发生未知错误，请查看日志。' };
}

/**
 * 统一调用后端命令。
 *
 * 浏览器模式（`pnpm dev` 且未运行在 Tauri 中）会走内存实现，
 * 便于在不启动桌面外壳的情况下预览界面。
 */
export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isMockRuntime()) {
    try {
      return (await handleMockInvoke(command, args)) as T;
    } catch (error) {
      throw normalizeError(error);
    }
  }
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw normalizeError(error);
  }
}

/** 订阅后端事件；返回取消订阅函数。 */
export async function subscribe<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  if (isMockRuntime()) {
    return subscribeMockEvent(event, handler as (payload: unknown) => void);
  }
  return listen<T>(event, (message) => handler(message.payload));
}

export type { UnlistenFn };
