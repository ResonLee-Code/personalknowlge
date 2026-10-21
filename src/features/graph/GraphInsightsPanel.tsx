import { ArrowUpRight, Copy, GitBranch, Loader2, Network, RefreshCw } from 'lucide-react';
import { useState } from 'react';
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
import type { DuplicateCluster, GraphInsights } from '@/types';
import { reportFailure } from '@/lib/actionFeedback';
import { errorMessage } from '@/lib/format';
import { noteService } from '@/services/noteService';
import { writingService } from '@/services/writingService';

interface GraphInsightsPanelProps {
  insights: GraphInsights | null;
  loading: boolean;
  error: string | null;
  focusTitle: string | null;
  onRetry: () => void;
  onOpen: (noteId: string) => void;
  onFocus: (noteId: string) => void;
}

function Score({ value }: { value: number }) {
  return (
    <span className="shrink-0 rounded bg-[var(--surface-muted)] px-1.5 py-0.5 text-[10px] tabular-nums text-[var(--ink-subtle)]">
      {Math.round(value * 100)}%
    </span>
  );
}

/** Graph 2.0 洞察栏：所有推荐都先展示证据，重复只提供非破坏性预览。 */
export function GraphInsightsPanel({
  insights,
  loading,
  error,
  focusTitle,
  onRetry,
  onOpen,
  onFocus,
}: GraphInsightsPanelProps) {
  const [duplicatePreview, setDuplicatePreview] = useState<DuplicateCluster | null>(null);
  const [summaryDraft, setSummaryDraft] = useState<Awaited<
    ReturnType<typeof writingService.draft>
  > | null>(null);
  const [summaryBusy, setSummaryBusy] = useState(false);

  if (loading && !insights) {
    return (
      <div className="flex items-center gap-2 p-3 text-xs text-[var(--ink-subtle)]">
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
        正在计算可解释连接…
      </div>
    );
  }
  if (error && !insights) {
    return (
      <div className="space-y-2 p-3">
        <p className="text-xs text-[var(--ink)]">洞察加载失败</p>
        <p className="text-[11px] leading-relaxed text-[var(--ink-subtle)]">{error}</p>
        <Button size="sm" variant="secondary" onClick={onRetry}>
          <RefreshCw className="h-3 w-3" /> 重试
        </Button>
      </div>
    );
  }
  if (!insights) return null;

  const createClusterSummary = async (cluster: DuplicateCluster) => {
    setSummaryBusy(true);
    try {
      const titles = cluster.members.map((member) => member.title).join('、');
      const draft = await writingService.draft({
        noteId: cluster.members[0]!.noteId,
        mode: 'shorten',
        instruction: `概括这组近重复笔记的共同主题、主要差异与建议保留内容：${titles}`,
        confirmOutbound: false,
      });
      setSummaryDraft(draft);
    } catch (cause) {
      reportFailure('生成聚类摘要失败', errorMessage(cause));
    } finally {
      setSummaryBusy(false);
    }
  };

  const saveClusterSummary = async (cluster: DuplicateCluster) => {
    if (!summaryDraft) return;
    setSummaryBusy(true);
    try {
      const sources = [
        ...new Map(summaryDraft.citations.map((citation) => [citation.noteId, citation])).values(),
      ]
        .map((citation) => `- [[${citation.title}]]`)
        .join('\n');
      const created = await noteService.create({
        title: `聚类摘要：${cluster.members[0]?.title ?? '近重复笔记'}`,
        initialContent: `${summaryDraft.text}\n\n## 写作来源\n\n${sources}\n`,
        categoryId: null,
        tags: ['图谱摘要'],
        parentId: null,
      });
      toast.success('聚类摘要已确认并写入知识库');
      setDuplicatePreview(null);
      setSummaryDraft(null);
      onOpen(created.note.id);
    } catch (cause) {
      reportFailure('写入聚类摘要失败', errorMessage(cause));
    } finally {
      setSummaryBusy(false);
    }
  };

  return (
    <div className="space-y-3 overflow-y-auto p-3" data-testid="graph-insights">
      <div className="flex items-center gap-2">
        <span className="rounded-full bg-[var(--surface-muted)] px-2 py-0.5 text-[10px] text-[var(--ink-muted)]">
          {insights.mode === 'vector' ? '本地向量 + 证据' : '本地词项 + 证据'}
        </span>
        <span className="text-[10px] text-[var(--ink-subtle)]">
          {insights.sampledNodes} / {insights.totalNodes} 节点 · {insights.durationMs} ms
        </span>
      </div>

      {insights.warnings.map((warning) => (
        <p
          key={warning}
          className="rounded-md border border-amber-500/25 bg-amber-500/10 px-2 py-1.5 text-[10px] leading-relaxed text-amber-700 dark:text-amber-400"
        >
          {warning}
        </p>
      ))}

      <section className="space-y-1.5" data-testid="graph-neighbors">
        <h2 className="flex items-center gap-1.5 text-xs font-medium text-[var(--ink)]">
          <Network className="h-3.5 w-3.5" />
          语义近邻
        </h2>
        {!focusTitle ? (
          <p className="text-[11px] leading-relaxed text-[var(--ink-subtle)]">
            从图中选中一篇笔记，查看最相关的 10 篇及连接原因。
          </p>
        ) : insights.neighbors.length === 0 ? (
          <p className="text-[11px] text-[var(--ink-subtle)]">
            《{focusTitle}》在当前抽样范围内没有可解释近邻。
          </p>
        ) : (
          <ul className="space-y-1">
            {insights.neighbors.map((neighbor) => (
              <li
                key={neighbor.noteId}
                className="rounded-md border border-[var(--border-subtle)] p-2"
              >
                <div className="flex items-center gap-1.5">
                  <button
                    type="button"
                    className="min-w-0 flex-1 truncate text-left text-[11px] font-medium text-[var(--ink)] hover:text-[var(--brand)]"
                    title={neighbor.title}
                    onClick={() => onFocus(neighbor.noteId)}
                  >
                    {neighbor.title}
                  </button>
                  <Score value={neighbor.score} />
                </div>
                <ul className="mt-1 space-y-0.5">
                  {neighbor.evidence.map((evidence) => (
                    <li
                      key={`${evidence.kind}-${evidence.label}`}
                      className="text-[10px] leading-relaxed text-[var(--ink-subtle)]"
                    >
                      {evidence.label}
                    </li>
                  ))}
                </ul>
                <div className="mt-1 flex justify-end gap-1">
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-6 px-1.5 text-[10px]"
                    onClick={() => onOpen(neighbor.noteId)}
                  >
                    打开 <ArrowUpRight className="h-3 w-3" />
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="space-y-1.5 border-t border-[var(--border-subtle)] pt-3">
        <h2 className="flex items-center gap-1.5 text-xs font-medium text-[var(--ink)]">
          <Copy className="h-3.5 w-3.5" />
          近重复
        </h2>
        {insights.duplicateClusters.length === 0 ? (
          <p className="text-[11px] text-[var(--ink-subtle)]">当前抽样范围内没有明显重复。</p>
        ) : (
          <ul className="space-y-1">
            {insights.duplicateClusters.map((cluster) => (
              <li key={cluster.id}>
                <button
                  type="button"
                  className="w-full rounded-md border border-[var(--border-subtle)] px-2 py-1.5 text-left hover:bg-[var(--surface-muted)]"
                  onClick={() => setDuplicatePreview(cluster)}
                >
                  <span className="flex items-center justify-between gap-2">
                    <span className="truncate text-[11px] text-[var(--ink)]">
                      {cluster.members.map((member) => member.title).join(' / ')}
                    </span>
                    <Score value={cluster.similarity} />
                  </span>
                  <span className="mt-0.5 block text-[10px] text-[var(--ink-subtle)]">
                    {cluster.members.length} 篇 · 只读预览，不自动合并
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="space-y-1.5 border-t border-[var(--border-subtle)] pt-3">
        <h2 className="flex items-center gap-1.5 text-xs font-medium text-[var(--ink)]">
          <GitBranch className="h-3.5 w-3.5" />
          结构与孤岛
        </h2>
        <div className="grid grid-cols-3 gap-1 text-center">
          {[
            ['孤儿', insights.orphanNotes.length],
            ['桥梁', insights.bridgeNotes.length],
            ['孤岛分类', insights.islandCategories.length],
          ].map(([label, count]) => (
            <div key={String(label)} className="rounded-md bg-[var(--surface-muted)] px-1 py-1.5">
              <p className="text-sm font-medium tabular-nums text-[var(--ink)]">{count}</p>
              <p className="text-[9px] text-[var(--ink-subtle)]">{label}</p>
            </div>
          ))}
        </div>
        {insights.bridgeNotes.slice(0, 3).map((bridge) => (
          <button
            key={bridge.noteId}
            type="button"
            className="block w-full truncate text-left text-[10px] text-[var(--ink-subtle)] hover:text-[var(--brand)]"
            onClick={() => onFocus(bridge.noteId)}
            title={bridge.explanation}
          >
            桥梁 · {bridge.title}：{bridge.explanation}
          </button>
        ))}
        {insights.islandCategories.slice(0, 2).map((island) => (
          <p key={island.categoryId} className="text-[10px] text-[var(--ink-subtle)]">
            孤岛 · {island.categoryPath}：{island.explanation}
          </p>
        ))}
      </section>

      <Dialog
        open={duplicatePreview !== null}
        onOpenChange={(open) => (!open ? setDuplicatePreview(null) : undefined)}
      >
        <DialogContent className="w-[min(760px,calc(100vw-2rem))]" data-testid="duplicate-preview">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Copy className="h-4 w-4" /> 近重复只读预览
            </DialogTitle>
            <DialogDescription>
              {duplicatePreview?.reason}
              当前版本不会自动改写任何笔记；请分别打开核对后再决定如何整理。
            </DialogDescription>
          </DialogHeader>
          <div className="grid max-h-[56vh] gap-2 overflow-y-auto md:grid-cols-2">
            {duplicatePreview?.members.map((member) => (
              <article
                key={member.noteId}
                className="rounded-lg border border-[var(--border-subtle)] p-3"
              >
                <h3 className="text-xs font-medium text-[var(--ink)]">{member.title}</h3>
                <p className="mt-2 whitespace-pre-wrap text-[11px] leading-relaxed text-[var(--ink-muted)]">
                  {member.excerpt || '（空正文）'}
                </p>
                <Button
                  size="sm"
                  variant="secondary"
                  className="mt-3"
                  onClick={() => onOpen(member.noteId)}
                >
                  打开笔记
                </Button>
              </article>
            ))}
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setDuplicatePreview(null)}>
              关闭
            </Button>
            {duplicatePreview ? (
              <Button
                variant="secondary"
                disabled={summaryBusy}
                onClick={() => void createClusterSummary(duplicatePreview)}
              >
                {summaryBusy ? '生成中…' : '生成引用摘要'}
              </Button>
            ) : null}
            {summaryDraft ? (
              <Button
                disabled={summaryBusy}
                onClick={() => duplicatePreview && void saveClusterSummary(duplicatePreview)}
              >
                确认新建摘要笔记
              </Button>
            ) : null}
          </DialogFooter>
          {summaryDraft ? (
            <div className="mt-3 space-y-1 rounded-md border border-[var(--border-subtle)] p-2">
              <p className="text-[10px] font-medium text-[var(--ink)]">摘要预览（写入前可取消）</p>
              <p className="whitespace-pre-wrap text-[11px] leading-relaxed text-[var(--ink-muted)]">
                {summaryDraft.text}
              </p>
              {summaryDraft.warnings.map((warning) => (
                <p key={warning} className="text-[10px] text-amber-600">
                  {warning}
                </p>
              ))}
            </div>
          ) : null}
        </DialogContent>
      </Dialog>
    </div>
  );
}
