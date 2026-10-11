import { AlertTriangle, Check, Loader2 } from 'lucide-react';

import type { SaveStatus } from '@/stores/notesStore';

const LABELS: Record<SaveStatus, string> = {
  idle: '已同步',
  dirty: '未保存',
  saving: '保存中',
  saved: '已保存',
  error: '保存失败',
};

/** 编辑器保存状态（设计文档 16.5）。 */
export function SaveStatusBadge({ status, detail }: { status: SaveStatus; detail?: string | null }) {
  // 「保存成功但索引没更新」是一种**降级**：正文写盘了，但检索里找不到这篇。
  // 以前它和正常保存长得一模一样（绿勾 + 已保存），原因只藏在悬停提示里——
  // 用户不会去悬停，于是以为一切正常。这里让它显式可见。
  const degraded = status === 'saved' && Boolean(detail?.trim());

  const icon = (() => {
    if (degraded) return <AlertTriangle className="h-3 w-3" />;
    switch (status) {
      case 'saving':
        return <Loader2 className="h-3 w-3 animate-spin" />;
      case 'saved':
        return <Check className="h-3 w-3" />;
      case 'error':
        return <AlertTriangle className="h-3 w-3" />;
      default:
        return null;
    }
  })();

  return (
    <span
      title={detail ?? undefined}
      data-testid="save-status-badge"
      data-degraded={degraded ? 'true' : undefined}
      className={[
        'hidden items-center gap-1 rounded-full px-2 py-0.5 text-[11px] sm:flex',
        status === 'error'
          ? 'bg-red-500/10 text-red-500'
          : degraded
            ? 'bg-amber-500/10 text-amber-600'
          : status === 'dirty'
            ? 'bg-amber-500/10 text-amber-600'
            : 'bg-[var(--surface-muted)] text-[var(--ink-subtle)]',
      ].join(' ')}
    >
      {icon}
      {degraded ? '已保存（检索索引待重建）' : LABELS[status]}
    </span>
  );
}
