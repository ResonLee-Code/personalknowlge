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
  const icon = (() => {
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
      className={[
        'hidden items-center gap-1 rounded-full px-2 py-0.5 text-[11px] sm:flex',
        status === 'error'
          ? 'bg-red-500/10 text-red-500'
          : status === 'dirty'
            ? 'bg-amber-500/10 text-amber-600'
            : 'bg-[var(--surface-muted)] text-[var(--ink-subtle)]',
      ].join(' ')}
    >
      {icon}
      {LABELS[status]}
    </span>
  );
}
