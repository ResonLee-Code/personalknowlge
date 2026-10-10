/** 知识时效与来源可靠性的展示逻辑（V1.1.2 N1）。 */

export type FreshnessState = 'unknown' | 'valid' | 'expiring' | 'expired';

/** 解析 `valid_until`（支持 `2026-12-31` 与完整 ISO）。 */
export function parseValidUntil(value?: string | null): Date | null {
  if (!value || !value.trim()) return null;
  const text = value.trim();
  const date = /^\d{4}-\d{2}-\d{2}$/.test(text) ? new Date(`${text}T23:59:59`) : new Date(text);
  return Number.isNaN(date.getTime()) ? null : date;
}

/**
 * 时效状态：
 * - `expired`：已过期；
 * - `expiring`：30 天内到期（提醒用户复核）；
 * - `valid`：仍在有效期内；
 * - `unknown`：没有设置时效。
 */
export function freshnessState(
  value?: string | null,
  now: Date = new Date(),
): FreshnessState {
  const until = parseValidUntil(value);
  if (!until) return 'unknown';
  if (until.getTime() < now.getTime()) return 'expired';
  const thirtyDays = 30 * 24 * 60 * 60 * 1000;
  return until.getTime() - now.getTime() <= thirtyDays ? 'expiring' : 'valid';
}

/** 来源可靠性 1–5 → 星标（0 或未设置时返回空串）。 */
export function reliabilityStars(level?: number | null): string {
  if (!level || level < 1) return '';
  const bounded = Math.min(5, Math.round(level));
  return '★'.repeat(bounded) + '☆'.repeat(5 - bounded);
}

/** 来源类型的展示名（未知类型原样显示）。 */
export const SOURCE_TYPE_LABELS: Record<string, string> = {
  official: '官方文档',
  book: '书籍',
  paper: '论文',
  experiment: '实测',
  personal: '个人推测',
  conversation: '交流',
  web: '网页剪藏',
  ai: 'AI 生成',
  research: '调研报告',
  unknown: '未标注',
};

export function sourceTypeLabel(value?: string | null): string {
  if (!value || !value.trim()) return '';
  const key = value.trim().toLowerCase();
  return SOURCE_TYPE_LABELS[key] ?? value.trim();
}
