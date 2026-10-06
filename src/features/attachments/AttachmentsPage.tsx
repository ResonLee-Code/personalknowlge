import { ExternalLink, HardDrive, ImageOff, RefreshCw, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { Skeleton } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import { ROUTES } from '@/lib/constants';
import { errorMessage, formatBytes, formatDateTime } from '@/lib/format';
import { attachmentService } from '@/services/attachmentService';
import type { AttachmentEntry, AttachmentListReport } from '@/types';

/** 附件管理（V1.1.2）：清点粘贴/导入产生的附件，查看引用并清理。 */
export function AttachmentsPage() {
  const navigate = useNavigate();
  const [report, setReport] = useState<AttachmentListReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const loadRef = useRef<() => Promise<void>>(async () => {});

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      const next = await attachmentService.list().catch((cause: unknown) => {
        if (!cancelled) setError(errorMessage(cause));
        return null;
      });
      if (!cancelled && next) {
        setReport(next);
        setError(null);
      }
    };
    loadRef.current = run;
    void run();
    return () => {
      cancelled = true;
    };
  }, []);

  const reload = useCallback(async () => {
    await loadRef.current();
  }, []);

  const groups = useMemo(() => {
    const items = report?.items ?? [];
    const byMonth = new Map<string, AttachmentEntry[]>();
    for (const item of items) {
      const list = byMonth.get(item.month);
      if (list) list.push(item);
      else byMonth.set(item.month, [item]);
    }
    return [...byMonth.entries()].sort((left, right) => right[0].localeCompare(left[0]));
  }, [report]);

  const remove = async (entry: AttachmentEntry) => {
    const warning =
      entry.references.length > 0
        ? `\n\n注意：${entry.references.length} 篇笔记正在引用它（${entry.references
            .map((item) => item.title)
            .slice(0, 3)
            .join('、')}${entry.references.length > 3 ? ' 等' : ''}），删除后这些引用会失效。`
        : '';
    if (!window.confirm(`确定删除附件「${entry.fileName}」？此操作不可恢复。${warning}`)) {
      return;
    }
    setBusy(entry.relativePath);
    try {
      const freed = await attachmentService.remove(entry.relativePath);
      toast.success(`已删除附件，释放 ${formatBytes(freed)}`);
      await reload();
    } catch (cause) {
      toast.error(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const orphanCount = (report?.items.length ?? 0) - (report?.referencedCount ?? 0);

  return (
    <div className="flex h-full min-h-0 w-full flex-col">
      <header className="flex flex-wrap items-center gap-3 border-b border-[var(--border-subtle)] px-4 py-2.5">
        <HardDrive className="h-4 w-4 text-[var(--brand)]" />
        <h1 className="text-sm font-semibold text-[var(--ink)]">附件</h1>
        {report ? (
          <span className="text-[11px] text-[var(--ink-subtle)]">
            {report.items.length} 个文件 · 共 {formatBytes(report.totalBytes)} · 被引用{' '}
            {report.referencedCount} · 未引用 {orphanCount}
          </span>
        ) : null}
        <Button
          size="icon"
          variant="ghost"
          title="刷新"
          className="ml-auto"
          onClick={() => void reload()}
        >
          <RefreshCw className="h-4 w-4" />
        </Button>
      </header>

      <div className="min-h-0 flex-1 overflow-auto">
        {report === null && !error ? (
          <div className="space-y-2 p-4">
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-2/3" />
          </div>
        ) : error ? (
          <EmptyState
            icon={ImageOff}
            title="无法读取附件"
            description={error}
            action={
              <Button variant="secondary" onClick={() => void reload()}>
                重试
              </Button>
            }
            className="h-full"
          />
        ) : (report?.items.length ?? 0) === 0 ? (
          <EmptyState
            icon={ImageOff}
            title="还没有附件"
            description="在编辑器里粘贴或拖入图片会自动保存到 attachments/YYYY-MM/，之后可以在这里清点与清理。"
            className="h-full"
          />
        ) : (
          <div className="mx-auto w-full max-w-5xl space-y-4 p-4">
            {groups.map(([month, items]) => (
              <section key={month}>
                <h2 className="mb-1.5 text-xs font-medium text-[var(--ink-subtle)]">
                  {month === '未分月' ? month : `${month.slice(0, 4)} 年 ${month.slice(5)} 月`}
                  <span className="ml-1.5">{items.length}</span>
                </h2>
                <ul className="divide-y divide-[var(--border-subtle)] rounded-lg border border-[var(--border-subtle)]">
                  {items.map((entry) => (
                    <li
                      key={entry.relativePath}
                      data-testid="attachment-row"
                      className="flex flex-wrap items-center gap-3 px-3 py-2"
                    >
                      <span
                        className="min-w-0 flex-1 truncate text-xs text-[var(--ink)]"
                        title={entry.relativePath}
                      >
                        {entry.fileName}
                      </span>
                      <span className="text-[11px] tabular-nums text-[var(--ink-subtle)]">
                        {formatBytes(entry.bytes)}
                      </span>
                      <span className="hidden text-[11px] text-[var(--ink-subtle)] sm:block">
                        {formatDateTime(entry.modifiedAt)}
                      </span>
                      {entry.references.length > 0 ? (
                        <span className="flex items-center gap-1">
                          {entry.references.slice(0, 2).map((reference) => (
                            <button
                              key={reference.noteId}
                              type="button"
                              className="flex max-w-[140px] items-center gap-0.5 truncate rounded bg-[var(--brand-soft)] px-1.5 py-0.5 text-[10px] text-[var(--brand)]"
                              title={`打开「${reference.title}」`}
                              onClick={() => navigate(ROUTES.note(reference.noteId))}
                            >
                              <ExternalLink className="h-2.5 w-2.5 shrink-0" />
                              <span className="truncate">{reference.title}</span>
                            </button>
                          ))}
                          {entry.references.length > 2 ? (
                            <span className="text-[10px] text-[var(--ink-subtle)]">
                              +{entry.references.length - 2}
                            </span>
                          ) : null}
                        </span>
                      ) : (
                        <span className="rounded bg-[var(--surface-muted)] px-1.5 py-0.5 text-[10px] text-[var(--ink-subtle)]">
                          未引用
                        </span>
                      )}
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-6 w-6"
                        title="删除附件"
                        aria-label={`删除附件：${entry.fileName}`}
                        disabled={busy === entry.relativePath}
                        onClick={() => void remove(entry)}
                      >
                        <Trash2 className="h-3 w-3" />
                      </Button>
                    </li>
                  ))}
                </ul>
              </section>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
