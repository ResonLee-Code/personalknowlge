import {
  BookmarkPlus,
  CheckCircle2,
  ExternalLink,
  FlaskConical,
  Loader2,
  ShieldCheck,
  Sparkles,
  TriangleAlert,
  X,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { MarkdownPreview } from '@/components/editor/MarkdownPreview';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { FailureDetail } from '@/components/ui/failure-detail';
import { Label, Textarea } from '@/components/ui/input';
import { ROUTES } from '@/lib/constants';
import { errorMessage } from '@/lib/format';
import { aiService } from '@/services/aiService';
import { askService } from '@/services/askService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useUiStore } from '@/stores/uiStore';
import type { AiPolicy, AskAnswer, AskNotePreview, AskScope } from '@/types';

const SCOPE_LABELS: Record<AskScope, string> = {
  note: '当前笔记',
  category: '当前分类',
  tag: '当前标签',
  all: '全库',
};

/**
 * Ask（V2.0 M1）：从右侧打开的知识库问答面板。
 *
 * 后端返回的每个来源都带原文块区间；这里只负责作用域选择与跳转。
 * 未得到证据时明确展示“不生成结论”，不以模板文字冒充答案。
 */
export function AskKnowledgePanel() {
  const navigate = useNavigate();
  const setOpen = useUiStore((state) => state.setAskPanelOpen);
  const seed = useUiStore((state) => state.askSeed);
  const openResearchDraft = useUiStore((state) => state.openResearchDraft);
  const setPendingHighlight = useUiStore((state) => state.setPendingHighlight);
  const detail = useNotesStore((state) => state.detail);
  const categories = useCategoriesStore((state) => state.flat);
  const tags = useTagsStore((state) => state.tags);

  const [question, setQuestion] = useState(seed?.question ?? '');
  const [selection, setSelection] = useState<string | null>(seed?.selection ?? null);
  const [noteId] = useState<string | null>(seed?.noteId ?? detail?.note.id ?? null);
  const [scope, setScope] = useState<AskScope>(() => (noteId ? 'note' : 'all'));
  const [categoryId, setCategoryId] = useState<string | null>(detail?.note.categoryId ?? null);
  const [tagId, setTagId] = useState<string | null>(null);
  const [policy, setPolicy] = useState<AiPolicy | null>(null);
  const [confirmOutbound, setConfirmOutbound] = useState(false);
  const [turns, setTurns] = useState<AskAnswer[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saveAnswer, setSaveAnswer] = useState<AskAnswer | null>(null);
  const [savePreview, setSavePreview] = useState<AskNotePreview | null>(null);
  const [saveTitle, setSaveTitle] = useState('');
  const [saveCategoryId, setSaveCategoryId] = useState<string | null>(null);
  const [saveBusy, setSaveBusy] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void aiService
      .policy()
      .then((value) => {
        if (!cancelled) setPolicy(value);
      })
      .catch(() => {
        if (!cancelled) setPolicy(null);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const scopeId = useMemo(() => {
    if (scope === 'note') return noteId;
    if (scope === 'category') return categoryId;
    if (scope === 'tag') return tagId;
    return null;
  }, [scope, noteId, categoryId, tagId]);

  const missingScope = scope !== 'all' && !scopeId;
  const remoteMaySend = Boolean(
    policy?.remoteModelCompose && policy.remoteModelAuthorizedAt && policy.remoteModelKeySet,
  );
  const needsPerCallConfirm = Boolean(remoteMaySend && policy?.confirmEachOutbound);

  const submit = async () => {
    if (!question.trim() || missingScope || busy) return;
    setBusy(true);
    setError(null);
    try {
      const result = await askService.ask({
        question,
        scope,
        scopeId,
        selection,
        history: turns.slice(-6).map((turn) => ({
          question: turn.question,
          answer: turn.text,
        })),
        confirmOutbound,
      });
      setTurns((previous) => [...previous, result]);
      setQuestion('');
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(false);
    }
  };

  const openSaveDialog = async (answer: AskAnswer) => {
    setSaveAnswer(answer);
    setSaveBusy(true);
    setSaveError(null);
    try {
      const preview = await askService.previewAnswerNote({
        answer,
        confirm: false,
      });
      setSavePreview(preview);
      setSaveTitle(preview.title);
      setSaveCategoryId(preview.categoryId ?? null);
    } catch (caught) {
      setSaveAnswer(null);
      setError(errorMessage(caught));
    } finally {
      setSaveBusy(false);
    }
  };

  const savePreviewedAnswer = async () => {
    if (!saveAnswer || !savePreview || saveBusy) return;
    setSaveBusy(true);
    setSaveError(null);
    try {
      const saved = await askService.saveAnswer({
        answer: saveAnswer,
        title: saveTitle.trim(),
        categoryId: saveCategoryId,
        confirm: true,
      });
      await useNotesStore.getState().refresh();
      toast.success(`答案已存为「${saved.title}」`);
      setSavePreview(null);
      setSaveAnswer(null);
      setOpen(false);
      navigate(ROUTES.note(saved.noteId));
    } catch (caught) {
      setSaveError(errorMessage(caught));
    } finally {
      setSaveBusy(false);
    }
  };

  const openCitation = (noteId: string, charStart: number, charEnd: number) => {
    setPendingHighlight({ noteId, charStart, charEnd });
    navigate(ROUTES.note(noteId));
  };

  return (
    <>
      <Dialog open onOpenChange={setOpen}>
        <DialogContent className="bottom-4 left-auto right-4 top-4 flex h-auto w-[min(540px,calc(100vw-2rem))] translate-x-0 translate-y-0 flex-col overflow-hidden p-0">
          <DialogHeader className="mb-0 border-b border-[var(--border-subtle)] px-5 py-4 pr-12">
            <DialogTitle className="flex items-center gap-2">
              <Sparkles className="h-4 w-4 text-[var(--brand)]" />
              问知识库
            </DialogTitle>
            <DialogDescription>
              答案只使用本轮检索到的本地笔记；可连续追问，只有确认预览后才存为笔记。
            </DialogDescription>
          </DialogHeader>

          <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
            <div className="grid gap-3 sm:grid-cols-2">
              <div className="space-y-1.5">
                <Label htmlFor="ask-scope">提问范围</Label>
                <select
                  id="ask-scope"
                  value={scope}
                  onChange={(event) => setScope(event.target.value as AskScope)}
                  className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand)]"
                >
                  <option value="note" disabled={!noteId}>
                    当前笔记
                  </option>
                  <option value="category">当前分类</option>
                  <option value="tag">当前标签</option>
                  <option value="all">全库</option>
                </select>
              </div>

              {scope === 'category' ? (
                <div className="space-y-1.5">
                  <Label htmlFor="ask-category">分类</Label>
                  <select
                    id="ask-category"
                    value={categoryId ?? ''}
                    onChange={(event) => setCategoryId(event.target.value || null)}
                    className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand)]"
                  >
                    <option value="">选择一个分类…</option>
                    {categories.map((category) => (
                      <option key={category.id} value={category.id}>
                        {category.name}
                      </option>
                    ))}
                  </select>
                </div>
              ) : null}

              {scope === 'tag' ? (
                <div className="space-y-1.5">
                  <Label htmlFor="ask-tag">标签</Label>
                  <select
                    id="ask-tag"
                    value={tagId ?? ''}
                    onChange={(event) => setTagId(event.target.value || null)}
                    className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand)]"
                  >
                    <option value="">选择一个标签…</option>
                    {tags.map((tag) => (
                      <option key={tag.id} value={tag.id}>
                        {tag.name}
                      </option>
                    ))}
                  </select>
                </div>
              ) : null}
            </div>

            {selection ? (
              <div className="mt-4 rounded-md border border-[var(--border-subtle)] bg-[var(--brand-soft)]/40 p-2.5">
                <div className="flex items-center justify-between gap-2">
                  <span className="text-[11px] font-medium text-[var(--brand)]">本次选中文字</span>
                  <button
                    type="button"
                    aria-label="移除选中文字"
                    title="移除选中文字"
                    onClick={() => setSelection(null)}
                    className="rounded p-0.5 text-[var(--ink-subtle)] hover:bg-[var(--surface)] hover:text-[var(--ink)]"
                  >
                    <X className="h-3.5 w-3.5" />
                  </button>
                </div>
                <p className="mt-1 line-clamp-4 whitespace-pre-wrap text-[11px] leading-relaxed text-[var(--ink-muted)]">
                  {selection}
                </p>
                <p className="mt-1 text-[10px] text-[var(--ink-subtle)]">
                  选中文字只帮助理解问题，不能作为引用来源。
                </p>
              </div>
            ) : null}

            <div className="mt-4 space-y-1.5">
              <Label htmlFor="ask-question">问题</Label>
              <Textarea
                id="ask-question"
                value={question}
                onChange={(event) => setQuestion(event.target.value)}
                onKeyDown={(event) => {
                  if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
                    event.preventDefault();
                    void submit();
                  }
                }}
                placeholder="例如：之前关于 SQLite FTS5 的结论有哪些？"
                className="min-h-24 resize-y"
                maxLength={2000}
              />
              <p className="flex items-center gap-1.5 text-[11px] text-[var(--ink-subtle)]">
                <ShieldCheck className="h-3.5 w-3.5" />
                默认只检索本地；在线模型必须已授权。⌘/Ctrl + Enter 提交。
              </p>
            </div>

            {needsPerCallConfirm ? (
              <label className="mt-3 flex cursor-pointer items-start gap-2 rounded-md border border-amber-500/30 bg-amber-500/10 p-2.5 text-xs text-[var(--ink-muted)]">
                <input
                  type="checkbox"
                  checked={confirmOutbound}
                  onChange={(event) => setConfirmOutbound(event.target.checked)}
                  className="mt-0.5 accent-[var(--brand)]"
                />
                <span>本次允许把检索片段交给已配置的在线模型；不勾选则零请求并自动降级。</span>
              </label>
            ) : null}

            <Button
              className="mt-3 w-full"
              disabled={!question.trim() || missingScope || busy}
              onClick={() => void submit()}
            >
              {busy ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Sparkles className="h-4 w-4" />
              )}
              {busy
                ? '正在重新检索并核对引用…'
                : turns.length > 0
                  ? `在「${SCOPE_LABELS[scope]}」中继续追问`
                  : `在「${SCOPE_LABELS[scope]}」中提问`}
            </Button>

            {missingScope ? (
              <p className="mt-2 text-xs text-amber-600 dark:text-amber-300">
                请先选择具体的范围。
              </p>
            ) : null}
            {error ? (
              <div className="mt-3 rounded-md border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-700 dark:text-red-300">
                <FailureDetail summary="提问失败" detail={error} />
              </div>
            ) : null}

            {turns.length > 0 ? (
              <div className="mt-5 space-y-6" aria-live="polite">
                {turns.map((answer, turnIndex) => (
                  <section
                    key={`${turnIndex}-${answer.question}`}
                    className="border-t border-[var(--border-subtle)] pt-4"
                  >
                    <p className="mb-2 text-[11px] font-medium text-[var(--ink-subtle)]">
                      第 {turnIndex + 1} 轮 · {answer.question}
                    </p>
                    <div className="mb-3 flex flex-wrap items-center gap-1.5">
                      <Badge variant={answer.evidenceSufficient ? 'brand' : 'outline'}>
                        {answer.evidenceSufficient ? (
                          <CheckCircle2 className="mr-1 h-3 w-3" />
                        ) : (
                          <TriangleAlert className="mr-1 h-3 w-3" />
                        )}
                        {answer.evidenceSufficient ? '有可引用材料' : '证据不足'}
                      </Badge>
                      <Badge variant="outline">{answer.tier}</Badge>
                      <span className="text-[11px] text-[var(--ink-subtle)]">
                        {answer.retrievedCount} 条材料 · {answer.tookMs}ms · {answer.provider}
                      </span>
                      <Button
                        variant="outline"
                        size="sm"
                        className="h-7"
                        onClick={() => {
                          openResearchDraft({
                            topic: answer.question,
                            scope: '从 Ask 会话发起的深入调研',
                          });
                          setOpen(false);
                          navigate(ROUTES.research);
                        }}
                      >
                        <FlaskConical className="h-3.5 w-3.5" />
                        深入调研
                      </Button>
                      {answer.evidenceSufficient ? (
                        <Button
                          variant="outline"
                          size="sm"
                          className="ml-auto h-7"
                          disabled={saveBusy}
                          onClick={() => void openSaveDialog(answer)}
                        >
                          {saveBusy && saveAnswer === answer ? (
                            <Loader2 className="h-3.5 w-3.5 animate-spin" />
                          ) : (
                            <BookmarkPlus className="h-3.5 w-3.5" />
                          )}
                          存为笔记…
                        </Button>
                      ) : null}
                    </div>

                    <div className="rounded-lg border border-[var(--border-subtle)] bg-[var(--surface)]">
                      <MarkdownPreview content={answer.text} />
                    </div>

                    {answer.warnings.map((warning) => (
                      <p
                        key={warning}
                        className="mt-2 flex items-start gap-1.5 text-[11px] text-amber-700 dark:text-amber-300"
                      >
                        <TriangleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                        {warning}
                      </p>
                    ))}

                    <div className="mt-4 space-y-2">
                      {answer.citations.length > 0 ? (
                        <>
                          <p className="text-xs font-medium text-[var(--ink-muted)]">来源与段落</p>
                          {answer.citations.map((citation) => (
                            <button
                              key={`${citation.citationId}-${citation.noteId}-${citation.chunkIndex}`}
                              type="button"
                              onClick={() =>
                                openCitation(citation.noteId, citation.charStart, citation.charEnd)
                              }
                              className="group w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface-muted)] p-2.5 text-left transition-colors hover:border-[var(--brand)]"
                            >
                              <span className="flex items-center gap-1.5 text-xs font-medium text-[var(--ink)]">
                                <span className="rounded bg-[var(--brand-soft)] px-1.5 py-0.5 text-[10px] text-[var(--brand)]">
                                  {citation.citationId}
                                </span>
                                <span className="min-w-0 flex-1 truncate">{citation.title}</span>
                                <ExternalLink className="h-3 w-3 shrink-0 text-[var(--ink-subtle)] group-hover:text-[var(--brand)]" />
                              </span>
                              {citation.heading ? (
                                <span className="mt-1 block text-[10px] text-[var(--brand)]">
                                  {citation.heading}
                                </span>
                              ) : null}
                              <span className="mt-1 line-clamp-3 block text-[11px] leading-relaxed text-[var(--ink-muted)]">
                                {citation.snippet}
                              </span>
                            </button>
                          ))}
                        </>
                      ) : (
                        <p className="text-xs text-[var(--ink-subtle)]">{answer.evidenceNote}</p>
                      )}
                    </div>
                  </section>
                ))}
              </div>
            ) : null}
          </div>
        </DialogContent>
      </Dialog>
      {savePreview ? (
        <Dialog
          open
          onOpenChange={(open) => {
            if (!open) {
              setSavePreview(null);
              setSaveAnswer(null);
              setSaveError(null);
            }
          }}
        >
          <DialogContent className="flex max-h-[88vh] w-[min(680px,calc(100vw-2rem))] flex-col overflow-hidden p-0">
            <DialogHeader className="mb-0 border-b border-[var(--border-subtle)] px-5 py-4 pr-12">
              <DialogTitle className="flex items-center gap-2">
                <BookmarkPlus className="h-4 w-4 text-[var(--brand)]" />
                把答案存为笔记
              </DialogTitle>
              <DialogDescription>
                写入前预览正文与来源；确认后创建普通 Markdown 笔记，不覆盖已有笔记。
              </DialogDescription>
            </DialogHeader>
            <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
              <div className="grid gap-3 sm:grid-cols-2">
                <div className="space-y-1.5">
                  <Label htmlFor="ask-save-title">笔记标题</Label>
                  <input
                    id="ask-save-title"
                    value={saveTitle}
                    onChange={(event) => setSaveTitle(event.target.value)}
                    maxLength={120}
                    className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-3 text-sm text-[var(--ink)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand)]"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label htmlFor="ask-save-category">保存到</Label>
                  <select
                    id="ask-save-category"
                    value={saveCategoryId ?? ''}
                    onChange={(event) => setSaveCategoryId(event.target.value || null)}
                    className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand)]"
                  >
                    <option value="">Inbox（未分类）</option>
                    {categories.map((category) => (
                      <option key={category.id} value={category.id}>
                        {category.name}
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              {savePreview.warnings.map((warning) => (
                <p
                  key={warning}
                  className="mt-3 flex items-start gap-1.5 text-[11px] text-amber-700 dark:text-amber-300"
                >
                  <TriangleAlert className="mt-0.5 h-3.5 w-3.5 shrink-0" />
                  {warning}
                </p>
              ))}

              <div className="mt-4 overflow-hidden rounded-lg border border-[var(--border-subtle)] bg-[var(--surface)]">
                <MarkdownPreview content={savePreview.content} />
              </div>
            </div>
            <div className="border-t border-[var(--border-subtle)] px-5 py-3">
              {saveError ? (
                <div className="mb-2 rounded-md border border-red-500/30 bg-red-500/10 p-2 text-xs text-red-700 dark:text-red-300">
                  <FailureDetail summary="保存失败" detail={saveError} />
                </div>
              ) : null}
              <div className="flex justify-end gap-2">
                <Button
                  variant="outline"
                  disabled={saveBusy}
                  onClick={() => {
                    setSavePreview(null);
                    setSaveAnswer(null);
                  }}
                >
                  取消
                </Button>
                <Button
                  disabled={!saveTitle.trim() || saveBusy}
                  onClick={() => void savePreviewedAnswer()}
                >
                  {saveBusy ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <BookmarkPlus className="h-4 w-4" />
                  )}
                  确认写入
                </Button>
              </div>
            </div>
          </DialogContent>
        </Dialog>
      ) : null}
    </>
  );
}
