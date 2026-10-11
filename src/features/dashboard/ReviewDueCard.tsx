import { CalendarClock } from 'lucide-react';

import { cn } from '@/lib/utils';
import type { ReviewDueNote } from '@/types';

/**
 * 待复核卡片（V1.2.1 阶段 D4 / N1 的 `valid_until` 生命周期）。
 *
 * 只在**确实有**待复核笔记时出现：已过期的排在前面（红色），
 * 即将到期的标注剩余天数（琥珀色）。数据来自 `note_due_for_review`。
 */
export function ReviewDueCard({
  items,
  error,
  onRetry,
  onOpen,
}: {
  items: ReviewDueNote[];
  /** 读取失败的原因：**不能**让失败显示成「没有待复核」 */
  error?: string | null;
  onRetry?: () => void;
  onOpen: (noteId: string) => void;
}) {
  if (error) {
    return (
      <section
        data-testid="review-due-error"
        className="flex flex-wrap items-center gap-2 rounded-xl border border-amber-500/40 bg-amber-500/10 p-4 text-sm text-amber-900 dark:text-amber-200"
      >
        <CalendarClock className="h-4 w-4" />
        <span>待复核读取失败：{error}</span>
        {onRetry ? (
          <button
            type="button"
            data-testid="review-due-retry"
            className="rounded border border-amber-500/50 px-2 py-0.5 text-xs hover:bg-amber-500/10"
            onClick={onRetry}
          >
            重试
          </button>
        ) : null}
      </section>
    );
  }
  if (items.length === 0) return null;

  const overdueCount = items.filter((item) => item.overdueDays > 0).length;

  return (
    <section
      data-testid="review-due-card"
      className="rounded-xl border border-[var(--border-subtle)] p-4"
    >
      <div className="flex items-center gap-2">
        <CalendarClock className="h-4 w-4 text-[var(--brand)]" />
        <h2 className="text-sm font-medium text-[var(--ink)]">待复核</h2>
        <span className="text-xs text-[var(--ink-subtle)]">
          {items.length} 篇（其中已过期 {overdueCount} 篇）
        </span>
      </div>
      <ul className="mt-3 flex flex-col gap-1.5">
        {items.map((item) => {
          const overdue = item.overdueDays > 0;
          const label = overdue
            ? `已过期 ${item.overdueDays} 天`
            : `还有 ${Math.abs(item.overdueDays)} 天到期`;
          return (
            <li key={item.noteId} className="flex items-center gap-2 text-sm">
              <span
                className={cn(
                  'shrink-0 rounded px-1.5 py-0.5 text-[11px]',
                  overdue
                    ? 'bg-rose-500/15 text-rose-700 dark:text-rose-300'
                    : 'bg-amber-500/15 text-amber-700 dark:text-amber-300',
                )}
              >
                {label}
              </span>
              <button
                type="button"
                className="min-w-0 flex-1 truncate text-left text-[var(--ink)] hover:underline"
                onClick={() => onOpen(item.noteId)}
              >
                {item.title}
              </button>
              <span className="shrink-0 text-xs text-[var(--ink-subtle)]">
                {item.categoryPath || 'Inbox'} · {item.validUntil}
              </span>
            </li>
          );
        })}
      </ul>
      <p className="mt-3 text-[11px] text-[var(--ink-subtle)]">
        时效来自笔记的 <code>valid_until</code>（可在笔记的「来源与时效」里修改）；默认显示已过期与
        14 天内到期的笔记。
      </p>
    </section>
  );
}
