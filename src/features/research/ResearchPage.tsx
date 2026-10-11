import {
  Ban,
  Check,
  CircleDot,
  FileText,
  FlaskConical,
  Loader2,
  Sparkles,
  TriangleAlert,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { MarkdownPreview } from '@/components/editor/MarkdownPreview';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input, Label } from '@/components/ui/input';
import { EVENTS, ROUTES } from '@/lib/constants';
import { errorMessage, formatDateTime } from '@/lib/format';
import { cn } from '@/lib/utils';
import { aiService } from '@/services/aiService';
import { subscribe } from '@/services/api';
import { researchService } from '@/services/researchService';
import type {
  AiPolicy,
  ResearchSession,
  ResearchProgress,
  ResearchSessionSummary,
  ResearchReport,
  ResearchSourceLevel,
  ResearchStatus,
  ResearchWriteMode,
} from '@/types';

/** 五步流程（与后端状态机一一对应）。 */
const STEPS: Array<{ status: ResearchStatus; label: string }> = [
  { status: 'planned', label: '① 立项' },
  { status: 'collected', label: '② 收集' },
  { status: 'verified', label: '③ 验证' },
  { status: 'composed', label: '④ 汇总' },
  { status: 'proposed', label: '⑤ 写入提案' },
  { status: 'written', label: '已落库' },
];

const LEVEL_TONE: Record<ResearchSourceLevel, string> = {
  A: 'bg-emerald-500/15 text-emerald-700 dark:text-emerald-300',
  B: 'bg-sky-500/15 text-sky-700 dark:text-sky-300',
  C: 'bg-slate-500/15 text-slate-700 dark:text-slate-300',
  D: 'bg-amber-500/15 text-amber-700 dark:text-amber-300',
  E: 'bg-rose-500/15 text-rose-700 dark:text-rose-300',
};

const STATUS_LABEL: Record<ResearchStatus, string> = {
  planned: '已立项',
  collected: '已收集',
  verified: '已验证',
  composed: '已汇总',
  proposed: '待确认写入',
  written: '已写入知识库',
  aborted: '已中止',
};

function statusIndex(status: ResearchStatus): number {
  if (status === 'aborted') return -1;
  return STEPS.findIndex((step) => step.status === status);
}

/**
 * 报告卡上的「平均置信度」：取结论段与关键事实段（有打分的那些）的均值。
 *
 * 与后端 `confidence_label` 同阈值（≥0.7 高 / ≥0.45 中 / 其余低），
 * 只是把后端给的分数换成一个一眼能看到的总览。
 */
function averageConfidence(report: ResearchReport): { label: string; value: number } | null {
  const scored = [...report.conclusions, ...report.keyFacts]
    .map((claim) => claim.confidence)
    .filter((value): value is number => typeof value === 'number');
  if (scored.length === 0) return null;
  const value = scored.reduce((sum, item) => sum + item, 0) / scored.length;
  const label = value >= 0.7 ? '高' : value >= 0.45 ? '中' : '低';
  return { label, value };
}

/**
 * 研究助手（V1.2.1 阶段 D2，对标语雀「深入调研」的思路）。
 *
 * 页面只负责推进五步与确认写入；闸门、分级、报告结构与写入纪律都在后端，
 * 因此这里不重复任何判定逻辑。默认只检索本地知识库（外部检索属 D3）。
 */
