import { type ClassValue, clsx } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** 合并 Tailwind 类名（shadcn/ui 约定）。 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** 等待若干毫秒。 */
export function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** 截断字符串。 */
export function truncate(value: string, max = 80) {
  if (value.length <= max) return value;
  return `${value.slice(0, max)}…`;
}

/** 按错误码判断是否属于「未初始化」错误。 */
export function isNotInitialized(error: unknown) {
  return (error as { code?: string } | null)?.code === 'NOT_INITIALIZED';
}
