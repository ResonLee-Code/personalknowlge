import { AlertTriangle, Check, Loader2 } from 'lucide-react';

import { FailureDetail } from '@/components/ui/failure-detail';
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
  /**
   * 保存失败的原因（Track B1，2026-10-11 真机验收）：
   * 以前失败只显示「保存失败」，原因挂在 `title` 上——**要悬停才看得到，等于没给原因**。
   * 与上面「索引待重建」同一类问题，这里一并显式显示（完整原文仍放 `title`）。
   */
  const failureReason = status === 'error' ? detail?.trim() || null : null;

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
      {degraded ? (
        '已保存（检索索引待重建）'
      ) : failureReason ? (
        <>
          保存失败
          <span
            data-testid="save-error-reason"
            className="max-w-[260px] truncate font-normal"
          >
            ：{failureReason}
          </span>
          {/*
            Track F2：头部空间有限，长原因只能截断——截断掉的部分以前**没有地方能看到**
            （完整原文只挂在 `title` 上，要悬停，而且悬停也看不全）。这里给一个能点开的入口。
          */}
          <FailureDetail detail={failureReason} testId="save-error" />
        </>
      ) : (
        LABELS[status]
      )}
    </span>
  );
}
