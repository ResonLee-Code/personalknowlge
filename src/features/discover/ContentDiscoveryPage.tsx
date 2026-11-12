import {
  Check,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  Compass,
  ExternalLink,
  FileText,
  FileSearch,
  Loader2,
  RefreshCw,
  Save,
  ShieldCheck,
  Trash2,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { MarkdownPreview } from '@/components/editor/MarkdownPreview';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input, Label } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { reportFailure } from '@/lib/actionFeedback';
import { EVENTS, ROUTES } from '@/lib/constants';
import { errorMessage } from '@/lib/format';
import { cn } from '@/lib/utils';
import { aiService } from '@/services/aiService';
import { subscribe } from '@/services/api';
import { contentDiscoveryService } from '@/services/contentDiscoveryService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import type {
  AiPolicy,
  ContentCapture,
  ContentCaptureStatus,
  ContentDiscoveryCandidate,
  ContentDiscoveryOptions,
  ContentDiscoveryProgress,
} from '@/types';

const STATUS_LABEL: Record<ContentCaptureStatus, string> = {
  received: '已接收',
  discovering: '发现中',
  awaitingSelection: '待选择',
  fetching: '抓取中',
  ready: '待入库',
  saved: '已入库',
  failed: '失败',
  ignored: '已忽略',
  duplicate: '重复内容',
};

const STATUS_TONE: Record<ContentCaptureStatus, string> = {
  received: 'bg-sky-500/10 text-sky-700 dark:text-sky-300',
  discovering: 'bg-sky-500/10 text-sky-700 dark:text-sky-300',
  awaitingSelection: 'bg-amber-500/10 text-amber-700 dark:text-amber-300',
  fetching: 'bg-sky-500/10 text-sky-700 dark:text-sky-300',
  ready: 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-300',
  saved: 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-300',
  failed: 'bg-red-500/10 text-red-700 dark:text-red-300',
  ignored: 'bg-[var(--surface-sunken)] text-[var(--ink-subtle)]',
  duplicate: 'bg-amber-500/10 text-amber-700 dark:text-amber-300',
};

const DEFAULT_OPTIONS: ContentDiscoveryOptions = {
  maxDepth: 1,
  maxPages: 20,
  samePath: true,
  includeSubdomains: false,
  includeExternal: false,
  includeHidden: false,
  keepFullText: false,
};

type PendingAction = 'capture' | 'crawl' | 'refresh' | null;

