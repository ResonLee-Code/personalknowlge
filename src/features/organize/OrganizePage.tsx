import {
  Check,
  CirclePause,
  CirclePlay,
  Loader2,
  RotateCcw,
  Sparkles,
  Square,
  TriangleAlert,
  Undo2,
  X,
} from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { toast } from 'sonner';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import { FailureDetail } from '@/components/ui/failure-detail';
import { Label } from '@/components/ui/input';
import { EVENTS, ROUTES } from '@/lib/constants';
import { errorMessage, formatDateTime } from '@/lib/format';
import { organizeService } from '@/services/organizeService';
import { aiService } from '@/services/aiService';
import { subscribe } from '@/services/api';
import { useNotesStore } from '@/stores/notesStore';
import type {
  OrganizeJob,
  OrganizeJobProgress,
  OrganizeSuggestion,
  OrganizeSuggestionKind,
  OrganizeSuggestionStatus,
  AiPolicy,
} from '@/types';

const KIND_LABEL: Record<OrganizeSuggestionKind, string> = {
  summary: '摘要',
  tags: '标签',
  relations: '关联',
  duplicate: '重复',
};

function suggestionAdditions(suggestion: OrganizeSuggestion): string[] {
  switch (suggestion.payload.type) {
    case 'summary':
      return [
        '## 摘要',
        '',
        suggestion.payload.summary,
        ...suggestion.payload.bullets.map((bullet) => `- ${bullet}`),
      ];
    case 'tags':
      return suggestion.payload.tags.map((tag) => `+ 标签：${tag.tag} — ${tag.reason}`);
    case 'relations':
      return suggestion.payload.relations.map(
        (relation) => `+ ${relation.relationType} → ${relation.targetTitle} — ${relation.reason}`,
      );
    case 'duplicate':
      return suggestion.payload.candidates.map(
        (candidate) => `近重复：${candidate.title}（${Math.round(candidate.score * 100)}%）`,
      );
  }
}

/**
 * Organize P0：建议先进入待确认队列，接受才写 Markdown，接受后可字节级撤销。
 */
