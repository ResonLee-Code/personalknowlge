import {
  AlertTriangle,
  Check,
  ChevronRight,
  ListTree,
  Loader2,
  PenLine,
  RefreshCw,
  Sparkles,
  X,
} from 'lucide-react';
import { useMemo, useState } from 'react';
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
import { reportFailure } from '@/lib/actionFeedback';
import { diffLines, diffStats } from '@/lib/diff';
import { errorMessage } from '@/lib/format';
import { writingService } from '@/services/writingService';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';
import type {
  AskCitation,
  WritingApplyInput,
  WritingApplyPreview,
  WritingDraft,
  WritingMode,
} from '@/types';

interface WritingPanelProps {
  noteId: string;
  noteTitle: string;
  content: string;
  baseContentHash: string;
  selection: string | null;
  selectionStart: number | null;
  selectionEnd: number | null;
  onClose: () => void;
  onApplied: (detail: NonNullable<ReturnType<typeof useNotesStore.getState>['detail']>) => void;
}

const MODES: Array<{
  value: WritingMode;
  label: string;
  description: string;
  replaces: boolean;
}> = [
  {
    value: 'outline',
    label: '大纲',
    description: '逐条生成可接受的结构建议',
    replaces: false,
  },
  {
    value: 'continue',
    label: '续写',
    description: '在目标位置继续写并附引用',
    replaces: false,
  },
  {
    value: 'rewrite',
    label: '改写',
    description: '替换选中文字或整篇正文',
    replaces: true,
  },
  {
    value: 'shorten',
    label: '缩短',
    description: '压缩选中文字',
    replaces: true,
  },
  {
    value: 'expand',
    label: '扩写',
    description: '补充有来源的解释与例子',
    replaces: true,
  },
];

function citationNumbers(citation: AskCitation): string {
  return `[${citation.citationId}]`;
}

