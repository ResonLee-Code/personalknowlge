import { format, formatDistanceToNow, isValid, parseISO } from 'date-fns';
import { zhCN } from 'date-fns/locale';

/** 解析后端返回的 ISO 8601 时间。 */
export function parseDate(value?: string | null): Date | null {
  if (!value) return null;
  try {
    const date = parseISO(value);
    return isValid(date) ? date : null;
  } catch {
    return null;
  }
}

/** `2026-09-23 12:00` 形式。 */
export function formatDateTime(value?: string | null): string {
  const date = parseDate(value);
  if (!date) return '—';
  return format(date, 'yyyy-MM-dd HH:mm');
}

/** `09-23 12:00` 形式（列表用）。 */
export function formatShortDate(value?: string | null): string {
  const date = parseDate(value);
  if (!date) return '—';
  const now = new Date();
  if (date.getFullYear() === now.getFullYear()) {
    return format(date, 'MM-dd HH:mm');
  }
  return format(date, 'yyyy-MM-dd');
}

/** 相对时间：`3 分钟前`。 */
export function formatRelative(value?: string | null): string {
  const date = parseDate(value);
  if (!date) return '—';
  return formatDistanceToNow(date, { addSuffix: true, locale: zhCN });
}

/** 文件大小。 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB'];
  const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
  const value = bytes / 1024 ** index;
  return `${value.toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

/** 数字紧凑显示。 */
export function formatCount(value: number): string {
  if (value >= 10000) return `${(value / 10000).toFixed(1)} 万`;
  return String(value);
}

/** 错误提示文案：优先使用后端 message。 */
export function errorMessage(error: unknown): string {
  if (typeof error === 'string') return error;
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === 'string' && message.length > 0) return message;
  }
  return '操作失败，请稍后重试。';
}