export function OrganizePage() {
  const detail = useNotesStore((state) => state.detail);
  const items = useNotesStore((state) => state.items);
  const selection = useNotesStore((state) => state.selection);
  const refreshNotes = useNotesStore((state) => state.refresh);
  const [kind, setKind] = useState<OrganizeSuggestionKind>('summary');
  const [status, setStatus] = useState<OrganizeSuggestionStatus>('pending');
  const [suggestions, setSuggestions] = useState<OrganizeSuggestion[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [jobs, setJobs] = useState<OrganizeJob[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [jobBusy, setJobBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [policy, setPolicy] = useState<AiPolicy | null>(null);
  const [confirmOutbound, setConfirmOutbound] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [nextSuggestions, nextJobs] = await Promise.all([
        organizeService.list(status, null, 300),
        organizeService.jobs(20),
      ]);
      setSuggestions(nextSuggestions);
      setSelectedIds((current) => {
        const valid = new Set(
          nextSuggestions.filter((item) => item.status === 'pending').map((item) => item.id),
        );
        return new Set([...current].filter((id) => valid.has(id)));
      });
      setJobs(nextJobs);
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }, [status]);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([organizeService.list(status, null, 300), organizeService.jobs(20)])
      .then(([nextSuggestions, nextJobs]) => {
        if (cancelled) return;
        setSuggestions(nextSuggestions);
        setSelectedIds(new Set());
        setJobs(nextJobs);
        setError(null);
      })
      .catch((caught) => {
        if (!cancelled) setError(errorMessage(caught));
      });
    return () => {
      cancelled = true;
    };
  }, [status]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void subscribe<OrganizeJobProgress>(EVENTS.organizeProgress, (progress) => {
      if (disposed) return;
      setJobs((current) =>
        current.map((job) =>
          job.id === progress.jobId
            ? {
                ...job,
                status: progress.status,
                total: progress.total,
                completed: progress.completed,
                failed: progress.failed,
              }
            : job,
        ),
      );
    })
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => undefined);
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    void aiService.policy().then((value) => {
      if (!cancelled) setPolicy(value);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const action = async (key: string, task: () => Promise<unknown>, success: string) => {
    if (busy) return;
    setBusy(key);
    setError(null);
    try {
      await task();
      await Promise.all([refresh(), refreshNotes()]);
      toast.success(success);
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(null);
    }
  };

  const singleTarget =
    selection.length > 0 ? null : detail?.note.status === 'active' ? detail.note.id : null;
  const batchTargets =
    selection.length > 0
      ? selection
      : items.filter((item) => item.status === 'active').map((item) => item.id);
  const needsPerCallConfirm = Boolean(
    policy?.confirmEachOutbound &&
    ((policy.remoteModelCompose && policy.remoteModelAuthorizedAt && policy.remoteModelKeySet) ||
      (policy.vectorEnabled && policy.vectorAuthorizedAt && policy.remoteEmbeddingModel)),
  );

  const generateSingle = () => {
    if (!singleTarget) return;
    void action(
      'generate-one',
      () => organizeService.generate(singleTarget, kind, confirmOutbound),
      `已生成${KIND_LABEL[kind]}建议`,
    );
  };

  const runBatch = () => {
    if (batchTargets.length === 0) return;
    void action(
      'generate-batch',
      async () => {
        const job = await organizeService.createJob(kind, batchTargets, confirmOutbound);
        setJobs((current) => [job, ...current]);
        return organizeService.runJob(job.id);
      },
      `批量任务已完成（${batchTargets.length} 篇）`,
    );
  };

  const acceptSelected = async () => {
    if (selectedIds.size === 0 || busy) return;
    setBusy('accept-selected');
    setError(null);
    try {
      const report = await organizeService.acceptMany([...selectedIds]);
      await Promise.all([refresh(), refreshNotes()]);
      if (report.failures.length === 0) {
        toast.success(`已接受 ${report.succeeded} 条建议`);
      } else {
        toast.warning(
          `已接受 ${report.succeeded}/${report.requested} 条，${report.failures.length} 条失败`,
        );
      }
      setSelectedIds(new Set());
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(null);
    }
  };

  const jobAction = async (job: OrganizeJob, verb: 'pause' | 'resume' | 'cancel' | 'retry') => {
    if (jobBusy) return;
    const fn = {
      pause: organizeService.pauseJob,
      resume: organizeService.resumeJob,
      cancel: organizeService.cancelJob,
      retry: organizeService.retryJob,
    }[verb];
    setJobBusy(`${verb}-${job.id}`);
    setError(null);
    try {
      await fn(job.id);
      await Promise.all([refresh(), refreshNotes()]);
      toast.success(
        {
          pause: '任务已暂停',
          resume: '任务已继续',
          cancel: '任务已取消',
          retry: '任务已重试',
        }[verb],
      );
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setJobBusy(null);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="border-b border-[var(--border-subtle)] px-6 py-4">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <h1 className="flex items-center gap-2 text-lg font-semibold text-[var(--ink)]">
              <Sparkles className="h-4 w-4 text-[var(--brand)]" />
              整理建议
            </h1>
            <p className="mt-1 text-xs text-[var(--ink-subtle)]">
              AI 只生成建议；只有点击接受才写文件。接受后可一键撤销到原字节。
            </p>
          </div>
          <div className="flex flex-wrap items-end gap-2">
            <div className="space-y-1">
              <Label htmlFor="organize-kind">建议类型</Label>
              <select
                id="organize-kind"
                value={kind}
                onChange={(event) => setKind(event.target.value as OrganizeSuggestionKind)}
                className="h-9 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm"
              >
                <option value="summary">摘要</option>
                <option value="tags">标签</option>
                <option value="relations">关联</option>
                <option value="duplicate">重复检测</option>
              </select>
            </div>
            <Button
              variant="outline"
              disabled={!singleTarget || Boolean(busy)}
              onClick={generateSingle}
            >
              {busy === 'generate-one' ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Sparkles className="h-4 w-4" />
              )}
              生成当前笔记
            </Button>
            <Button disabled={batchTargets.length === 0 || Boolean(busy)} onClick={runBatch}>
              {busy === 'generate-batch' ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Sparkles className="h-4 w-4" />
              )}
              批量生成 {batchTargets.length} 篇
            </Button>
            <Button
              variant="outline"
              disabled={selectedIds.size === 0 || Boolean(busy)}
              onClick={() => void acceptSelected()}
            >
              {busy === 'accept-selected' ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Check className="h-4 w-4" />
              )}
              批量接受 {selectedIds.size}
            </Button>
          </div>
        </div>
        {error ? (
          <div className="mt-3 rounded-md border border-red-500/30 bg-red-500/10 p-2 text-xs text-red-700 dark:text-red-300">
            <FailureDetail summary="整理操作失败" detail={error} />
          </div>
        ) : null}
        {needsPerCallConfirm ? (
          <label className="mt-3 flex items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 p-2.5 text-xs text-[var(--ink-muted)]">
            <input
              type="checkbox"
              checked={confirmOutbound}
              onChange={(event) => setConfirmOutbound(event.target.checked)}
              className="mt-0.5 accent-[var(--brand)]"
            />
            <span>
              本次允许把当前笔记材料交给已授权的在线模型或嵌入端点；不勾选时对应远端档零请求并回退本地
              / 规则。
            </span>
          </label>
        ) : null}
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_320px]">
        <div className="min-h-0 overflow-y-auto px-6 py-4">
          <div className="mb-3 flex items-center justify-between gap-3">
            <div className="flex items-center gap-2">
              <Label htmlFor="organize-status">队列状态</Label>
              <select
                id="organize-status"
                value={status}
                onChange={(event) => setStatus(event.target.value as OrganizeSuggestionStatus)}
                className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
              >
                <option value="pending">待确认</option>
                <option value="accepted">已接受</option>
                <option value="rejected">已拒绝</option>
                <option value="superseded">已替换</option>
              </select>
            </div>
            <Button variant="ghost" size="sm" onClick={() => void refresh()}>
              刷新
            </Button>
          </div>

          {suggestions.length === 0 ? (
            <EmptyState
              icon={Sparkles}
              title="队列里没有建议"
              description="选择一篇笔记生成建议，或对当前范围执行批量整理。"
            />
          ) : (
            <div className="space-y-3">
              {suggestions.map((suggestion) => (
                <article
                  key={suggestion.id}
                  className="rounded-lg border border-[var(--border-subtle)] bg-[var(--surface)] p-4"
                >
                  <div className="flex flex-wrap items-center gap-2">
                    {suggestion.status === 'pending' ? (
                      <input
                        type="checkbox"
                        aria-label={`选择建议：${suggestion.noteTitle}`}
                        checked={selectedIds.has(suggestion.id)}
                        onChange={(event) =>
                          setSelectedIds((current) => {
                            const next = new Set(current);
                            if (event.target.checked) next.add(suggestion.id);
                            else next.delete(suggestion.id);
                            return next;
                          })
                        }
                        className="accent-[var(--brand)]"
                      />
                    ) : null}
                    <Badge variant="brand">{KIND_LABEL[suggestion.kind]}</Badge>
                    <Link
                      to={ROUTES.note(suggestion.noteId)}
                      className="min-w-0 flex-1 truncate text-sm font-medium hover:text-[var(--brand)]"
                    >
                      {suggestion.noteTitle}
                    </Link>
                    <Badge variant="outline">{suggestion.status}</Badge>
                  </div>

                  <div className="mt-3 rounded-md bg-[var(--surface-muted)] p-3 font-mono text-[11px] leading-relaxed">
                    {suggestionAdditions(suggestion).map((line, index) => (
                      <p
                        key={`${index}-${line}`}
                        className={
                          line.startsWith('+') || index === 0
                            ? 'text-emerald-700 dark:text-emerald-300'
                            : 'text-[var(--ink-muted)]'
                        }
                      >
                        {line || ' '}
                      </p>
                    ))}
                  </div>

                  <details className="mt-3 text-xs text-[var(--ink-muted)]">
                    <summary className="cursor-pointer">
                      证据 {suggestion.evidence.length} 条 · {suggestion.model} ·{' '}
                      {suggestion.promptVersion}
                    </summary>
                    <div className="mt-2 space-y-2">
                      {suggestion.evidence.map((evidence) => (
                        <div
                          key={`${evidence.noteId}-${evidence.charStart}`}
                          className="rounded border border-[var(--border-subtle)] p-2 text-[11px]"
                        >
                          {evidence.heading ? (
                            <p className="mb-1 text-[var(--brand)]">{evidence.heading}</p>
                          ) : null}
                          <p className="line-clamp-4 whitespace-pre-wrap">{evidence.snippet}</p>
                        </div>
                      ))}
                    </div>
                  </details>

                  <div className="mt-3 flex flex-wrap items-center gap-2">
                    <span className="text-[10px] text-[var(--ink-subtle)]">
                      {formatDateTime(suggestion.updatedAt)}
                    </span>
                    {suggestion.status === 'pending' ? (
                      <>
                        <Button
                          size="sm"
                          className="ml-auto"
                          disabled={Boolean(busy)}
                          onClick={() =>
                            void action(
                              `accept-${suggestion.id}`,
                              () => organizeService.accept(suggestion.id),
                              '建议已应用',
                            )
                          }
                        >
                          <Check className="h-3.5 w-3.5" />
                          接受
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          disabled={Boolean(busy)}
                          onClick={() =>
                            void action(
                              `reject-${suggestion.id}`,
                              () => organizeService.reject(suggestion.id),
                              '建议已拒绝',
                            )
                          }
                        >
                          <X className="h-3.5 w-3.5" />
                          拒绝
                        </Button>
                      </>
                    ) : null}
                    {suggestion.undoAvailable ? (
                      <Button
                        variant="outline"
                        size="sm"
                        className="ml-auto"
                        disabled={Boolean(busy)}
                        onClick={() =>
                          void action(
                            `undo-${suggestion.id}`,
                            () => organizeService.undo(suggestion.id),
                            '已恢复到接受前字节',
                          )
                        }
                      >
                        <Undo2 className="h-3.5 w-3.5" />
                        撤销
                      </Button>
                    ) : null}
                  </div>
                </article>
              ))}
            </div>
          )}
        </div>

        <aside className="min-h-0 overflow-y-auto border-l border-[var(--border-subtle)] bg-[var(--surface-muted)] p-4">
          <h2 className="text-sm font-semibold">后台任务</h2>
          <p className="mt-1 text-[11px] text-[var(--ink-subtle)]">
            状态持久化；退出应用后未完成的任务会暂停，下次打开可继续。
          </p>
          <div className="mt-3 space-y-2">
            {jobs.map((job) => {
              const percent =
                job.total > 0 ? Math.round(((job.completed + job.failed) / job.total) * 100) : 0;
              return (
                <div
                  key={job.id}
                  className="rounded-lg border border-[var(--border-subtle)] bg-[var(--surface)] p-3"
                >
                  <div className="flex items-center gap-2">
                    <span className="text-xs font-medium">{job.kind}</span>
                    <Badge variant="outline">{job.status}</Badge>
                  </div>
                  <p className="mt-1 text-[10px] text-[var(--ink-subtle)]">
                    {job.completed + job.failed}/{job.total} · 失败 {job.failed}
                  </p>
                  <div className="mt-2 h-1.5 overflow-hidden rounded bg-[var(--surface-muted)]">
                    <div className="h-full bg-[var(--brand)]" style={{ width: `${percent}%` }} />
                  </div>
                  <div className="mt-2 flex flex-wrap gap-1">
                    {job.status === 'running' || job.status === 'queued' ? (
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={Boolean(jobBusy)}
                        onClick={() => void jobAction(job, 'pause')}
                      >
                        <CirclePause className="h-3.5 w-3.5" />
                        暂停
                      </Button>
                    ) : null}
                    {job.status === 'paused' || job.status === 'failed' ? (
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={Boolean(jobBusy)}
                        onClick={() =>
                          void jobAction(job, job.status === 'failed' ? 'retry' : 'resume')
                        }
                      >
                        {job.status === 'failed' ? (
                          <RotateCcw className="h-3.5 w-3.5" />
                        ) : (
                          <CirclePlay className="h-3.5 w-3.5" />
                        )}
                        {job.status === 'failed' ? '重试' : '继续'}
                      </Button>
                    ) : null}
                    {['queued', 'running', 'paused'].includes(job.status) ? (
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={Boolean(jobBusy)}
                        onClick={() => void jobAction(job, 'cancel')}
                      >
                        <Square className="h-3.5 w-3.5" />
                        取消
                      </Button>
                    ) : null}
                  </div>
                </div>
              );
            })}
            {jobs.length === 0 ? (
              <p className="flex items-center gap-1.5 text-xs text-[var(--ink-subtle)]">
                <TriangleAlert className="h-3.5 w-3.5" />
                还没有批量任务
              </p>
            ) : null}
          </div>
        </aside>
      </div>
    </div>
  );
}