/** Writing 2.0：生成、引用、逐段接受、diff 与最终确认写入。 */
export function WritingPanel({
  noteId,
  noteTitle,
  content,
  baseContentHash,
  selection,
  selectionStart,
  selectionEnd,
  onClose,
  onApplied,
}: WritingPanelProps) {
  const [mode, setMode] = useState<WritingMode>('outline');
  const [instruction, setInstruction] = useState('');
  const [confirmOutbound, setConfirmOutbound] = useState(false);
  const [draft, setDraft] = useState<WritingDraft | null>(null);
  const [accepted, setAccepted] = useState<Set<string>>(new Set());
  const [preview, setPreview] = useState<WritingApplyPreview | null>(null);
  const [busy, setBusy] = useState<'generate' | 'preview' | 'apply' | null>(null);
  const [error, setError] = useState<string | null>(null);

  const acceptedSegments = useMemo(
    () => draft?.segments.filter((segment) => accepted.has(segment.id)) ?? [],
    [accepted, draft],
  );
  const rows = useMemo(
    () => (preview ? diffLines(content, preview.content) : []),
    [content, preview],
  );
  const stats = useMemo(() => diffStats(rows), [rows]);
  const replaceMode = MODES.find((item) => item.value === mode)?.replaces ?? false;

  const resetGenerated = () => {
    setDraft(null);
    setAccepted(new Set());
    setPreview(null);
    setError(null);
  };

  const generate = async () => {
    setBusy('generate');
    setError(null);
    try {
      const next = await writingService.draft({
        noteId,
        mode,
        instruction,
        selection,
        selectionStart,
        selectionEnd,
        confirmOutbound,
      });
      setDraft(next);
      setAccepted(new Set(next.segments.map((segment) => segment.id)));
      setPreview(null);
    } catch (cause) {
      setDraft(null);
      setPreview(null);
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const inputFor = (confirm: boolean): WritingApplyInput | null => {
    if (!draft || acceptedSegments.length === 0) return null;
    return {
      noteId,
      baseContentHash,
      mode,
      targetStart: draft.targetStart,
      targetEnd: draft.targetEnd,
      acceptedSegments,
      confirm,
    };
  };

  const loadPreview = async () => {
    const input = inputFor(false);
    if (!input) {
      setError('至少接受一个生成段落。');
      return;
    }
    setBusy('preview');
    setError(null);
    try {
      setPreview(await writingService.applyPreview(input));
    } catch (cause) {
      setPreview(null);
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const apply = async () => {
    const input = inputFor(true);
    if (!input || !preview) return;
    setBusy('apply');
    setError(null);
    try {
      const result = await writingService.apply(input);
      toast.success(
        `已写入 ${result.acceptedSegments} 段，附 ${result.sourceCount} 条来源；可继续使用撤销 / 历史版本。`,
      );
      onApplied(result.detail);
    } catch (cause) {
      reportFailure('写入写作草稿失败', errorMessage(cause));
      setError(errorMessage(cause));
    } finally {
      setBusy(null);
    }
  };

  const openCitation = (citation: AskCitation) => {
    const notes = useNotesStore.getState();
    useUiStore.getState().setPendingHighlight({
      noteId: citation.noteId,
      charEnd: citation.charEnd,
      charStart: citation.charStart,
    });
    void notes.openDetail(citation.noteId);
    onClose();
  };

  return (
    <Dialog open onOpenChange={(open) => (!open ? onClose() : undefined)}>
      <DialogContent className="w-[min(1040px,calc(100vw-2rem))]" data-testid="writing-panel">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="h-4 w-4" />
            写作助手：{noteTitle}
          </DialogTitle>
          <DialogDescription>
            生成只形成草稿。每个段落必须绑定可解析引用；逐段接受后查看 diff，最后确认才写入。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="grid grid-cols-2 gap-1 rounded-lg bg-[var(--surface-muted)] p-1 sm:grid-cols-5">
            {MODES.map((item) => (
              <button
                key={item.value}
                type="button"
                className={`rounded-md px-2 py-1.5 text-xs ${
                  mode === item.value
                    ? 'bg-[var(--surface)] font-medium text-[var(--ink)] shadow-sm'
                    : 'text-[var(--ink-muted)]'
                }`}
                title={item.description}
                onClick={() => {
                  setMode(item.value);
                  resetGenerated();
                }}
              >
                {item.label}
              </button>
            ))}
          </div>

          <div className="grid gap-3 md:grid-cols-[1fr_auto]">
            <div className="space-y-1">
              <Label htmlFor="writing-instruction">写作要求</Label>
              <Input
                id="writing-instruction"
                value={instruction}
                onChange={(event) => {
                  setInstruction(event.target.value);
                  resetGenerated();
                }}
                placeholder="例如：整理成三层大纲；保持审慎语气；扩写例子"
                className="h-8 text-xs"
              />
            </div>
            <div className="flex items-end gap-2">
              {selection ? (
                <span className="max-w-64 truncate text-[10px] text-[var(--ink-subtle)]">
                  已选中：{selection}
                </span>
              ) : replaceMode ? (
                <span className="flex items-center gap-1 text-[10px] text-amber-600">
                  <AlertTriangle className="h-3 w-3" />
                  未选择时将以整篇正文为目标
                </span>
              ) : null}
              <Button size="sm" disabled={busy !== null} onClick={() => void generate()}>
                {busy === 'generate' ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Sparkles className="h-3.5 w-3.5" />
                )}
                生成草稿
              </Button>
            </div>
          </div>
          <label className="flex items-center gap-2 text-[11px] text-[var(--ink-muted)]">
            <input
              type="checkbox"
              checked={confirmOutbound}
              onChange={(event) => setConfirmOutbound(event.target.checked)}
            />
            若在线模型已配置，允许本次确认外发材料（默认不确认）
          </label>

          {error ? (
            <p className="rounded-md border border-red-500/30 bg-red-500/5 px-3 py-2 text-xs text-red-600">
              {error}
            </p>
          ) : null}

          {draft ? (
            <div className="grid min-h-0 gap-3 lg:grid-cols-2">
              <section className="space-y-2">
                <div className="flex items-center justify-between text-xs">
                  <span className="font-medium text-[var(--ink)]">
                    生成段落（{acceptedSegments.length}/{draft.segments.length}）
                  </span>
                  <span className="text-[10px] text-[var(--ink-subtle)]">
                    {draft.provider} · {draft.tookMs} ms
                  </span>
                </div>
                {draft.warnings.map((warning) => (
                  <p key={warning} className="text-[10px] leading-relaxed text-amber-600">
                    {warning}
                  </p>
                ))}
                <div className="space-y-1.5">
                  {draft.segments.map((segment) => {
                    const checked = accepted.has(segment.id);
                    return (
                      <article
                        key={segment.id}
                        data-testid={`writing-segment-${segment.id}`}
                        className={`rounded-lg border p-2 ${
                          checked
                            ? 'border-[var(--brand)]/40 bg-[var(--brand-soft)]/30'
                            : 'border-[var(--border-subtle)] opacity-70'
                        }`}
                      >
                        <div className="flex items-start gap-2">
                          <button
                            type="button"
                            aria-label={`${checked ? '拒绝' : '接受'}段落 ${segment.id}`}
                            className={`mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded border ${
                              checked
                                ? 'border-[var(--brand)] bg-[var(--brand)] text-white'
                                : 'border-[var(--border-subtle)]'
                            }`}
                            onClick={() => {
                              setAccepted((current) => {
                                const next = new Set(current);
                                if (next.has(segment.id)) next.delete(segment.id);
                                else next.add(segment.id);
                                return next;
                              });
                              setPreview(null);
                            }}
                          >
                            {checked ? <Check className="h-3 w-3" /> : null}
                          </button>
                          <p className="min-w-0 flex-1 whitespace-pre-wrap text-xs leading-relaxed text-[var(--ink)]">
                            {segment.text}
                          </p>
                        </div>
                        <div className="mt-1.5 flex flex-wrap gap-1 pl-6">
                          {segment.citations.map((citation) => (
                            <button
                              key={`${segment.id}-${citation.citationId}-${citation.noteId}`}
                              type="button"
                              className="rounded bg-[var(--surface)] px-1.5 py-0.5 text-[9px] text-[var(--brand)] hover:underline"
                              title={citation.snippet}
                              onClick={() => openCitation(citation)}
                            >
                              {citationNumbers(citation)} {citation.title}
                              <ChevronRight className="ml-0.5 inline h-2.5 w-2.5" />
                            </button>
                          ))}
                        </div>
                      </article>
                    );
                  })}
                </div>
                <Button
                  size="sm"
                  variant="secondary"
                  disabled={busy !== null || acceptedSegments.length === 0}
                  onClick={() => void loadPreview()}
                >
                  <RefreshCw
                    className={`h-3.5 w-3.5 ${busy === 'preview' ? 'animate-spin' : ''}`}
                  />
                  预览 diff
                </Button>
              </section>

              <section className="space-y-2">
                <div className="flex items-center justify-between">
                  <h3 className="flex items-center gap-1.5 text-xs font-medium text-[var(--ink)]">
                    <ListTree className="h-3.5 w-3.5" />
                    diff 预览
                  </h3>
                  {preview ? (
                    <span className="text-[10px] tabular-nums text-[var(--ink-subtle)]">
                      +{stats.added} −{stats.removed}
                    </span>
                  ) : null}
                </div>
                {preview ? (
                  <div
                    className="max-h-80 overflow-auto rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] font-mono text-[10px]"
                    data-testid="writing-diff"
                  >
                    {rows.map((row, index) => (
                      <div
                        key={`${row.kind}-${row.leftNo}-${row.rightNo}-${index}`}
                        data-testid={`writing-diff-${row.kind}`}
                        className={`flex gap-2 border-b border-[var(--border-subtle)] px-2 py-1 ${
                          row.kind === 'add'
                            ? 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-400'
                            : row.kind === 'remove'
                              ? 'bg-red-500/10 text-red-600'
                              : 'text-[var(--ink-muted)]'
                        }`}
                      >
                        <span className="w-8 shrink-0 text-right text-[var(--ink-subtle)]">
                          {row.leftNo ?? '·'}
                        </span>
                        <span className="w-8 shrink-0 text-right text-[var(--ink-subtle)]">
                          {row.rightNo ?? '·'}
                        </span>
                        <span className="min-w-0 flex-1 whitespace-pre-wrap">
                          {row.kind === 'add' ? '+ ' : row.kind === 'remove' ? '− ' : '  '}
                          {row.text}
                        </span>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="flex h-48 items-center justify-center rounded-md border border-dashed border-[var(--border-subtle)] text-[11px] text-[var(--ink-subtle)]">
                    逐段接受后点击「预览 diff」
                  </div>
                )}
                {preview?.warnings.map((warning) => (
                  <p key={warning} className="text-[10px] text-amber-600">
                    {warning}
                  </p>
                ))}
              </section>
            </div>
          ) : null}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            <X className="h-3.5 w-3.5" /> 取消
          </Button>
          <Button
            disabled={busy !== null || !preview || acceptedSegments.length === 0}
            onClick={() => void apply()}
          >
            {busy === 'apply' ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <PenLine className="h-3.5 w-3.5" />
            )}
            确认写入 {acceptedSegments.length} 段
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
