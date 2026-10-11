import {
  BarChart3,
  CalendarClock,
  CalendarDays,
  Clock,
  FileText,
  FolderTree,
  Inbox,
  Pin,
  Plus,
  RefreshCw,
  Search,
  Tag,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Skeleton } from '@/components/ui/badge';
import { UnresolvedLinksCard } from '@/components/notes/UnresolvedLinksCard';
import { ReviewDueCard } from '@/features/dashboard/ReviewDueCard';
import { reportAction } from '@/lib/actionFeedback';
import { formatRelative } from '@/lib/format';
import { ROUTES } from '@/lib/constants';
import { cn } from '@/lib/utils';
import { noteService } from '@/services/noteService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { NoteSummary, ReviewDueNote } from '@/types';

/** 首页（设计文档 16.3）。 */
export function DashboardPage() {
  const navigate = useNavigate();
  const workspace = useWorkspaceStore((state) => state.workspace);
  const stats = useWorkspaceStore((state) => state.stats);
  const indexStatus = useWorkspaceStore((state) => state.indexStatus);
  const rebuildIndex = useWorkspaceStore((state) => state.rebuildIndex);
  const setNewNoteDialogOpen = useUiStore((state) => state.setNewNoteDialogOpen);
  const setCommandPaletteOpen = useUiStore((state) => state.setCommandPaletteOpen);
  const categories = useCategoriesStore((state) => state.flat);
  const pathOf = useCategoriesStore((state) => state.pathOf);
  const [recent, setRecent] = useState<NoteSummary[]>([]);
  const [dueForReview, setDueForReview] = useState<ReviewDueNote[]>([]);
  const [loading, setLoading] = useState(true);
  const [keyword, setKeyword] = useState('');

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      setLoading(true);
      await useNotesStore.getState().refresh();
      const result = await noteService.recent(6);
      if (!cancelled) {
        setRecent(result);
        setLoading(false);
      }
      // 待复核（D4）：失败不影响首页其它区块
      const due = await noteService.dueForReview(14).catch(() => [] as ReviewDueNote[]);
      if (!cancelled) setDueForReview(due);
    };
    void load();
    return () => {
      cancelled = true;
    };
  }, [workspace?.id]);

  return (
    <div className="h-full w-full overflow-auto">
      <div className="mx-auto w-full max-w-6xl space-y-6 p-6">
        <header className="space-y-3">
          <h1 className="text-xl font-semibold text-[var(--ink)]">{workspace?.name ?? '知识库'}</h1>
          <p className="text-xs text-[var(--ink-subtle)]">{workspace?.rootPath}</p>
          <div className="flex flex-wrap gap-2">
            <div className="relative min-w-[220px] flex-1">
              <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--ink-subtle)]" />
              <Input
                value={keyword}
                placeholder="快速搜索笔记…（⌘K 打开全局搜索）"
                className="pl-8"
                onChange={(event) => setKeyword(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    navigate(`${ROUTES.search}?q=${encodeURIComponent(keyword)}`);
                  }
                }}
              />
            </div>
            <Button variant="secondary" onClick={() => setCommandPaletteOpen(true)}>
              <Search className="h-4 w-4" />
              全局搜索
            </Button>
            <Button onClick={() => setNewNoteDialogOpen(true)}>
              <Plus className="h-4 w-4" />
              新建笔记
            </Button>
          </div>
        </header>

        <section className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <StatCard
            icon={Inbox}
            label="Inbox 待整理"
            value={stats.inboxCount}
            onClick={() => navigate(ROUTES.inbox)}
          />
          <StatCard
            icon={FolderTree}
            label="分类"
            value={stats.categoryCount}
            onClick={() => navigate(ROUTES.categories)}
          />
          <StatCard
            icon={Tag}
            label="标签"
            value={stats.tagCount}
            onClick={() => navigate(ROUTES.tags)}
          />
          <StatCard
            icon={Clock}
            label="全部笔记"
            value={stats.noteCount}
            onClick={() => navigate(ROUTES.notes)}
          />
        </section>

        {/* 统计明细（对标语雀「统计」页）：字数、近期更新、置顶 —— 全部来自本地数据库 */}
        <section
          data-testid="workspace-stats"
          className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4"
        >
          <div className="mb-3 flex items-center justify-between">
            <h2 className="flex items-center gap-1.5 text-sm font-semibold text-[var(--ink)]">
              <BarChart3 className="h-3.5 w-3.5 text-[var(--brand)]" />
              统计
            </h2>
            <span className="text-[11px] text-[var(--ink-subtle)]">数据只来自本地工作区</span>
          </div>
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <StatCard
              icon={FileText}
              label="正文字数"
              value={stats.totalWords}
              onClick={() => navigate(ROUTES.notes)}
            />
            <StatCard
              icon={CalendarClock}
              label="近 7 天更新"
              value={stats.updatedLast7d}
              onClick={() => navigate(ROUTES.notes)}
            />
            <StatCard
              icon={CalendarDays}
              label="近 30 天更新"
              value={stats.updatedLast30d}
              onClick={() => navigate(ROUTES.notes)}
            />
            <StatCard
              icon={Pin}
              label="置顶笔记"
              value={stats.pinnedCount}
              onClick={() => navigate(ROUTES.notes)}
            />
          </div>
          <p className="mt-3 text-[11px] text-[var(--ink-subtle)]">
            「近 7 / 30 天更新」按笔记的最后修改时间统计；置顶笔记在所有排序下都排在最前。
          </p>
          {/* 写作活动：近 14 天每天有更新的笔记数（对标语雀统计页的写作活动） */}
          <div className="mt-4" data-testid="writing-activity">
            <div className="mb-1.5 flex items-center justify-between">
              <h3 className="text-xs font-medium text-[var(--ink-muted)]">近 14 天写作活动</h3>
              <span className="text-[11px] text-[var(--ink-subtle)]">
                共 {stats.activity.reduce((sum, day) => sum + day.count, 0)} 次更新
              </span>
            </div>
            <div className="flex h-14 items-end gap-1">
              {stats.activity.length === 0 ? (
                <span className="text-[11px] text-[var(--ink-subtle)]">还没有更新记录</span>
              ) : (
                stats.activity.map((day) => {
                  const max = Math.max(1, ...stats.activity.map((item) => item.count));
                  const height = day.count === 0 ? 4 : Math.max(8, Math.round((day.count / max) * 48));
                  return (
                    <div
                      key={day.date}
                      title={`${day.date}：${day.count} 篇更新`}
                      className={cn(
                        'flex-1 rounded-sm transition-colors',
                        day.count === 0 ? 'bg-[var(--surface-muted)]' : 'bg-[var(--brand)]/70',
                      )}
                      style={{ height }}
                    />
                  );
                })
              )}
            </div>
            <div className="mt-1 flex justify-between text-[10px] text-[var(--ink-subtle)]">
              <span>{stats.activity[0]?.date ?? ''}</span>
              <span>{stats.activity.at(-1)?.date ?? ''}</span>
            </div>
          </div>
        </section>

        {/* 待复核（D4）：只在有时效到期的笔记时出现 */}
        <ReviewDueCard items={dueForReview} onOpen={(id) => navigate(ROUTES.note(id))} />

        <section className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-sm font-semibold text-[var(--ink)]">最近编辑</h2>
            <Button size="sm" variant="ghost" onClick={() => navigate(ROUTES.notes)}>
              查看全部
            </Button>
          </div>
          {loading ? (
            <div className="space-y-2">
              <Skeleton className="h-10 w-full" />
              <Skeleton className="h-10 w-full" />
            </div>
          ) : recent.length === 0 ? (
            <p className="text-xs text-[var(--ink-subtle)]">
              还没有笔记，使用「新建笔记」开始记录第一条知识。
            </p>
          ) : (
            <ul className="divide-y divide-[var(--border-subtle)]">
              {recent.map((note) => (
                <li key={note.id}>
                  <button
                    type="button"
                    className="flex w-full items-center gap-3 py-2 text-left transition-colors hover:text-[var(--brand)]"
                    onClick={() => navigate(`/notes/${note.id}`)}
                  >
                    <span className="min-w-0 flex-1 truncate text-sm text-[var(--ink)]">
                      {note.title}
                    </span>
                    <span className="hidden text-[11px] text-[var(--ink-subtle)] sm:block">
                      {note.categoryPath || 'Inbox'}
                    </span>
                    <span className="text-[11px] text-[var(--ink-subtle)]">
                      {formatRelative(note.updatedAt)}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="grid items-start gap-3 lg:grid-cols-[2fr_1fr]">
          <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4">
            <h2 className="mb-2 text-sm font-semibold text-[var(--ink)]">常用分类</h2>
            {categories.length === 0 ? (
              <p className="text-xs text-[var(--ink-subtle)]">暂无分类，可在侧边栏创建。</p>
            ) : (
              <ul className="space-y-1">
                {categories.slice(0, 6).map((category) => (
                  <li key={category.id}>
                    <button
                      type="button"
                      className="flex w-full items-center justify-between rounded-md px-2 py-1.5 text-sm text-[var(--ink-muted)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]"
                      onClick={() => navigate(`/notes?categoryId=${category.id}`)}
                    >
                      <span className="truncate">{pathOf(category.id)}</span>
                      <span className="text-[11px] text-[var(--ink-subtle)]">{category.noteCount}</span>
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="space-y-3">
            <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4">
              <h2 className="mb-2 text-sm font-semibold text-[var(--ink)]">索引状态</h2>
              <dl className="space-y-1.5 text-xs text-[var(--ink-muted)]">
                <div className="flex justify-between">
                  <dt>状态</dt>
                  <dd>{indexStatus?.state ?? 'ready'}</dd>
                </div>
                <div className="flex justify-between">
                  <dt>已索引 / 全部</dt>
                  <dd>
                    {indexStatus?.indexedNotes ?? 0} / {indexStatus?.totalNotes ?? 0}
                  </dd>
                </div>
                <div className="flex justify-between">
                  <dt>最近重建</dt>
                  <dd>
                    {indexStatus?.lastRebuiltAt ? formatRelative(indexStatus.lastRebuiltAt) : '—'}
                  </dd>
                </div>
              </dl>
              <Button
                size="sm"
                variant="secondary"
                className="mt-3"
                onClick={() =>
                  void rebuildIndex().then((ok) =>
                    reportAction(ok, '索引已重建', useWorkspaceStore.getState().error, '重建索引失败'),
                  )
                }
              >
                <RefreshCw className="h-3.5 w-3.5" />
                重建索引
              </Button>
            </div>

            <UnresolvedLinksCard />
          </div>
        </section>
      </div>
    </div>
  );
}

function StatCard({
  icon: Icon,
  label,
  value,
  onClick,
}: {
  icon: typeof Inbox;
  label: string;
  value: number;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center gap-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4 text-left transition-colors hover:border-[var(--brand)]"
    >
      <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-[var(--brand-soft)] text-[var(--brand)]">
        <Icon className="h-4 w-4" />
      </span>
      <span>
        <span className="block text-lg font-semibold text-[var(--ink)]">{value}</span>
        <span className="block text-[11px] text-[var(--ink-subtle)]">{label}</span>
      </span>
    </button>
  );
}