export function ContentDiscoveryPage() {
  const [params] = useSearchParams();
  const categories = useCategoriesStore((state) => state.flat);
  const refreshCategories = useCategoriesStore((state) => state.refresh);
  const [policy, setPolicy] = useState<AiPolicy | null>(null);
  const [captures, setCaptures] = useState<ContentCapture[]>([]);
  const [current, setCurrent] = useState<ContentCapture | null>(null);
  const [url, setUrl] = useState(params.get('url') ?? params.get('text') ?? '');
  const [userNote, setUserNote] = useState(params.get('note') ?? '');
  const [selectedUrls, setSelectedUrls] = useState<Set<string>>(new Set());
  const [options, setOptions] = useState<ContentDiscoveryOptions>(DEFAULT_OPTIONS);
  const [busy, setBusy] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [pendingAction, setPendingAction] = useState<PendingAction>(null);
  const [saveOpen, setSaveOpen] = useState(false);
  const [saveTitle, setSaveTitle] = useState('');
  const [saveCategoryId, setSaveCategoryId] = useState('');
  const [saveTags, setSaveTags] = useState('');
  const [saveKeepFullText, setSaveKeepFullText] = useState(false);
  const [progress, setProgress] = useState<ContentDiscoveryProgress | null>(null);
  const [previewOpen, setPreviewOpen] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const [nextPolicy, list] = await Promise.all([
        aiService.policy(),
        contentDiscoveryService.list(null, 60),
      ]);
      setPolicy(nextPolicy);
      setCaptures(list);
      setLoadError(null);
      return list;
    } catch (error) {
      setLoadError(errorMessage(error));
      return [];
    }
  }, []);

  useEffect(() => {
    void Promise.resolve().then(() => {
      void refresh().then(async (list) => {
        if (list[0]) {
          const next = await contentDiscoveryService.get(list[0].id);
          setCurrent(next);
          setSelectedUrls(defaultSelectedUrls(next));
        }
      });
      void refreshCategories();
    });
  }, [refresh, refreshCategories]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void subscribe<ContentDiscoveryProgress>(EVENTS.contentDiscoveryProgress, (payload) => {
      setProgress(payload);
      if (payload.status === 'ready') {
        void refresh();
      }
    }).then((dispose) => {
      if (disposed) dispose();
      else unlisten = dispose;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refresh]);

  const networkReady = policy?.network === 'allowNetwork' && Boolean(policy.networkConfirmedAt);

  const selectableCandidates = useMemo(
    () => current?.candidates.filter((candidate) => candidateCanSelect(candidate, options)) ?? [],
    [current, options],
  );

  const openCaptureConfirm = () => {
    if (!url.trim()) {
      toast.info('请先输入网页 URL 或分享文本。');
      return;
    }
    setPendingAction('capture');
  };

  const openCrawlConfirm = () => {
    if (selectedUrls.size === 0) {
      toast.info('请先勾选需要深入抓取的链接。');
      return;
    }
    setPendingAction('crawl');
  };

  const executePending = async () => {
    const action = pendingAction;
    setPendingAction(null);
    if (!action) return;
    setBusy(true);
    try {
      if (action === 'capture') {
        const created = await contentDiscoveryService.create({
          url,
          userNote: userNote.trim() || null,
          confirmed: true,
        });
        setCurrent(created);
        setSelectedUrls(defaultSelectedUrls(created));
        setCaptures(await contentDiscoveryService.list(null, 60));
        toast.success(
          created.candidates.length > 0
            ? `已发现 ${created.candidates.length} 个链接`
            : '内容已进入收件箱',
        );
      } else if (current) {
        if (action === 'refresh') {
          const refreshed = await contentDiscoveryService.refresh(current.id, true);
          setCurrent(refreshed);
          setCaptures(await contentDiscoveryService.list(null, 60));
          toast.info(
            refreshed.sourceChanged
              ? '来源已有变化，收件箱预览已刷新；不会自动覆盖已入库笔记。'
              : '来源暂无正文变化。',
          );
          return;
        }
        const crawled = await contentDiscoveryService.crawlSelected({
          captureId: current.id,
          selectedUrls: [...selectedUrls],
          options,
          confirmed: true,
        });
        setCurrent(crawled);
        setProgress(null);
        setSelectedUrls(new Set());
        setCaptures(await contentDiscoveryService.list(null, 60));
        toast.success(`已抓取 ${crawled.pageCount} 个页面`);
      }
    } catch (error) {
      reportFailure('内容发现失败', errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const openCapture = async (id: string) => {
    setBusy(true);
    try {
      const next = await contentDiscoveryService.get(id);
      setCurrent(next);
      setSelectedUrls(defaultSelectedUrls(next));
    } catch (error) {
      reportFailure('读取内容失败', errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const saveCapture = async () => {
    if (!current) return;
    setBusy(true);
    try {
      const result = await contentDiscoveryService.save({
        captureId: current.id,
        title: saveTitle.trim() || current.title,
        categoryId: saveCategoryId || null,
        tags: parseTags(saveTags),
        keepFullText: saveKeepFullText,
      });
      setSaveOpen(false);
      setCurrent(await contentDiscoveryService.get(current.id));
      setCaptures(await contentDiscoveryService.list(null, 60));
      toast.success(`已写入笔记「${result.noteTitle}」`);
    } catch (error) {
      reportFailure('写入知识库失败', errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const ignoreCapture = async () => {
    if (!current) return;
    setBusy(true);
    try {
      const ignored = await contentDiscoveryService.ignore(current.id);
      setCurrent(ignored);
      setCaptures(await contentDiscoveryService.list(null, 60));
      toast.info('已忽略这条内容');
    } catch (error) {
      reportFailure('忽略内容失败', errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const toggleCandidate = (candidate: ContentDiscoveryCandidate) => {
    setSelectedUrls((previous) => {
      const next = new Set(previous);
      if (next.has(candidate.canonicalUrl)) next.delete(candidate.canonicalUrl);
      else next.add(candidate.canonicalUrl);
      return next;
    });
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 overflow-y-auto p-6">
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="flex items-center gap-2 text-lg font-semibold text-[var(--ink)]">
            <Compass className="h-5 w-5 text-[var(--brand)]" />
            内容发现
          </h1>
          <p className="mt-1 max-w-3xl text-sm text-[var(--ink-muted)]">
            把看到的知识网站、微信公众号或 Git 内容收进待确认收件箱，按需勾选链接深入抓取，
            再生成摘要和整理建议。
          </p>
        </div>
        <span className="inline-flex items-center gap-1 rounded-full bg-[var(--surface-muted)] px-2.5 py-1 text-[11px] text-[var(--ink-subtle)]">
          <ShieldCheck className="h-3.5 w-3.5" />
          静态抓取 · 不执行页面脚本
        </span>
      </header>

      {!networkReady && policy ? (
        <div className="rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-sm text-amber-900 dark:text-amber-200">
          内容发现需要先在
          <Link className="mx-1 underline" to={ROUTES.settings}>
            设置 → AI → 联网与增强
          </Link>
          开启“允许联网检索”并确认一次。
        </div>
      ) : null}

      {loadError ? (
        <p className="rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-sm text-red-700 dark:text-red-200">
          读取内容收件箱失败：{loadError}
        </p>
      ) : null}

      <div className="grid min-h-0 gap-4 lg:grid-cols-[300px_minmax(0,1fr)]">
        <aside className="rounded-xl border border-[var(--border-subtle)] p-3">
          <div className="flex items-center justify-between gap-2">
            <h2 className="text-sm font-medium">内容收件箱</h2>
            <span className="text-[11px] text-[var(--ink-subtle)]">{captures.length}</span>
          </div>
          <div className="mt-3 space-y-1.5">
            {captures.length === 0 ? (
              <p className="rounded-lg border border-dashed border-[var(--border-subtle)] p-4 text-center text-xs text-[var(--ink-subtle)]">
                还没有发现内容。
              </p>
            ) : (
              captures.map((capture) => (
                <button
                  key={capture.id}
                  type="button"
                  data-testid={`content-capture-${capture.id}`}
                  onClick={() => void openCapture(capture.id)}
                  className={cn(
                    'w-full rounded-lg border px-3 py-2 text-left transition-colors',
                    current?.id === capture.id
                      ? 'border-[var(--brand)] bg-[var(--brand-soft)]'
                      : 'border-[var(--border-subtle)] hover:bg-[var(--surface-muted)]',
                  )}
                >
                  <p className="line-clamp-2 text-xs font-medium text-[var(--ink)]">
                    {capture.title}
                  </p>
                  <div className="mt-1 flex items-center justify-between gap-2">
                    <span
                      className={cn(
                        'rounded-full px-1.5 py-0.5 text-[10px]',
                        STATUS_TONE[capture.status],
                      )}
                    >
                      {STATUS_LABEL[capture.status]}
                    </span>
                    <span className="truncate text-[10px] text-[var(--ink-subtle)]">
                      {capture.site ?? capture.sourceKind}
                    </span>
                  </div>
                </button>
              ))
            )}
          </div>
        </aside>

        <main className="min-w-0 space-y-4">
          <section className="rounded-xl border border-[var(--border-subtle)] p-4">
            <h2 className="text-sm font-medium">新建内容发现</h2>
            <div className="mt-3 grid gap-3">
              <div>
                <Label htmlFor="content-discovery-url">网页 URL 或分享文本</Label>
                <Input
                  id="content-discovery-url"
                  data-testid="content-discovery-url"
                  value={url}
                  onChange={(event) => setUrl(event.target.value)}
                  placeholder="https://example.com/article 或粘贴包含链接的分享文本"
                />
              </div>
              <div>
                <Label htmlFor="content-discovery-note">为什么保存（可选）</Label>
                <Input
                  id="content-discovery-note"
                  data-testid="content-discovery-note"
                  value={userNote}
                  onChange={(event) => setUserNote(event.target.value)}
                  placeholder="例如：准备用于 Rust 调度方案选型"
                />
              </div>
              <div className="flex flex-wrap items-center justify-between gap-2">
                <p className="text-[11px] text-[var(--ink-subtle)]">
                  只会读取公开页面；本次抓取需要你明确确认。
                </p>
                <Button
                  data-testid="content-discovery-create"
                  disabled={busy || !networkReady}
                  onClick={openCaptureConfirm}
                >
                  {busy ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <FileSearch className="h-4 w-4" />
                  )}
                  开始发现
                </Button>
              </div>
            </div>
          </section>

          {current ? (
            <>
              <section
                data-testid="content-discovery-detail"
                className="rounded-xl border border-[var(--border-subtle)] p-4"
              >
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex flex-wrap items-center gap-2">
                      <h2 className="truncate text-base font-semibold">{current.title}</h2>
                      <span
                        className={cn(
                          'rounded-full px-2 py-0.5 text-[10px]',
                          STATUS_TONE[current.status],
                        )}
                      >
                        {STATUS_LABEL[current.status]}
                      </span>
                    </div>
                    <a
                      href={current.finalUrl}
                      target="_blank"
                      rel="noreferrer"
                      className="mt-1 inline-flex items-center gap-1 text-xs text-[var(--brand)] hover:underline"
                    >
                      {current.site ?? current.finalUrl}
                      <ExternalLink className="h-3 w-3" />
                    </a>
                  </div>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="secondary"
                      data-testid="content-discovery-refresh"
                      disabled={busy || !networkReady || current.status === 'ignored'}
                      onClick={() => setPendingAction('refresh')}
                    >
                      <RefreshCw className="h-3.5 w-3.5" />
                      检查来源更新
                    </Button>
                    {current.noteId ? (
                      <Button asChild size="sm" variant="secondary">
                        <Link to={ROUTES.note(current.noteId)}>打开笔记</Link>
                      </Button>
                    ) : (
                      <>
                        <Button
                          size="sm"
                          variant="secondary"
                          data-testid="content-discovery-ignore"
                          disabled={busy || current.status === 'ignored'}
                          onClick={() => void ignoreCapture()}
                        >
                          <Trash2 className="h-3.5 w-3.5" />
                          忽略
                        </Button>
                        <Button
                          size="sm"
                          data-testid="content-discovery-save-open"
                          disabled={busy || current.status === 'ignored'}
                          onClick={() => {
                            setSaveTitle(current.title);
                            setSaveCategoryId(current.suggestedCategoryId ?? '');
                            setSaveTags(current.tags.join(', '));
                            setSaveKeepFullText(current.keepFullText);
                            setSaveOpen(true);
                          }}
                        >
                          <Save className="h-3.5 w-3.5" />
                          写入知识库
                        </Button>
                      </>
                    )}
                  </div>
                </div>
                {current.userNote ? (
                  <p className="mt-3 rounded-md bg-[var(--surface-muted)] px-3 py-2 text-xs text-[var(--ink-muted)]">
                    保存动机：{current.userNote}
                  </p>
                ) : null}
                {current.status === 'ignored' ? (
                  <div
                    data-testid="content-discovery-save-state"
                    className="mt-3 rounded-md border border-[var(--border-subtle)] bg-[var(--surface-muted)] px-3 py-2 text-xs text-[var(--ink-muted)]"
                  >
                    已忽略，未写入知识库。
                  </div>
                ) : current.noteId ? (
                  <div
                    data-testid="content-discovery-save-state"
                    className="mt-3 flex flex-wrap items-center gap-2 rounded-md border border-emerald-500/40 bg-emerald-500/10 px-3 py-2 text-xs text-emerald-800 dark:text-emerald-200"
                  >
                    <Check className="h-4 w-4" />
                    <span>已写入知识库。</span>
                    <Link className="font-medium underline" to={ROUTES.note(current.noteId)}>
                      打开对应笔记
                    </Link>
                  </div>
                ) : (
                  <div
                    data-testid="content-discovery-save-state"
                    className="mt-3 rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-900 dark:text-amber-200"
                  >
                    尚未写入知识库。当前内容只保存在内容收件箱；确认抓取正文无误后，请点击右上角“写入知识库”并再次确认。
                  </div>
                )}
                {progress?.captureId === current.id ? (
                  <div className="mt-3 rounded-md border border-sky-500/30 bg-sky-500/10 px-3 py-2 text-xs text-sky-800 dark:text-sky-200">
                    {progress.status === 'fetching'
                      ? `正在抓取 ${progress.completed}/${progress.total}${progress.currentUrl ? ` · ${progress.currentUrl}` : ''}`
                      : '页面抓取完成，正在生成汇总。'}
                  </div>
                ) : null}
                {current.sourceChanged ? (
                  <div className="mt-3 rounded-md border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-amber-900 dark:text-amber-200">
                    来源正文已变化，收件箱预览已更新；正式笔记没有被自动覆盖。
                  </div>
                ) : null}
                {current.summary ? (
                  <div className="mt-3 space-y-2">
                    <h3 className="text-xs font-medium text-[var(--ink)]">内容摘要</h3>
                    <p className="text-sm leading-relaxed text-[var(--ink-muted)]">
                      {current.summary}
                    </p>
                    {current.keyPoints.length > 0 ? (
                      <ul className="list-disc space-y-1 pl-5 text-xs text-[var(--ink-muted)]">
                        {current.keyPoints.map((point) => (
                          <li key={point}>{point}</li>
                        ))}
                      </ul>
                    ) : null}
                  </div>
                ) : null}
                <div className="mt-3 flex flex-wrap gap-2 text-[11px] text-[var(--ink-subtle)]">
                  <span>页面 {current.pageCount}</span>
                  <span>正文 {current.wordCount} 字</span>
                  <span>来源 {current.sourceKind}</span>
                  {current.suggestedCategoryId ? (
                    <span className="rounded-full bg-[var(--surface-muted)] px-2 py-0.5">
                      建议分类：
                      {categories.find((item) => item.id === current.suggestedCategoryId)?.name ??
                        '已匹配分类'}
                    </span>
                  ) : null}
                  {current.tags.map((tag) => (
                    <span key={tag} className="rounded-full bg-[var(--surface-muted)] px-2 py-0.5">
                      {tag}
                    </span>
                  ))}
                </div>
                {current.relatedNotes.length > 0 ? (
                  <div className="mt-3 rounded-lg border border-[var(--border-subtle)] px-3 py-2">
                    <p className="text-[11px] font-medium text-[var(--ink-muted)]">相关笔记建议</p>
                    <div className="mt-1 flex flex-wrap gap-x-3 gap-y-1">
                      {current.relatedNotes.map((note) => (
                        <Link
                          key={note.noteId}
                          to={ROUTES.note(note.noteId)}
                          className="text-xs text-[var(--brand)] hover:underline"
                        >
                          {note.title}
                        </Link>
                      ))}
                    </div>
                  </div>
                ) : null}
              </section>

              <section
                data-testid="content-discovery-preview"
                className="rounded-xl border border-[var(--border-subtle)] p-4"
              >
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div>
                    <h2 className="flex items-center gap-2 text-sm font-medium">
                      <FileText className="h-4 w-4 text-[var(--brand)]" />
                      抓取正文预览
                    </h2>
                    <p className="mt-0.5 text-[11px] text-[var(--ink-subtle)]">
                      这里显示实际抽取并汇总的 Markdown
                      内容；未入库时它位于临时缓存，不会创建正式笔记。
                    </p>
                  </div>
                  <Button
                    size="sm"
                    variant="secondary"
                    data-testid="content-discovery-preview-toggle"
                    onClick={() => setPreviewOpen((value) => !value)}
                  >
                    {previewOpen ? (
                      <ChevronUp className="h-3.5 w-3.5" />
                    ) : (
                      <ChevronDown className="h-3.5 w-3.5" />
                    )}
                    {previewOpen ? '收起正文' : '查看正文'}
                  </Button>
                </div>
                {previewOpen ? (
                  <div
                    data-testid="content-discovery-preview-content"
                    className="mt-3 max-h-[520px] overflow-y-auto rounded-lg border border-[var(--border-subtle)] bg-[var(--surface)]"
                  >
                    <MarkdownPreview content={current.markdown ?? ''} />
                  </div>
                ) : null}
              </section>

              <section className="rounded-xl border border-[var(--border-subtle)] p-4">
                <div className="flex flex-wrap items-center justify-between gap-2">
                  <div>
                    <h2 className="text-sm font-medium">发现的相关链接</h2>
                    <p className="mt-0.5 text-[11px] text-[var(--ink-subtle)]">
                      根页面始终保留；勾选链接后最多追加 {options.maxPages - 1} 页。
                    </p>
                  </div>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() =>
                        setSelectedUrls(
                          new Set(selectableCandidates.map((item) => item.canonicalUrl)),
                        )
                      }
                    >
                      全选可抓取
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => setSelectedUrls(new Set())}>
                      清空
                    </Button>
                    <Button
                      size="sm"
                      data-testid="content-discovery-crawl"
                      disabled={busy || selectedUrls.size === 0}
                      onClick={openCrawlConfirm}
                    >
                      {busy ? (
                        <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      ) : (
                        <ChevronRight className="h-3.5 w-3.5" />
                      )}
                      抓取选中 {selectedUrls.size}
                    </Button>
                  </div>
                </div>

                <div className="mt-3 flex flex-wrap gap-4 rounded-lg bg-[var(--surface-muted)] px-3 py-2">
                  <OptionSwitch
                    label="同路径子树"
                    checked={options.samePath}
                    onChange={(samePath) => setOptions((value) => ({ ...value, samePath }))}
                  />
                  <OptionSwitch
                    label="包含隐藏/折叠链接"
                    checked={options.includeHidden}
                    onChange={(includeHidden) =>
                      setOptions((value) => ({ ...value, includeHidden }))
                    }
                  />
                  <OptionSwitch
                    label="包含子域名"
                    checked={options.includeSubdomains}
                    onChange={(includeSubdomains) =>
                      setOptions((value) => ({ ...value, includeSubdomains }))
                    }
                  />
                  <OptionSwitch
                    label="允许跨域"
                    checked={options.includeExternal}
                    onChange={(includeExternal) =>
                      setOptions((value) => ({ ...value, includeExternal }))
                    }
                  />
                  <OptionSwitch
                    label="保存完整原文"
                    checked={options.keepFullText}
                    onChange={(keepFullText) => setOptions((value) => ({ ...value, keepFullText }))}
                  />
                </div>

                <div className="mt-3 max-h-80 space-y-1 overflow-y-auto">
                  {current.candidates.length === 0 ? (
                    <p className="rounded-lg border border-dashed border-[var(--border-subtle)] p-4 text-center text-xs text-[var(--ink-subtle)]">
                      当前页面没有发现可探索链接。
                    </p>
                  ) : (
                    current.candidates.map((candidate) => {
                      const selectable = candidateCanSelect(candidate, options);
                      return (
                        <button
                          key={candidate.canonicalUrl}
                          type="button"
                          data-testid="content-candidate"
                          disabled={!selectable}
                          onClick={() => toggleCandidate(candidate)}
                          className={cn(
                            'flex w-full items-start gap-2 rounded-lg border px-3 py-2 text-left transition-colors',
                            selectedUrls.has(candidate.canonicalUrl)
                              ? 'border-[var(--brand)] bg-[var(--brand-soft)]'
                              : 'border-[var(--border-subtle)]',
                            selectable
                              ? 'hover:bg-[var(--surface-muted)]'
                              : 'cursor-not-allowed opacity-55',
                          )}
                        >
                          <span
                            className={cn(
                              'mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded border',
                              selectedUrls.has(candidate.canonicalUrl)
                                ? 'border-[var(--brand)] bg-[var(--brand)] text-white'
                                : 'border-[var(--border-subtle)]',
                            )}
                          >
                            {selectedUrls.has(candidate.canonicalUrl) ? (
                              <Check className="h-3 w-3" />
                            ) : null}
                          </span>
                          <span className="min-w-0 flex-1">
                            <span className="block truncate text-xs font-medium text-[var(--ink)]">
                              {candidate.title}
                            </span>
                            <span className="mt-0.5 block truncate text-[10px] text-[var(--ink-subtle)]">
                              {candidate.url}
                            </span>
                            {candidate.blockedReason ? (
                              <span className="mt-1 inline-block rounded bg-amber-500/10 px-1.5 py-0.5 text-[10px] text-amber-700 dark:text-amber-300">
                                {blockedReasonLabel(candidate.blockedReason)}
                              </span>
                            ) : null}
                          </span>
                        </button>
                      );
                    })
                  )}
                </div>
              </section>

              {current.pages.length > 0 ? (
                <section className="rounded-xl border border-[var(--border-subtle)] p-4">
                  <h2 className="text-sm font-medium">已抓取页面</h2>
                  <div className="mt-2 space-y-1">
                    {current.pages.map((page) => (
                      <div
                        key={page.canonicalUrl}
                        className="flex items-center justify-between gap-3 rounded-md bg-[var(--surface-muted)] px-3 py-1.5 text-xs"
                      >
                        <span className="min-w-0 truncate">{page.title}</span>
                        <span
                          className={
                            page.status === 'ready'
                              ? 'shrink-0 text-emerald-600 dark:text-emerald-300'
                              : 'shrink-0 text-red-600 dark:text-red-300'
                          }
                        >
                          {page.status === 'ready' ? `${page.wordCount} 字` : '失败'}
                        </span>
                      </div>
                    ))}
                  </div>
                </section>
              ) : null}
            </>
          ) : null}
        </main>
      </div>

      <Dialog
        open={pendingAction !== null}
        onOpenChange={(open) => !open && setPendingAction(null)}
      >
        <DialogContent data-testid="content-discovery-network-confirm">
          <DialogHeader>
            <DialogTitle>确认本次联网抓取</DialogTitle>
            <DialogDescription>
              {pendingAction === 'capture'
                ? '将访问你填写的公开页面并解析已有 HTML 链接；已授权时还会把链接标题和上下文交给 Decision Model 筛选。不会执行脚本，也不会上传知识库正文。'
                : pendingAction === 'crawl'
                  ? `将访问你勾选的 ${selectedUrls.size} 个链接。默认遵守域名策略、robots.txt、页数和时间限制；已授权模型只会看到链接标题与上下文。`
                  : '将重新读取原始来源并比较正文哈希。预览可以更新，但不会自动覆盖已经保存的正式笔记。'}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button variant="secondary" onClick={() => setPendingAction(null)}>
              取消
            </Button>
            <Button
              data-testid="content-discovery-network-confirm-ok"
              disabled={busy}
              onClick={() => void executePending()}
            >
              仅此一次，继续
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={saveOpen} onOpenChange={setSaveOpen}>
        <DialogContent data-testid="content-discovery-save-dialog">
          <DialogHeader>
            <DialogTitle>写入知识库</DialogTitle>
            <DialogDescription>
              会先保存为普通 Markdown 笔记；临时网页缓存不会进入备份。
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div>
              <Label htmlFor="content-save-title">笔记标题</Label>
              <Input
                id="content-save-title"
                data-testid="content-save-title"
                value={saveTitle}
                onChange={(event) => setSaveTitle(event.target.value)}
              />
            </div>
            <div>
              <Label htmlFor="content-save-category">分类</Label>
              <select
                id="content-save-category"
                value={saveCategoryId}
                onChange={(event) => setSaveCategoryId(event.target.value)}
                className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm"
              >
                <option value="">Inbox（不分类）</option>
                {categories.map((category) => (
                  <option key={category.id} value={category.id}>
                    {category.name}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <Label htmlFor="content-save-tags">标签（逗号分隔）</Label>
              <Input
                id="content-save-tags"
                value={saveTags}
                onChange={(event) => setSaveTags(event.target.value)}
                placeholder="例如：Rust, 调度"
              />
            </div>
            <div className="rounded-lg border border-[var(--border-subtle)] bg-[var(--surface-muted)] px-3 py-2">
              <OptionSwitch
                label="保存完整抓取正文"
                checked={saveKeepFullText}
                onChange={setSaveKeepFullText}
              />
              <p className="mt-1 pl-8 text-[11px] text-[var(--ink-subtle)]">
                {saveKeepFullText
                  ? `将保存当前约 ${current?.wordCount ?? 0} 字的抓取正文。`
                  : '关闭时只保存摘要、关键点和约 1,800 字摘录，完整原文仍可从来源重新读取。'}
              </p>
            </div>
          </div>
          <DialogFooter>
            <Button variant="secondary" onClick={() => setSaveOpen(false)}>
              取消
            </Button>
            <Button
              data-testid="content-discovery-save"
              disabled={busy || !saveTitle.trim()}
              onClick={() => void saveCapture()}
            >
              确认写入
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

function OptionSwitch({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-[11px] text-[var(--ink-muted)]">
      <Switch checked={checked} onCheckedChange={onChange} aria-label={label} />
      {label}
    </label>
  );
}

function candidateCanSelect(
  candidate: ContentDiscoveryCandidate,
  options: ContentDiscoveryOptions,
) {
  if (candidate.hidden && !options.includeHidden) return false;
  if (
    !candidate.sameOrigin &&
    !options.includeExternal &&
    !(options.includeSubdomains && candidate.sameSite)
  ) {
    return false;
  }
  if (candidate.blockedReason === 'outsidePath' && options.samePath) return false;
  return (
    !candidate.blockedReason ||
    ['hidden', 'external', 'outsidePath'].includes(candidate.blockedReason)
  );
}

function blockedReasonLabel(reason: string) {
  switch (reason) {
    case 'hidden':
      return '隐藏/折叠链接，需要开启选项';
    case 'external':
      return '跨域链接，需要开启选项';
    case 'outsidePath':
      return '不在当前路径子树';
    case 'decisionLowRelevance':
      return 'Decision Model 判定明显低相关';
    default:
      return reason;
  }
}

function defaultSelectedUrls(capture: ContentCapture) {
  return new Set(
    capture.candidates
      .filter((candidate) => candidate.selected && candidateCanSelect(candidate, DEFAULT_OPTIONS))
      .map((candidate) => candidate.canonicalUrl),
  );
}

function parseTags(value: string) {
  return [
    ...new Set(
      value
        .split(/[,，\n]/)
        .map((tag) => tag.trim())
        .filter(Boolean),
    ),
  ];
}
