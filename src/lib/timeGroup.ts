/** 按时间分组（回收站用）：今天 / 昨天 / 近 7 天 / 更早。 */

export type RecencyBucket = 'today' | 'yesterday' | 'last7' | 'earlier';

export const RECENCY_LABELS: Record<RecencyBucket, string> = {
  today: '今天',
  yesterday: '昨天',
  last7: '近 7 天',
  earlier: '更早',
};

export const RECENCY_ORDER: RecencyBucket[] = ['today', 'yesterday', 'last7', 'earlier'];

export function recencyBucket(value: string | Date | null | undefined, now = new Date()): RecencyBucket {
  const date = value instanceof Date ? value : value ? new Date(value) : null;
  if (!date || Number.isNaN(date.getTime())) return 'earlier';
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const timestamp = date.getTime();
  if (timestamp >= startOfToday) return 'today';
  if (timestamp >= startOfToday - 24 * 60 * 60 * 1000) return 'yesterday';
  if (timestamp >= startOfToday - 7 * 24 * 60 * 60 * 1000) return 'last7';
  return 'earlier';
}

/** 按时间分桶，空桶会被跳过；桶内保持输入顺序（列表本身已排序）。 */
export function groupByRecency<T>(
  items: T[],
  timestampOf: (item: T) => string | Date | null | undefined,
  now = new Date(),
): Array<{ bucket: RecencyBucket; label: string; items: T[] }> {
  const buckets = new Map<RecencyBucket, T[]>();
  for (const item of items) {
    const bucket = recencyBucket(timestampOf(item), now);
    const list = buckets.get(bucket);
    if (list) list.push(item);
    else buckets.set(bucket, [item]);
  }
  return RECENCY_ORDER.filter((bucket) => buckets.has(bucket)).map((bucket) => ({
    bucket,
    label: RECENCY_LABELS[bucket],
    items: buckets.get(bucket)!,
  }));
}
