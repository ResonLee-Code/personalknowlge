import { Clock, History, Loader2, RotateCcw, Tag, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Skeleton } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import { reportFailure } from '@/lib/actionFeedback';
import { diffLines, diffStats } from '@/lib/diff';
import { errorMessage, formatDateTime } from '@/lib/format';
import { cn } from '@/lib/utils';
import { noteService } from '@/services/noteService';
import { useNotesStore } from '@/stores/notesStore';
import type { HistoryEntry } from '@/types';

/** 「当前版本」（右侧对比项的默认值，不是真实快照）。 */
const CURRENT = '__current__';

/**
 * 历史版本（对标语雀「查看历史版本」）：
 * 列出快照、对比任意两个版本（含「当前版本」）、还原到某个版本。
 */
export function HistoryDialog({
  noteId,
  currentContent,
  onClose,
  onRestored,
}: {
  noteId: string;
  /** 编辑器里的当前正文（用于「与当前版本对比」） */
  currentContent: string;
  onClose: () => void;
  onRestored?: () => void;
}) {
  const [entries, setEntries] = useState<HistoryEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [leftId, setLeftId] = useState<string | null>(null);
  const [rightId, setRightId] = useState<string>(CURRENT);
  const [contents, setContents] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  /** 「保存当前为版本」的版本名输入 */
  const [versionName, setVersionName] = useState('');
  /** 正在内联命名的快照 id 与其输入值 */
  const [namingId, setNamingId] = useState<string | null>(null);
  const [namingValue, setNamingValue] = useState('');

  /** 首次加载：直接内联在 effect 里（避免在 effect 中调用会同步 setState 的函数）。 */
  useEffect(() => {
    let cancelled = false;
    void noteService
      .historyList(noteId)
      .then((list) => {
        if (cancelled) return;
        setEntries(list);
        // 默认对比：最新快照 → 当前版本
        setLeftId(list[0]?.entryId ?? null);
        setRightId(CURRENT);
      })
      .catch((cause) => {
        if (cancelled) return;
        setError(errorMessage(cause));
        setEntries([]);
      });
    return () => {
      cancelled = true;
    };
  }, [noteId]);

  /** 还原 / 删除之后重新拉取（事件处理里调用，无 effect 限制）。 */
  const reload = async () => {
    try {
      const list = await noteService.historyList(noteId);
      setEntries(list);
      setLeftId(list[0]?.entryId ?? null);
    } catch (cause) {
      setError(errorMessage(cause));
    }
  };

  /** 按需拉取某个版本的内容（含「当前版本」直接用编辑器里的正文）。 */
  const ensureContent = useCallback(
    async (entryId: string | null): Promise<string> => {
      if (!entryId) return '';
      if (entryId === CURRENT) return currentContent;
      if (contents[entryId] !== undefined) return contents[entryId]!;
      const loaded = await noteService.historyRead(noteId, entryId);
      setContents((previous) => ({ ...previous, [entryId]: loaded.body }));
      return loaded.body;
    },
    [contents, currentContent, noteId],
  );

  const [diff, setDiff] = useState<{ left: string; right: string } | null>(null);
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [left, right] = await Promise.all([ensureContent(leftId), ensureContent(rightId)]);
        if (!cancelled) setDiff({ left, right });
      } catch (cause) {
        if (!cancelled) setError(errorMessage(cause));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [ensureContent, leftId, rightId]);

  const rows = useMemo(
    () => (diff ? diffLines(diff.left, diff.right) : []),
    [diff],
  );
  const stats = useMemo(() => diffStats(rows), [rows]);

  const restore = async () => {
    if (!leftId || leftId === CURRENT) {
      toast.error('请先在左侧选择一个历史版本');
      return;
    }
    if (!window.confirm('用这个历史版本覆盖当前正文？（当前内容也会被记入历史，可再回滚）')) {
      return;
    }
    setBusy(true);
    try {
      const restored = await noteService.historyRestore(noteId, leftId);
      // 直接用返回值更新 store：不再二次请求，避免与并发自动保存抢写 detail
      useNotesStore.setState({
        detail: restored,
        selectedId: noteId,
        saveStatus: 'idle',
        saveError: null,
        conflict: null,
      });
      toast.success('已还原到该历史版本（只回滚正文）');
      onRestored?.();
      await reload();
    } catch (cause) {
      reportFailure('还原历史版本失败', errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  /** 把当前正文存成一个命名版本（对标语雀「保存为版本」）。 */
  const saveVersion = async () => {
    const label = versionName.trim();
    if (!label) {
      toast.error('请填写版本名');
      return;
    }
    setBusy(true);
    try {
      await noteService.historySaveVersion(noteId, label);
      toast.success(`已保存版本「${label}」`);
      setVersionName('');
      await reload();
    } catch (cause) {
      reportFailure('保存版本失败', errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  /** 命名已有快照（空值 = 清除命名）。 */
  const renameVersion = async (entryId: string, label: string) => {
    try {
      const list = await noteService.historyLabel(noteId, entryId, label.trim() || null);
      setEntries(list);
      setNamingId(null);
      setNamingValue('');
      toast.success(label.trim() ? '已命名该版本' : '已清除版本名');
    } catch (cause) {
      reportFailure('命名版本失败', errorMessage(cause));
    }
  };

  const remove = async (entryId: string) => {
    try {
      const remaining = await noteService.historyRemove(noteId, entryId);
      toast.success(`已删除该快照，剩余 ${remaining} 份`);
      await reload();
    } catch (cause) {
      reportFailure('删除快照失败', errorMessage(cause));
    }
  };

  return (
    <Dialog open onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent className="w-[min(880px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-1.5">
            <History className="h-4 w-4 text-[var(--brand)]" />
            历史版本
          </DialogTitle>
          <DialogDescription>
            每次保存会把**保存前**的内容留一份快照（存放在
            <code className="mx-1">.knowledge/history/</code>，可直接用其它编辑器查看）。
            选择左右两版即可对比；还原会覆盖当前正文，但**不会**改动标题、标签等元数据。
          </DialogDescription>
        </DialogHeader>

        {error ? <p className="text-xs text-red-500">{error}</p> : null}

        <div className="grid min-h-0 gap-3 md:grid-cols-[240px_1fr]">
          <div className="flex min-h-0 flex-col gap-1.5">
            {/* 对标语雀「保存为版本」：把当前正文显式存成一个命名版本 */}
            <div className="flex items-center gap-1.5 rounded-md border border-[var(--border-subtle)] p-1.5">
              <Input
                aria-label="新版本名称"
                value={versionName}
                data-testid="history-version-name"
                placeholder="版本名，例如 v1 交付版"
                className="h-7 text-xs"
                onChange={(event) => setVersionName(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') void saveVersion();
                }}
              />
              <Button
                size="sm"
                variant="secondary"
                data-testid="history-save-version"
                disabled={busy}
                onClick={() => void saveVersion()}
              >
                保存为版本
              </Button>
            </div>
            <div className="max-h-[42vh] min-h-[140px] space-y-1 overflow-auto rounded-md border border-[var(--border-subtle)] p-1.5">
            {entries === null ? (
              <div className="space-y-1.5 p-1">
                {[0, 1, 2].map((index) => (
                  <Skeleton key={index} className="h-10" />
                ))}
              </div>
            ) : entries.length === 0 ? (
              <p className="p-2 text-xs leading-relaxed text-[var(--ink-subtle)]">
                还没有历史版本。保存过一次之后，这里会记录保存前的内容。
              </p>
            ) : (
              <>
                <button
                  type="button"
                  data-testid="history-entry-current"
                  onClick={() => setRightId(CURRENT)}
                  className={cn(
                    'block w-full rounded px-2 py-1.5 text-left text-xs transition-colors',
                    rightId === CURRENT
                      ? 'bg-[var(--brand-soft)] text-[var(--brand)]'
                      : 'text-[var(--ink-muted)] hover:bg-[var(--surface-muted)]',
                  )}
                >
                  <span className="font-medium">当前版本</span>
                  <span className="ml-1 text-[10px] text-[var(--ink-subtle)]">编辑器中的内容</span>
                </button>
                {entries.map((entry) => (
                  <div
                    key={entry.entryId}
                    data-testid="history-entry"
                    className={cn(
                      'group rounded px-2 py-1.5 text-xs transition-colors',
                      leftId === entry.entryId
                        ? 'bg-[var(--brand-soft)]'
                        : 'hover:bg-[var(--surface-muted)]',
                    )}
                  >
                    <button
                      type="button"
                      onClick={() => setLeftId(entry.entryId)}
                      className="block w-full text-left"
                    >
                      {entry.label ? (
                        <span
                          data-testid="history-entry-label"
                          className="mb-0.5 flex items-center gap-1 truncate font-medium text-[var(--brand)]"
                        >
                          <Tag className="h-3 w-3" />
                          {entry.label}
                        </span>
                      ) : null}
                      <span className="flex items-center gap-1 text-[var(--ink)]">
                        <Clock className="h-3 w-3 text-[var(--ink-subtle)]" />
                        {formatDateTime(entry.savedAt)}
                      </span>
                      {entry.preview ? (
                        <span className="mt-0.5 block truncate text-[var(--ink-subtle)]">
                          {entry.preview}
                        </span>
                      ) : null}
                    </button>
                    {namingId === entry.entryId ? (
                      <div className="mt-1 flex items-center gap-1">
                        <Input
                          aria-label="重命名版本"
                          autoFocus
                          value={namingValue}
                          data-testid="history-label-input"
                          placeholder="版本名（留空即清除）"
                          className="h-6 text-[11px]"
                          onChange={(event) => setNamingValue(event.target.value)}
                          onKeyDown={(event) => {
                            if (event.key === 'Enter') void renameVersion(entry.entryId, namingValue);
                            if (event.key === 'Escape') setNamingId(null);
                          }}
                        />
                        <button
                          type="button"
                          data-testid="history-label-save"
                          className="shrink-0 text-[10px] text-[var(--brand)] underline decoration-dotted"
                          onClick={() => void renameVersion(entry.entryId, namingValue)}
                        >
                          保存
                        </button>
                      </div>
                    ) : (
                    <div className="mt-1 flex items-center gap-2 text-[10px]">
                      <button
                        type="button"
                        data-testid="history-label-edit"
                        onClick={() => {
                          setNamingId(entry.entryId);
                          setNamingValue(entry.label ?? '');
                        }}
                        className="underline decoration-dotted text-[var(--ink-subtle)] hover:text-[var(--brand)]"
                      >
                        {entry.label ? '改名' : '命名'}
                      </button>
                      <button
                        type="button"
                        onClick={() => setRightId(entry.entryId)}
                        className={cn(
                          'underline decoration-dotted',
                          rightId === entry.entryId
                            ? 'text-[var(--brand)]'
                            : 'text-[var(--ink-subtle)]',
                        )}
                      >
                        作为对比目标
                      </button>
                      <button
                        type="button"
                        onClick={() => void remove(entry.entryId)}
                        className="flex items-center gap-0.5 text-[var(--ink-subtle)] hover:text-red-500"
                      >
                        <Trash2 className="h-3 w-3" />
                        删除
                      </button>
                    </div>
                    )}
                  </div>
                ))}
              </>
            )}
            </div>
          </div>

          <div className="flex min-h-0 flex-col rounded-md border border-[var(--border-subtle)]">
            <div className="flex items-center gap-2 border-b border-[var(--border-subtle)] px-2 py-1.5 text-[11px] text-[var(--ink-subtle)]">
              <span>
                对比：
                <span className="ml-1 text-[var(--ink)]">
                  {leftId ? (leftId === CURRENT ? '当前版本' : '选中快照') : '（未选择）'}
                </span>
                <span className="mx-1">→</span>
                <span className="text-[var(--ink)]">
                  {rightId === CURRENT ? '当前版本' : '另一快照'}
                </span>
              </span>
              <span className="ml-auto flex items-center gap-2">
                <span className="text-emerald-600">+{stats.added}</span>
                <span className="text-red-500">−{stats.removed}</span>
              </span>
            </div>
            <div className="max-h-[46vh] min-h-[160px] overflow-auto font-mono text-[11px] leading-relaxed">
              {!diff ? (
                <p className="p-3 text-[var(--ink-subtle)]">正在加载对比…</p>
              ) : rows.length === 0 ? (
                <p className="p-3 text-[var(--ink-subtle)]">两个版本内容一致。</p>
              ) : (
                <table className="w-full border-collapse">
                  <tbody>
                    {rows.map((row, index) => (
                      <tr
                        key={index}
                        data-testid={`diff-row-${row.kind}`}
                        className={cn(
                          row.kind === 'add' && 'bg-emerald-500/10',
                          row.kind === 'remove' && 'bg-red-500/10',
                        )}
                      >
                        <td className="w-10 select-none px-1 text-right text-[var(--ink-subtle)]">
                          {row.leftNo ?? ''}
                        </td>
                        <td className="w-10 select-none px-1 text-right text-[var(--ink-subtle)]">
                          {row.rightNo ?? ''}
                        </td>
                        <td className="w-4 select-none text-center text-[var(--ink-subtle)]">
                          {row.kind === 'add' ? '+' : row.kind === 'remove' ? '−' : ' '}
                        </td>
                        <td className="whitespace-pre-wrap break-words px-1 text-[var(--ink)]">
                          {row.text || '\u00a0'}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          </div>
        </div>

        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            关闭
          </Button>
          <Button
            onClick={() => void restore()}
            disabled={busy || !leftId || leftId === CURRENT}
            data-testid="history-restore"
          >
            {busy ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <RotateCcw className="h-3.5 w-3.5" />
            )}
            还原到选中版本
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