export function ResearchPage() {
  const navigate = useNavigate();
  const [policy, setPolicy] = useState<AiPolicy | null>(null);
  const [topic, setTopic] = useState('');
  const [scope, setScope] = useState('');
  const [session, setSession] = useState<ResearchSession | null>(null);
  const [sessions, setSessions] = useState<ResearchSessionSummary[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [writeOpen, setWriteOpen] = useState(false);
  const [writeTitle, setWriteTitle] = useState('');
  const [writeMode, setWriteMode] = useState<ResearchWriteMode>('create');
  const [includeSpeculation, setIncludeSpeculation] = useState(false);
  const [createSourceNote, setCreateSourceNote] = useState(false);
  /** 本次页面生命周期内是否已经为「联网收集」做过单次确认（§9.7） */
  const [networkConfirmed, setNetworkConfirmed] = useState(false);
  const [networkAskOpen, setNetworkAskOpen] = useState(false);
  /** 后端推来的实时进度（§9.2：当前步骤 / 已抓页数 / 已采纳来源数；只含元数据） */
  const [progress, setProgress] = useState<ResearchProgress | null>(null);

  const refreshHistory = useCallback(async () => {
    try {
      setSessions(await researchService.list(20));
    } catch {
      setSessions([]);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    aiService
      .policy()
      .then((value) => {
        if (!cancelled) setPolicy(value);
      })
      .catch(() => {
        if (!cancelled) setPolicy(null);
      });
    researchService
      .list(20)
      .then((items) => {
        if (!cancelled) setSessions(items);
      })
      .catch(() => {
        if (!cancelled) setSessions([]);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  // 订阅研究进度事件：长步骤（联网抓取可达数分钟）时让用户看得见「进行到哪了」。
  // 事件通道不可用（浏览器预览 / 测试环境）时静默降级，不影响主流程。
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void subscribe<ResearchProgress>(EVENTS.researchProgress, (payload) => {
      if (disposed) return;
      setProgress(payload);
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

  const report = session?.report ?? null;
  const proposal = session?.proposal ?? null;
  const currentIndex = session ? statusIndex(session.status) : -1;

  const nextAction = useMemo(() => {
    if (!session) return null;
    switch (session.status) {
      case 'planned':
        return { key: 'research-collect', label: '开始收集材料' };
      case 'collected':
        return { key: 'research-verify', label: '开始交叉验证' };
      case 'verified':
        return { key: 'research-compose', label: '生成调研报告' };
      case 'composed':
        return { key: 'research-proposal', label: '生成写入提案' };
      case 'proposed':
        return { key: 'research-write-next', label: '写入知识库…' };
      default:
        return null;
    }
  }, [session]);

  const run = async (key: string, task: () => Promise<ResearchSession>) => {
    setBusy(key);
    setError(null);
    try {
      const next = await task();
      setSession(next);
      await refreshHistory();
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(null);
    }
  };

  const start = async () => {
    setBusy('research-start');
    setError(null);
    try {
      const created = await researchService.start({
        topic,
        scope: scope.trim() ? scope.trim() : null,
        writeBack: true,
      });
      setSession(created);
      await refreshHistory();
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(null);
    }
  };

  const abort = async () => {
    if (!session) return;
    await run('research-abort', () => researchService.abort(session.id, '用户中止'));
    toast.info('已中止：已得材料保留为草稿。');
  };

  const openWriteDialog = () => {
    if (!proposal) return;
    setWriteTitle(proposal.title);
    setWriteMode(proposal.duplicates.length > 0 ? 'append' : 'create');
    setIncludeSpeculation(false);
    setCreateSourceNote(proposal.suggestSourceNote);
    setWriteOpen(true);
  };

  /**
   * 收集材料：联网档位打开且本次还没确认过时，先弹一次说明（§9.7「首次发起研究」）。
   * 本地口径（默认）不会看到这个对话框。
   */
  const startCollect = () => {
    if (!session) return;
    const needsConfirm = policy?.network === 'allowNetwork' && !networkConfirmed;
    if (needsConfirm) {
      setNetworkAskOpen(true);
      return;
    }
    void run('research-collect', () => researchService.collect(session.id, networkConfirmed));
  };

  const confirmNetworkCollect = () => {
    setNetworkAskOpen(false);
    setNetworkConfirmed(true);
    if (!session) return;
    void run('research-collect', () => researchService.collect(session.id, true));
  };

  const confirmWrite = async () => {
    if (!session) return;
    setBusy('research-write');
    setError(null);
    try {
      const result = await researchService.write({
        sessionId: session.id,
        mode: writeMode,
        targetNoteId: proposal?.duplicates[0]?.noteId ?? null,
        includeSpeculation,
        createSourceNote,
        title: writeTitle,
      });
      setWriteOpen(false);
      if (result.outcome === 'abandoned') {
        toast.info('已放弃写入：没有新建或修改任何笔记。');
      } else {
        toast.success(
          result.outcome === 'created' ? '已写入 Inbox（可回滚）' : '已追加到已有笔记（可回滚）',
        );
      }
      setSession(await researchService.get(session.id));
      await refreshHistory();
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(null);
    }
  };

  const openSession = async (id: string) => {
    setBusy('research-open');
    setError(null);
    try {
      setSession(await researchService.get(id));
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setBusy(null);
    }
  };

  const aiDisabled = policy !== null && !policy.enabled;

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 overflow-y-auto p-6">
      <header className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h1 className="flex items-center gap-2 text-lg font-semibold text-[var(--ink)]">
            <FlaskConical className="h-5 w-5 text-[var(--brand)]" />
            研究助手
          </h1>
          <p className="mt-1 max-w-3xl text-sm text-[var(--ink-muted)]">
            用五步把「一个课题」做成可复核的调研报告：立项 → 收集 → 验证 → 汇总 → 写入提案。
            <span className="text-[var(--ink-subtle)]">
              {' '}
              当前只检索本地知识库（外部检索属阶段 D3，接入前不会发起任何网络请求）。
            </span>
          </p>
        </div>
        {session && session.status !== 'written' && session.status !== 'aborted' ? (
          <Button variant="ghost" data-testid="research-abort" onClick={() => void abort()}>
            <Ban className="h-4 w-4" />
            中止并保留草稿
          </Button>
        ) : null}
      </header>

      {aiDisabled ? (
        <div
          data-testid="research-ai-disabled"
          className="flex items-start gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-sm text-amber-900 dark:text-amber-200"
        >
          <TriangleAlert className="mt-0.5 h-4 w-4 shrink-0" />
          <div>
            AI 能力当前是关闭的（默认口径）。研究助手不会自行打开或联网——请先到{' '}
            <Link className="underline" to={ROUTES.settings}>
              设置 → AI
            </Link>{' '}
            打开总开关，并保持「仅本地」即可使用本地调研。
          </div>
        </div>
      ) : null}

      <div className="grid min-h-0 gap-4 lg:grid-cols-[320px_minmax(0,1fr)]">
        <aside className="flex flex-col gap-4">
          <section className="rounded-xl border border-[var(--border-subtle)] p-4">
            <h2 className="text-sm font-medium text-[var(--ink)]">新建调研</h2>
            <div className="mt-3 flex flex-col gap-3">
              <div>
                <Label htmlFor="research-topic">课题</Label>
                <Input
                  id="research-topic"
                  data-testid="research-topic"
                  placeholder="例如：Rust 异步运行时的调度策略"
                  value={topic}
                  onChange={(event) => setTopic(event.target.value)}
                />
              </div>
              <div>
                <Label htmlFor="research-scope">范围（可选）</Label>
                <Input
                  id="research-scope"
                  data-testid="research-scope"
                  placeholder="领域 / 关注点 / 时间窗"
                  value={scope}
                  onChange={(event) => setScope(event.target.value)}
                />
              </div>
              <Button
                data-testid="research-start"
                disabled={!topic.trim() || busy !== null}
                onClick={() => void start()}
              >
                {busy === 'research-start' ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Sparkles className="h-4 w-4" />
                )}
                生成研究计划
              </Button>
              <p className="text-xs text-[var(--ink-subtle)]">
                课题过于宽泛（例如「AI」）会被要求补充范围，避免直接烧预算。
              </p>
            </div>
          </section>

          <section className="rounded-xl border border-[var(--border-subtle)] p-4">
            <h2 className="text-sm font-medium text-[var(--ink)]">最近的调研</h2>
            <div className="mt-2 flex flex-col gap-1" data-testid="research-history">
              {sessions.length === 0 ? (
                <p className="text-xs text-[var(--ink-subtle)]">还没有调研记录。</p>
              ) : (
                sessions.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    className={cn(
                      'flex flex-col items-start gap-0.5 rounded-md px-2 py-1.5 text-left text-xs transition-colors',
                      session?.id === item.id
                        ? 'bg-[var(--brand-soft)] text-[var(--brand)]'
                        : 'text-[var(--ink-muted)] hover:bg-[var(--surface-muted)]',
                    )}
                    onClick={() => void openSession(item.id)}
                  >
                    <span className="w-full truncate font-medium">{item.topic}</span>
                    <span className="text-[11px] text-[var(--ink-subtle)]">
                      {STATUS_LABEL[item.status]} · {item.sourceCount} 条来源
                    </span>
                  </button>
                ))
              )}
            </div>
          </section>
        </aside>

        <main className="flex min-h-0 flex-col gap-4 pb-6">
          {error ? (
            <div
              data-testid="research-error"
              className="rounded-lg border border-rose-500/40 bg-rose-500/10 p-3 text-sm text-rose-700 dark:text-rose-300"
            >
              {error}
            </div>
          ) : null}

          {!session ? (
            <p className="rounded-xl border border-dashed border-[var(--border-subtle)] p-6 text-sm text-[var(--ink-muted)]">
              左侧填写课题并「生成研究计划」开始；也可以从「最近的调研」里接着往下做。
            </p>
          ) : (
            <>
              <section className="flex flex-wrap items-center gap-2 rounded-xl border border-[var(--border-subtle)] p-3">
                {STEPS.map((step, index) => {
                  const done = currentIndex >= index && currentIndex >= 0;
                  return (
                    <span
                      key={step.status}
                      className={cn(
                        'inline-flex items-center gap-1 rounded-full px-2.5 py-1 text-xs',
                        done
                          ? 'bg-[var(--brand-soft)] text-[var(--brand)]'
                          : 'bg-[var(--surface-muted)] text-[var(--ink-subtle)]',
                      )}
                    >
                      {done ? <Check className="h-3 w-3" /> : <CircleDot className="h-3 w-3" />}
                      {step.label}
                    </span>
                  );
                })}
                {nextAction ? (
                  <Button
                    className="ml-auto"
                    data-testid={nextAction.key}
                    disabled={busy !== null}
                    onClick={() => {
                      if (nextAction.key === 'research-collect') {
                        startCollect();
                      } else if (nextAction.key === 'research-verify') {
                        void run('research-verify', () => researchService.verify(session.id));
                      } else if (nextAction.key === 'research-compose') {
                        void run('research-compose', () => researchService.compose(session.id));
                      } else if (nextAction.key === 'research-proposal') {
                        void run('research-proposal', () => researchService.proposal(session.id));
                      } else {
                        openWriteDialog();
                      }
                    }}
                  >
                    {busy === nextAction.key ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : null}
                    {nextAction.label}
                  </Button>
                ) : null}
                {session.status === 'written' && session.writtenNoteId ? (
                  <div className="ml-auto flex items-center gap-2" data-testid="research-written">
                    <Badge>已写入知识库</Badge>
                    <Button
                      data-testid="research-written-open"
                      onClick={() => navigate(ROUTES.note(session.writtenNoteId!))}
                    >
                      打开笔记
                    </Button>
                  </div>
                ) : null}
              </section>

              {/*
                实时进度（§9.2）：长步骤（联网抓取可达预算上限）时给出「进行到哪了」，
                只显示后端推来的元数据——当前步骤 / 说明 / 已抓页数 / 已采纳来源数。
              */}
              {busy !== null && progress ? (
                <p
                  data-testid="research-progress"
                  className="mt-2 flex flex-wrap items-center gap-2 text-xs text-[var(--ink-muted)]"
                >
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                  <span>{progress.detail}</span>
                  {progress.pagesFetched > 0 ? <span>已抓 {progress.pagesFetched} 页</span> : null}
                  {progress.sources > 0 ? <span>来源 {progress.sources} 条</span> : null}
                </p>
              ) : null}

              {session.plan ? (
                <section
                  data-testid="research-plan"
                  className="rounded-xl border border-[var(--border-subtle)] p-4"
                >
                  <div className="flex items-center justify-between">
                    <h2 className="text-sm font-medium text-[var(--ink)]">研究计划</h2>
                    <span className="text-xs text-[var(--ink-subtle)]">
                      时间窗 {session.plan.timeWindowDays} 天 ·{' '}
                      {formatDateTime(session.createdAt)}
                    </span>
                  </div>
                  <ol className="mt-3 grid gap-1.5 text-sm text-[var(--ink)]">
                    {session.plan.subQuestions.map((question) => (
                      <li key={question.id} className="flex items-start gap-2">
                        <span className="mt-0.5 text-xs text-[var(--ink-subtle)]">
                          {question.id}
                        </span>
                        <span className="flex-1">{question.question}</span>
                        <span className="text-xs text-[var(--ink-subtle)]">
                          {question.localHits} 条本地材料
                        </span>
                      </li>
                    ))}
                  </ol>
                  <ul className="mt-3 list-disc space-y-1 pl-5 text-xs text-[var(--ink-muted)]">
                    {session.plan.notes.map((note) => (
                      <li key={note}>{note}</li>
                    ))}
                  </ul>
                  {session.plan.localBaseline.length > 0 ? (
                    <p className="mt-3 text-xs text-[var(--ink-muted)]">
                      已知基线：
                      {session.plan.localBaseline.map((item) => `《${item.title}》`).join('、')}
                    </p>
                  ) : null}
                </section>
              ) : null}

              {session.sources.length > 0 ? (
                <section
                  data-testid="research-sources"
                  className="rounded-xl border border-[var(--border-subtle)] p-4"
                >
                  <h2 className="text-sm font-medium text-[var(--ink)]">
                    来源（{session.sources.length}）
                  </h2>
                  <ul className="mt-3 flex flex-col gap-2">
                    {session.sources.map((source) => (
                      <li key={source.reference} className="flex items-start gap-2 text-sm">
                        <span className="mt-0.5 text-xs text-[var(--ink-subtle)]">
                          [{source.reference}]
                        </span>
                        <span
                          className={cn(
                            'rounded px-1.5 py-0.5 text-[11px]',
                            LEVEL_TONE[source.level],
                          )}
                        >
                          {source.level}
                        </span>
                        <span className="flex-1">
                          {source.noteId ? (
                            <Link
                              className="text-[var(--ink)] underline decoration-dotted"
                              to={ROUTES.note(source.noteId)}
                            >
                              {source.title}
                            </Link>
                          ) : (
                            <span className="text-[var(--ink)]">{source.title}</span>
                          )}
                          <span className="block text-xs text-[var(--ink-subtle)]">
                            {source.gradeReason}
                            {source.stale ? ' · 可能已过时' : ''}
                            {typeof source.credibility === 'number'
                              ? ` · 可信度 ${Math.round(source.credibility * 100)}%`
                              : ''}
                          </span>
                        </span>
                      </li>
                    ))}
                  </ul>
                </section>
              ) : null}

              {report ? (
                <section
                  data-testid="research-report"
                  className="rounded-xl border border-[var(--border-subtle)] p-4"
                >
                  <div className="flex flex-wrap items-center gap-2">
                    <h2 className="text-sm font-medium text-[var(--ink)]">调研报告</h2>
                    <Badge>资料截至 {report.dataCutoff}</Badge>
                    <Badge>{report.conclusions.length} 条结论</Badge>
                    <Badge>{report.pending.length} 条待验证</Badge>
                    {averageConfidence(report) ? (
                      <Badge data-testid="research-avg-confidence">
                        平均置信度 {averageConfidence(report)!.label}{' '}
                        {averageConfidence(report)!.value.toFixed(2)}
                      </Badge>
                    ) : null}
                    <span className="text-xs text-[var(--ink-subtle)]">
                      生成方式 {report.composer}（未接入模型）
                    </span>
                    <span className="text-xs text-[var(--ink-subtle)]">
                      打分 {report.decisionProvider ?? 'rule'}（规则档；Jev 接入后同口径替换）
                    </span>
                  </div>

                  {report.warnings.length > 0 ? (
                    <ul
                      data-testid="research-warnings"
                      className="mt-3 flex flex-col gap-1.5 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-900 dark:text-amber-200"
                    >
                      {report.warnings.map((warning) => (
                        <li key={`${warning.kind}-${warning.message}`}>
                          <TriangleAlert className="mr-1 inline h-3.5 w-3.5" />
                          {warning.message}
                          {warning.references.length > 0
                            ? `（${warning.references.map((value) => `[${value}]`).join('')}）`
                            : ''}
                        </li>
                      ))}
                    </ul>
                  ) : null}

                  {/*
                    验收 1：结论段每一句都能点开对应引用。
                    正文里的 `[n]` 是给人看的编号，这里把编号接上真正可点的目标——
                    本地笔记跳转过去、外部来源用系统浏览器打开（悬停显示级别与获取时间）。
                  */}
                  {report.conclusions.length > 0 || report.keyFacts.length > 0 ? (
                    <div className="mt-4" data-testid="research-citations">
                      <h3 className="text-xs font-medium text-[var(--ink)]">结论与引用（可点开）</h3>
                      <ul className="mt-2 flex flex-col gap-1.5 text-sm">
                        {[...report.conclusions, ...report.keyFacts].map((claim, index) => (
                          <li
                            key={`${claim.subQuestionId}-${claim.text}-${index}`}
                            className="flex flex-wrap items-baseline gap-1.5"
                          >
                            <span className="flex-1 text-[var(--ink)]">{claim.text}</span>
                            {claim.citations.map((reference) => {
                              const source = report.references.find(
                                (item) => item.reference === reference,
                              );
                              if (!source) return null;
                              const label = `[${reference}] ${source.title}`;
                              const hint = `${source.level} 级 · 获取 ${formatDateTime(
                                source.fetchedAt,
                              )}${source.stale ? ' · 可能已过时' : ''}`;
                              return source.noteId ? (
                                <Link
                                  key={reference}
                                  data-testid="citation-note"
                                  to={ROUTES.note(source.noteId)}
                                  title={`${hint}（本地笔记）`}
                                  className="rounded bg-[var(--brand-soft)] px-1.5 py-0.5 text-xs text-[var(--brand)] hover:brightness-95"
                                >
                                  {label}
                                </Link>
                              ) : (
                                <a
                                  key={reference}
                                  data-testid="citation-url"
                                  href={source.url ?? '#'}
                                  target="_blank"
                                  rel="noreferrer"
                                  title={hint}
                                  className="rounded border border-[var(--border-subtle)] px-1.5 py-0.5 text-xs text-[var(--ink-muted)] hover:bg-[var(--surface-muted)]"
                                >
                                  {label}
                                </a>
                              );
                            })}
                          </li>
                        ))}
                      </ul>
                    </div>
                  ) : null}

                  {report.markdown ? (
                    <div className="mt-4 max-w-none">
                      <MarkdownPreview content={report.markdown} />
                    </div>
                  ) : (
                    <p className="mt-3 text-sm text-[var(--ink-muted)]">
                      已验证，等待「生成调研报告」渲染正文。
                    </p>
                  )}

                  {report.speculation.length > 0 ? (
                    <details
                      data-testid="research-speculation"
                      className="mt-4 rounded-lg border border-[var(--border-subtle)] p-3 text-xs text-[var(--ink-muted)]"
                    >
                      <summary className="cursor-pointer text-[var(--ink)]">
                        模型推测段（默认折叠，未接入模型，不是结论）
                      </summary>
                      <pre className="mt-2 whitespace-pre-wrap font-sans">
                        {report.speculation.join('\n')}
                      </pre>
                    </details>
                  ) : null}
                </section>
              ) : null}

              {proposal ? (
                <section
                  data-testid="research-proposal"
                  className="rounded-xl border border-[var(--border-subtle)] p-4"
                >
                  <h2 className="text-sm font-medium text-[var(--ink)]">写入提案</h2>
                  <p className="mt-2 text-sm text-[var(--ink-muted)]">
                    标题：<span className="text-[var(--ink)]">{proposal.title}</span> · 目标：
                    {proposal.categoryId ? '指定分类' : 'Inbox（未分类）'} · 来源可靠性{' '}
                    {proposal.sourceReliability}/5 · 复核时间 {proposal.validUntil ?? '未设置'}
                  </p>
                  {proposal.duplicates.length > 0 ? (
                    <p className="mt-2 rounded-md bg-amber-500/10 p-2 text-xs text-amber-900 dark:text-amber-200">
                      已有 {proposal.duplicates.length} 篇同标题笔记（默认不覆盖）：可改用「追加」或改名新建。
                    </p>
                  ) : null}
                  <Button
                    className="mt-3"
                    data-testid="research-write-open"
                    onClick={openWriteDialog}
                  >
                    打开写入确认…
                  </Button>
                </section>
              ) : null}
            </>
          )}
        </main>
      </div>

      {networkAskOpen ? (
        <div
          data-testid="research-network-confirm"
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
        >
          <div className="w-full max-w-lg rounded-xl bg-[var(--surface-raised)] p-5 shadow-xl">
            <h2 className="flex items-center gap-2 text-base font-semibold text-[var(--ink)]">
              <TriangleAlert className="h-4 w-4 text-amber-500" />
              本次将联网检索
            </h2>
            <ul className="mt-3 list-disc space-y-1 pl-5 text-sm text-[var(--ink-muted)]">
              <li>
                会访问你配置的检索源：
                <code className="ml-1 text-xs">
                  {policy?.searchEndpoint ?? '（未配置，将不会发出请求）'}
                </code>
              </li>
              <li>会上传检索关键词（不包含你的笔记正文与会话内容）。</li>
              <li>
                预计最多抓取 {policy?.budget.maxPages ?? 12} 页；命中黑名单的域名强制丢弃，
                网页内容按「数据」处理（疑似注入的页面直接丢弃）。
              </li>
              <li>结果只是提案：是否写入知识库由你在下一步另行确认。</li>
              <li>可随时在「设置 → AI」切回「仅本地」。</li>
            </ul>
            <div className="mt-4 flex justify-end gap-2">
              <Button
                variant="ghost"
                data-testid="research-network-cancel"
                onClick={() => setNetworkAskOpen(false)}
              >
                暂不联网（只用本地）
              </Button>
              <Button data-testid="research-network-ok" onClick={confirmNetworkCollect}>
                仅此一次，继续
              </Button>
            </div>
          </div>
        </div>
      ) : null}

      {writeOpen && proposal ? (
        <div
          data-testid="research-write-dialog"
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
        >
          <div className="max-h-full w-full max-w-2xl overflow-y-auto rounded-xl bg-[var(--surface-raised)] p-5 shadow-xl">
            <h2 className="flex items-center gap-2 text-base font-semibold text-[var(--ink)]">
              <FileText className="h-4 w-4" />
              写入知识库（需要你确认）
            </h2>
            <p className="mt-1 text-xs text-[var(--ink-muted)]">
              写前预览如下；写入后可随时在「版本历史」里回滚。AI 不会自行落库。
            </p>

            <div className="mt-4 flex flex-col gap-3">
              <div>
                <Label htmlFor="research-write-title">标题</Label>
                <Input
                  id="research-write-title"
                  data-testid="research-write-title"
                  value={writeTitle}
                  onChange={(event) => setWriteTitle(event.target.value)}
                />
              </div>

              <fieldset className="flex flex-wrap gap-3 text-sm">
                <legend className="mb-1 text-xs text-[var(--ink-muted)]">写入方式</legend>
                {(
                  [
                    ['create', '新建笔记（默认 Inbox）'],
                    ['append', '追加到同标题笔记'],
                    ['abandon', '放弃本次写入'],
                  ] as Array<[ResearchWriteMode, string]>
                ).map(([value, label]) => (
                  <label key={value} className="inline-flex items-center gap-1.5">
                    <input
                      type="radio"
                      name="research-write-mode"
                      data-testid={`research-write-mode-${value}`}
                      value={value}
                      checked={writeMode === value}
                      onChange={() => setWriteMode(value)}
                    />
                    <span>{label}</span>
                  </label>
                ))}
              </fieldset>

              {writeMode === 'append' && proposal.duplicates.length === 0 ? (
                <p className="rounded-md bg-amber-500/10 p-2 text-xs text-amber-900 dark:text-amber-200">
                  没有同标题笔记可追加：请改用「新建」。
                </p>
              ) : null}
              {writeMode === 'append' ? (
                <p className="rounded-md bg-[var(--surface-muted)] p-2 text-xs text-[var(--ink-muted)]">
                  追加只会在正文里加一段报告，<strong>不会覆盖</strong>目标笔记已有的来源与时效标注
                  （只在原字段为空时补上）。
                </p>
              ) : null}

              <label className="inline-flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  data-testid="research-write-speculation"
                  checked={includeSpeculation}
                  onChange={(event) => setIncludeSpeculation(event.target.checked)}
                />
                <span>附带写入「模型推测」附录（默认不写：它不是结论）</span>
              </label>
              <label className="inline-flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  data-testid="research-write-source-note"
                  checked={createSourceNote}
                  onChange={(event) => setCreateSourceNote(event.target.checked)}
                />
                <span>同时创建「来源清单」子文档</span>
              </label>

              <div>
                <p className="mb-1 text-xs text-[var(--ink-muted)]">写前 diff</p>
                <pre
                  data-testid="research-write-diff"
                  className="max-h-64 overflow-auto rounded-lg border border-[var(--border-subtle)] p-3 text-xs leading-relaxed"
                >
                  {proposal.diffPreview}
                </pre>
              </div>
            </div>

            <div className="mt-4 flex justify-end gap-2">
              <Button
                variant="ghost"
                data-testid="research-write-cancel"
                onClick={() => setWriteOpen(false)}
              >
                取消
              </Button>
              <Button
                data-testid="research-write-confirm"
                disabled={busy !== null}
                onClick={() => void confirmWrite()}
              >
                {busy === 'research-write' ? <Loader2 className="h-4 w-4 animate-spin" /> : null}
                确认写入
              </Button>
            </div>
          </div>
        </div>
      ) : null}
    </div>

  );
}
