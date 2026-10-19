import { ChevronDown, ChevronRight, Link2, Plus, RefreshCw, Unlink2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { Skeleton } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { reportFailure } from '@/lib/actionFeedback';
import { ROUTES } from '@/lib/constants';
import { errorMessage, formatRelative } from '@/lib/format';
import { cn } from '@/lib/utils';
import { linkService } from '@/services/linkService';
import { useNotesStore } from '@/stores/notesStore';
import type { Backlink, OutgoingLink } from '@/types';

interface BacklinkPanelProps {
  noteId: string;
  /** 保存 / 重命名等写入操作后递增，用于主动刷新链接数据 */
  refreshKey?: number;
  /**
   * 「本篇未解析链接」是否展开（可选受控，2026-10-19）。
   *
   * 编辑器底部的摘要栏里有一个「未解析 N」入口；它把整块信息区展开的同一次
   * 也把这个值设为 true，让面板直接露出列表，用户不用点第二次。
   */
  unresolvedOpen?: boolean;
  onUnresolvedOpenChange?: (open: boolean) => void;
}

/**
 * 编辑器底部「反向链接」面板（`docs/PLAN_V1.1.md` M1-3 / F2、F3）。
 *
 * - 上半部分：谁引用了这篇笔记，可点击跳转到来源；
 * - 下半部分：本篇里还没解析的 `[[…]]`，可一键创建目标笔记并自动建立连接。
 */
export function BacklinkPanel({
  noteId,
  refreshKey = 0,
  unresolvedOpen: controlledUnresolvedOpen,
  onUnresolvedOpenChange,
}: BacklinkPanelProps) {
  const navigate = useNavigate();
  const [open, setOpen] = useState(true);
  /**
   * 「本篇未解析链接」默认**收起**（2026-10-19）。
   *
   * 背景：非全屏窗口里，这 9 条未解析链接（每条一行 + 创建按钮）会把正文编辑/预览
   * 挤到只剩两百多像素。它们不是每次都看，收起后用一个带数量的入口提示即可；
   * 需要处理时点开（展开后仍受面板自身的高度上限约束、可滚动）。
   */
  const [localUnresolvedOpen, setLocalUnresolvedOpen] = useState(false);
  const unresolvedOpen = controlledUnresolvedOpen ?? localUnresolvedOpen;
  const unresolvedRef = useRef<HTMLDivElement | null>(null);
  const setUnresolvedOpen = (next: boolean | ((value: boolean) => boolean)) => {
    const value = typeof next === 'function' ? next(unresolvedOpen) : next;
    setLocalUnresolvedOpen(value);
    onUnresolvedOpenChange?.(value);
  };
  /**
   * 从底部摘要栏点「未解析 N」时，列表在 42% 限高区里可能正好在视野外；
   * 展开后把它滚进可视区，避免「点了没反应」的错觉。
   */
  useEffect(() => {
    // jsdom 没实现 scrollIntoView（单元测试会跑），这里按可选方法调用
    if (unresolvedOpen) unresolvedRef.current?.scrollIntoView?.({ block: 'nearest' });
  }, [unresolvedOpen]);
  const [refreshing, setRefreshing] = useState(false);
  /** 已加载完成的笔记 id，用于避免切换笔记时短暂显示上一篇的数据 */
  const [loadedFor, setLoadedFor] = useState<string | null>(null);
  const [backlinks, setBacklinks] = useState<Backlink[]>([]);
  const [outgoing, setOutgoing] = useState<OutgoingLink[]>([]);
  const [creating, setCreating] = useState<string | null>(null);
  /** 加载失败的原因：不打断编辑，但**不能**把失败显示成「还没有笔记引用」 */
  const [loadError, setLoadError] = useState<string | null>(null);

  /** 供手动刷新与「创建笔记」后复用最新一次加载逻辑 */
  const loadRef = useRef<() => Promise<void>>(async () => {});

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      // 链接是派生数据，读取失败不该打断编辑；但**必须说出来**——
      // 以前失败被吞成空列表，面板显示「还没有笔记引用这篇」，是在撒谎。
      try {
        const [nextBacklinks, nextOutgoing] = await Promise.all([
          linkService.backlinks(noteId),
          linkService.outgoing(noteId),
        ]);
        if (cancelled) return;
        setBacklinks(nextBacklinks);
        setOutgoing(nextOutgoing);
        setLoadError(null);
      } catch (error) {
        if (cancelled) return;
        console.warn('[links] 加载反向链接失败', errorMessage(error));
        setBacklinks([]);
        setOutgoing([]);
        setLoadError(errorMessage(error));
      }
      if (cancelled) return;
      setLoadedFor(noteId);
    };
    loadRef.current = run;
    void run();
    return () => {
      cancelled = true;
    };
  }, [noteId, refreshKey]);

  const loaded = loadedFor === noteId;
  const unresolved = outgoing.filter((link) => !link.targetExists && link.linkType !== 'url');

  const createNote = async (targetTitle: string) => {
    setCreating(targetTitle);
    try {
      const created = await useNotesStore.getState().create({ title: targetTitle });
      if (!created) {
        toast.error('创建笔记失败');
        return;
      }
      toast.success(`已创建「${created.note.title}」并建立链接`);
      await loadRef.current();
    } catch (error) {
      reportFailure('创建关联笔记失败', errorMessage(error));
    } finally {
      setCreating(null);
    }
  };

  const refresh = async () => {
    setRefreshing(true);
    try {
      await loadRef.current();
    } finally {
      setRefreshing(false);
    }
  };

  return (
    <section
      data-testid="backlink-panel"
      className="shrink-0 border-t border-[var(--border-subtle)] bg-[var(--surface)]"
    >
      <header className="flex items-center gap-2 px-3 py-2">
        <button
          type="button"
          className="flex items-center gap-1 text-xs font-medium text-[var(--ink)]"
          aria-expanded={open}
          onClick={() => setOpen((value) => !value)}
        >
          {open ? (
            <ChevronDown className="h-3.5 w-3.5 text-[var(--ink-subtle)]" />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 text-[var(--ink-subtle)]" />
          )}
          <Link2 className="h-3.5 w-3.5 text-[var(--brand)]" />
          反向链接
        </button>
        {loadError ? (
          <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[11px] text-amber-600">
            读取失败
          </span>
        ) : (
          <span className="rounded-full bg-[var(--surface-muted)] px-2 py-0.5 text-[11px] text-[var(--ink-subtle)]">
            被 {backlinks.length} 篇引用
          </span>
        )}
        {unresolved.length > 0 ? (
          <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[11px] text-amber-600">
            未解析 {unresolved.length}
          </span>
        ) : null}
        <Button
          size="icon"
          variant="ghost"
          className="ml-auto h-6 w-6"
          title="刷新反向链接"
          onClick={() => void refresh()}
        >
          <RefreshCw className={cn('h-3.5 w-3.5', refreshing && 'animate-spin')} />
        </Button>
      </header>

      {open ? (
        <div className="max-h-56 space-y-3 overflow-auto px-3 pb-3">
          {!loaded ? (
            <div className="space-y-2">
              <Skeleton className="h-9 w-full" />
              <Skeleton className="h-9 w-2/3" />
            </div>
          ) : loadError ? (
            <p
              data-testid="backlink-load-error"
              className="flex flex-wrap items-center gap-2 text-xs text-amber-600"
            >
              读取反向链接失败：{loadError}
              <button
                type="button"
                data-testid="backlink-load-retry"
                className="rounded border border-amber-500/50 px-1.5 py-0.5 hover:bg-amber-500/10"
                onClick={() => void refresh()}
              >
                重试
              </button>
            </p>
          ) : backlinks.length === 0 ? (
            <p className="text-xs leading-relaxed text-[var(--ink-subtle)]">
              还没有笔记引用这篇笔记。在其它笔记里写 [[标题]] 即可建立连接。
            </p>
          ) : (
            <ul className="space-y-1">
              {backlinks.map((backlink, index) => (
                <li key={`${backlink.noteId}-${index}`}>
                  <button
                    type="button"
                    data-testid="backlink-item"
                    className="w-full rounded-md px-2 py-1.5 text-left transition-colors hover:bg-[var(--surface-muted)]"
                    onClick={() => navigate(ROUTES.note(backlink.noteId))}
                  >
                    <span className="flex items-center gap-1.5">
                      <Link2 className="h-3 w-3 shrink-0 text-[var(--brand)]" />
                      <span className="min-w-0 flex-1 truncate text-xs font-medium text-[var(--ink)]">
                        {backlink.title}
                      </span>
                      <span className="shrink-0 text-[10px] text-[var(--ink-subtle)]">
                        {formatRelative(backlink.updatedAt)}
                      </span>
                    </span>
                    {backlink.context ? (
                      <span className="mt-0.5 block truncate pl-4 text-[11px] text-[var(--ink-subtle)]">
                        {backlink.context}
                      </span>
                    ) : null}
                  </button>
                </li>
              ))}
            </ul>
          )}

          {unresolved.length > 0 ? (
            <div
              ref={unresolvedRef}
              className="space-y-1 border-t border-dashed border-[var(--border-subtle)] pt-2"
            >
              <button
                type="button"
                data-testid="unresolved-toggle"
                aria-expanded={unresolvedOpen}
                className="flex w-full items-center gap-1 rounded text-[11px] font-medium text-amber-600 hover:text-amber-700"
                onClick={() => setUnresolvedOpen((value) => !value)}
              >
                {unresolvedOpen ? (
                  <ChevronDown className="h-3 w-3" />
                ) : (
                  <ChevronRight className="h-3 w-3" />
                )}
                <Unlink2 className="h-3 w-3" />
                本篇未解析链接
                <span className="rounded-full bg-amber-500/10 px-1.5 text-[10px]">
                  {unresolved.length}
                </span>
              </button>
              {unresolvedOpen ? (
                <ul className="space-y-1">
                  {unresolved.map((link, index) => (
                    <li
                      key={`${link.targetTitle}-${index}`}
                      data-testid="unresolved-item"
                      className="flex items-center gap-2 rounded-md px-2 py-1"
                    >
                      <span className="min-w-0 flex-1 truncate text-xs text-[var(--ink-muted)]">
                        [[{link.targetTitle}]]
                      </span>
                      <Button
                        size="sm"
                        variant="secondary"
                        aria-label={`创建笔记：${link.targetTitle}`}
                        disabled={creating === link.targetTitle}
                        onClick={() => void createNote(link.targetTitle)}
                      >
                        <Plus className="h-3 w-3" />
                        创建
                      </Button>
                    </li>
                  ))}
                </ul>
              ) : null}
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
