import { GitBranch, Plus, Trash2, X } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
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
import { Input, Label } from '@/components/ui/input';
import { ROUTES } from '@/lib/constants';
import { errorMessage } from '@/lib/format';
import { RELATION_TONE_CLASS, RELATION_TYPES, RELATION_TYPE_LABELS, relationBadge } from '@/lib/relations';
import { cn } from '@/lib/utils';
import { noteService } from '@/services/noteService';
import { relationService } from '@/services/relationService';
import type { NoteSummary, RelationView } from '@/types';

interface RelationsPanelProps {
  noteId: string;
  /** 保存 / 重命名后递增，用于刷新 */
  refreshKey?: number;
  /** 关系变化后通知外部（例如刷新元信息栏角标） */
  onChanged?: () => void;
}

/**
 * 类型化关系面板（N2）。
 *
 * 出边（本篇声明的关系）可增删；入边（谁与本篇有关系）只读展示。
 * 声明写入本篇 frontmatter 的 `relations:` 字段，因此可被外部编辑器查看与修改。
 */
export function RelationsPanel({ noteId, refreshKey = 0, onChanged }: RelationsPanelProps) {
  const [view, setView] = useState<RelationView | null>(null);
  const [open, setOpen] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const loadRef = useRef<() => Promise<void>>(async () => {});

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      const next = await relationService.list(noteId).catch((error: unknown) => {
        console.warn('[relations] 加载关系失败', errorMessage(error));
        return { outgoing: [], incoming: [] } satisfies RelationView;
      });
      if (!cancelled) setView(next);
      // 注意：这里不能回调 onChanged，否则父组件刷新 refreshKey 会造成「加载→回调→再加载」死循环
    };
    loadRef.current = run;
    void run();
    return () => {
      cancelled = true;
    };
  }, [noteId, refreshKey]);

  const reload = useCallback(async () => {
    await loadRef.current();
  }, []);

  const navigate = useNavigate();
  const outgoing = view?.outgoing ?? [];
  const incoming = view?.incoming ?? [];

  const remove = async (id: string) => {
    try {
      const next = await relationService.remove(id);
      setView(next);
      toast.success('已删除关系');
      onChanged?.();
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  return (
    <section
      data-testid="relations-panel"
      className="shrink-0 border-t border-[var(--border-subtle)] bg-[var(--surface)]"
    >
      <header className="flex items-center gap-2 px-3 py-2">
        <button
          type="button"
          className="flex items-center gap-1 text-xs font-medium text-[var(--ink)]"
          aria-expanded={open}
          onClick={() => setOpen((value) => !value)}
        >
          <GitBranch className="h-3.5 w-3.5 text-[var(--brand)]" />
          关系
        </button>
        <span className="rounded-full bg-[var(--surface-muted)] px-2 py-0.5 text-[11px] text-[var(--ink-subtle)]">
          出 {outgoing.length} · 入 {incoming.length}
        </span>
        <Button
          size="sm"
          variant="secondary"
          className="ml-auto h-6"
          onClick={() => setAddOpen(true)}
        >
          <Plus className="h-3 w-3" />
          添加关系
        </Button>
      </header>

      {open ? (
        <div className="max-h-52 space-y-2 overflow-auto px-3 pb-3">
          {outgoing.length === 0 && incoming.length === 0 ? (
            <p className="text-xs leading-relaxed text-[var(--ink-subtle)]">
              还没有关系。可以标注「取代 / 冲突 / 派生自」等语义，比普通链接多一层判断依据。
            </p>
          ) : null}

          {outgoing.length > 0 ? (
            <ul className="space-y-1">
              {outgoing.map((relation) => {
                const badge = relationBadge(relation.relationType);
                return (
                  <li
                    key={relation.id}
                    data-testid="relation-outgoing"
                    className="flex items-center gap-2 rounded-md px-2 py-1 hover:bg-[var(--surface-muted)]"
                  >
                    <span
                      className={cn(
                        'shrink-0 rounded px-1.5 py-0.5 text-[10px]',
                        RELATION_TONE_CLASS[badge.tone],
                      )}
                    >
                      {badge.label}
                    </span>
                    <button
                      type="button"
                      className="min-w-0 flex-1 truncate text-left text-xs text-[var(--ink)] hover:text-[var(--brand)]"
                      onClick={() => navigate(ROUTES.note(relation.targetNoteId))}
                    >
                      {relation.targetTitle}
                    </button>
                    {relation.note ? (
                      <span className="hidden max-w-[180px] truncate text-[11px] text-[var(--ink-subtle)] sm:block">
                        {relation.note}
                      </span>
                    ) : null}
                    <Button
                      size="icon"
                      variant="ghost"
                      className="h-6 w-6"
                      title="删除关系"
                      aria-label={`删除关系：${relation.targetTitle}`}
                      onClick={() => void remove(relation.id)}
                    >
                      <Trash2 className="h-3 w-3" />
                    </Button>
                  </li>
                );
              })}
            </ul>
          ) : null}

          {incoming.length > 0 ? (
            <div className="space-y-1 border-t border-dashed border-[var(--border-subtle)] pt-2">
              <p className="text-[11px] font-medium text-[var(--ink-subtle)]">其它笔记对这篇的关系</p>
              <ul className="space-y-1">
                {incoming.map((relation) => {
                  const badge = relationBadge(relation.relationType);
                  return (
                    <li key={relation.id} data-testid="relation-incoming" className="flex items-center gap-2 px-2 py-1">
                      <span
                        className={cn(
                          'shrink-0 rounded px-1.5 py-0.5 text-[10px]',
                          RELATION_TONE_CLASS[badge.tone],
                        )}
                      >
                        {badge.label}
                      </span>
                      <button
                        type="button"
                        className="min-w-0 flex-1 truncate text-left text-xs text-[var(--ink-muted)] hover:text-[var(--brand)]"
                        onClick={() => navigate(ROUTES.note(relation.sourceNoteId))}
                      >
                        {relation.sourceTitle}
                      </button>
                    </li>
                  );
                })}
              </ul>
            </div>
          ) : null}
        </div>
      ) : null}

      {addOpen ? (
        <AddRelationDialog
          noteId={noteId}
          onClose={() => setAddOpen(false)}
          onAdded={async () => {
            setAddOpen(false);
            await reload();
            onChanged?.();
          }}
        />
      ) : null}
    </section>
  );
}

function AddRelationDialog({
  noteId,
  onClose,
  onAdded,
}: {
  noteId: string;
  onClose: () => void;
  onAdded: () => void;
}) {
  const [keyword, setKeyword] = useState('');
  const [candidates, setCandidates] = useState<NoteSummary[]>([]);
  // 读取失败不能显示成「没有匹配的笔记」（那是在撒谎）——要与「确实没结果」区分开
  const [loadError, setLoadError] = useState<string | null>(null);
  const [target, setTarget] = useState<NoteSummary | null>(null);
  const [relationType, setRelationType] = useState<string>('RELATED_TO');
  const [note, setNote] = useState('');
  const [busy, setBusy] = useState(false);

  const searchCandidates = useCallback(
    () =>
      noteService
        .list({ keyword: keyword.trim() || null, pageSize: 20, sort: 'titleAsc' })
        .then((result) => {
          setCandidates(result.items.filter((item) => item.id !== noteId));
          setLoadError(null);
        })
        .catch((error: unknown) => {
          setCandidates([]);
          setLoadError(errorMessage(error));
        }),
    [keyword, noteId],
  );

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void searchCandidates();
    }, 180);
    return () => window.clearTimeout(timer);
  }, [searchCandidates]);

  const submit = async () => {
    if (!target) {
      toast.error('请选择目标笔记');
      return;
    }
    setBusy(true);
    try {
      await relationService.add({
        sourceId: noteId,
        targetId: target.id,
        relationType,
        note: note.trim() || null,
      });
      toast.success('已添加关系');
      onAdded();
    } catch (error) {
      toast.error(errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent className="w-[min(560px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle>添加关系</DialogTitle>
          <DialogDescription>
            关系会写进本篇的 frontmatter（`relations:`），外部编辑器同样可见。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="relation-type">关系类型</Label>
            <select
              id="relation-type"
              value={relationType}
              onChange={(event) => setRelationType(event.target.value)}
              className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)]"
            >
              {RELATION_TYPES.map((type) => (
                <option key={type} value={type}>
                  {RELATION_TYPE_LABELS[type]}（{type}）
                </option>
              ))}
            </select>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="relation-target">目标笔记</Label>
            <Input
              id="relation-target"
              value={target ? target.title : keyword}
              placeholder="输入标题筛选…"
              onChange={(event) => {
                setTarget(null);
                setKeyword(event.target.value);
              }}
            />
            {!target ? (
              <ul className="max-h-40 overflow-auto rounded-md border border-[var(--border-subtle)]">
                {loadError ? (
                  <li className="flex items-center justify-between gap-2 px-2 py-1.5 text-[11px] text-red-500">
                    <span>读取笔记失败：{loadError}</span>
                    <button
                      type="button"
                      data-testid="relation-candidates-retry"
                      className="shrink-0 rounded border border-[var(--border-subtle)] px-1.5 py-0.5 text-[var(--ink-muted)] hover:text-[var(--ink)]"
                      onClick={() => void searchCandidates()}
                    >
                      重试
                    </button>
                  </li>
                ) : candidates.length === 0 ? (
                  <li className="px-2 py-1.5 text-[11px] text-[var(--ink-subtle)]">没有匹配的笔记</li>
                ) : (
                  candidates.map((item) => (
                    <li key={item.id}>
                      <button
                        type="button"
                        className="w-full px-2 py-1.5 text-left text-xs text-[var(--ink-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]"
                        onClick={() => setTarget(item)}
                      >
                        {item.title}
                      </button>
                    </li>
                  ))
                )}
              </ul>
            ) : (
              <p className="flex items-center gap-1 text-[11px] text-[var(--ink-subtle)]">
                已选择：{target.title}
                <button
                  type="button"
                  className="text-[var(--brand)]"
                  onClick={() => {
                    setTarget(null);
                    setKeyword('');
                  }}
                >
                  <X className="h-3 w-3" />
                </button>
              </p>
            )}
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="relation-note">说明（可选）</Label>
            <Input
              id="relation-note"
              value={note}
              placeholder="例如：v2 取代 v1 的结论"
              onChange={(event) => setNote(event.target.value)}
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={busy || !target}>
            添加
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
