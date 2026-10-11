import { RefreshCw, Search as SearchIcon, SlidersHorizontal, X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { toast } from 'sonner';

import { SearchResultItem } from '@/components/search/SearchResultItem';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { reportAction } from '@/lib/actionFeedback';
import { formatRelative } from '@/lib/format';
import {
  describeSearch,
  isMeaningfulSearch,
  loadRecent,
  loadSaved,
  newSavedId,
  pushRecent,
  saveRecent,
  type RecentSearch,
  saveSaved,
  type SavedSearch,
  type SearchCondition,
} from '@/lib/searchHistory';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useSearchStore } from '@/stores/searchStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import { useTagsStore } from '@/stores/tagsStore';

/** 搜索页面（设计文档 16.6）。 */
export function SearchPage() {
  const [params, setParams] = useSearchParams();
  const keyword = useSearchStore((state) => state.keyword);
  const response = useSearchStore((state) => state.response);
  const status = useSearchStore((state) => state.status);
  const loading = useSearchStore((state) => state.loading);
  const error = useSearchStore((state) => state.error);
  const categoryId = useSearchStore((state) => state.categoryId);
  const tagId = useSearchStore((state) => state.tagId);
  const favoriteOnly = useSearchStore((state) => state.favoriteOnly);
  const sort = useSearchStore((state) => state.sort);
  const page = useSearchStore((state) => state.page);
  const pageSize = useSearchStore((state) => state.pageSize);
  const setKeyword = useSearchStore((state) => state.setKeyword);
  const setFilters = useSearchStore((state) => state.setFilters);
  const setPage = useSearchStore((state) => state.setPage);
  const resetFilters = useSearchStore((state) => state.resetFilters);
  const run = useSearchStore((state) => state.run);
  const refreshStatus = useSearchStore((state) => state.refreshStatus);
  const rebuildIndex = useSearchStore((state) => state.rebuildIndex);
  const categories = useCategoriesStore((state) => state.flat);
  const pathOf = useCategoriesStore((state) => state.pathOf);
  const tags = useTagsStore((state) => state.tags);
  const [showFilters, setShowFilters] = useState(false);
  const workspaceId = useWorkspaceStore((state) => state.workspace?.id ?? 'default');
  const [recent, setRecent] = useState<RecentSearch[]>(() => loadRecent(workspaceId));
  const [saved, setSaved] = useState<SavedSearch[]>(() => loadSaved(workspaceId));
  const [saveDialogOpen, setSaveDialogOpen] = useState(false);

  const condition: SearchCondition = { keyword, categoryId, tagId, favoriteOnly };

  // 换库时读取该库的历史与收藏（异步设置，避免同步 setState 引发级联渲染）
  useEffect(() => {
    let cancelled = false;
    void Promise.resolve().then(() => {
      if (cancelled) return;
      setRecent(loadRecent(workspaceId));
      setSaved(loadSaved(workspaceId));
    });
    return () => {
      cancelled = true;
    };
  }, [workspaceId]);

  // 记录历史：有意义的条件变化后写入（去重、最多 8 条）
  useEffect(() => {
    const timer = window.setTimeout(() => {
      const current: SearchCondition = {
        keyword: useSearchStore.getState().keyword,
        categoryId: useSearchStore.getState().categoryId,
        tagId: useSearchStore.getState().tagId,
        favoriteOnly: useSearchStore.getState().favoriteOnly,
      };
      if (!isMeaningfulSearch(current)) return;
      setRecent((previous) => {
        const next = pushRecent(previous, current, new Date().toISOString());
        if (next === previous) return previous;
        saveRecent(workspaceId, next);
        return next;
      });
    }, 900);
    return () => window.clearTimeout(timer);
  }, [keyword, categoryId, tagId, favoriteOnly, workspaceId]);

  const applyCondition = (next: SearchCondition) => {
    setKeyword(next.keyword);
    setFilters({
      categoryId: next.categoryId,
      tagId: next.tagId,
      favoriteOnly: next.favoriteOnly,
    });
    setPage(1);
  };

  const removeSaved = (id: string) => {
    setSaved((previous) => {
      const next = previous.filter((item) => item.id !== id);
      saveSaved(workspaceId, next);
      return next;
    });
  };

  const initial = params.get('q') ?? '';
  const debouncedKeyword = useDebouncedValue(keyword, 220);

  useEffect(() => {
    if (initial && useSearchStore.getState().keyword !== initial) {
      setKeyword(initial);
    }
    void refreshStatus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initial]);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      void run();
    }, 0);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [debouncedKeyword, categoryId, tagId, favoriteOnly, sort, page]);

  useEffect(() => {
    if (keyword) setParams({ q: keyword }, { replace: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [keyword]);

  const totalPages = response ? Math.max(1, Math.ceil(response.total / pageSize)) : 1;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="space-y-3 border-b border-[var(--border-subtle)] p-4">
        <div className="flex items-center gap-2">
          <div className="relative flex-1">
            <SearchIcon className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[var(--ink-subtle)]" />
            <Input
              aria-label="搜索关键词"
              autoFocus
              value={keyword}
              placeholder="搜索标题、正文、标签（支持中文短语与英文前缀）"
              className="pl-9"
              onChange={(event) => setKeyword(event.target.value)}
            />
          </div>
          <Button
            variant={showFilters ? 'default' : 'secondary'}
            onClick={() => setShowFilters((value) => !value)}
          >
            <SlidersHorizontal className="h-4 w-4" />
            筛选
          </Button>
        </div>

        {recent.length > 0 || saved.length > 0 || isMeaningfulSearch(condition) ? (
          <div className="space-y-1.5">
            {recent.length > 0 ? (
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-[11px] text-[var(--ink-subtle)]">最近搜索</span>
                {recent.slice(0, 6).map((item) => (
                  <button
                    key={`${item.condition.keyword}-${item.condition.categoryId}-${item.condition.tagId}-${item.condition.favoriteOnly}-${item.at}`}
                    type="button"
                    data-testid="recent-search"
                    className="max-w-[240px] truncate rounded-full border border-[var(--border-subtle)] px-2 py-0.5 text-[11px] text-[var(--ink-muted)] transition-colors hover:border-[var(--brand)] hover:text-[var(--brand)]"
                    title={describeSearch(item.condition, {
                      categoryName: categories.find((c) => c.id === item.condition.categoryId)?.name,
                      tagName: tags.find((t) => t.id === item.condition.tagId)?.name,
                    })}
                    onClick={() => applyCondition(item.condition)}
                  >
                    {describeSearch(item.condition, {
                      categoryName: categories.find((c) => c.id === item.condition.categoryId)?.name,
                      tagName: tags.find((t) => t.id === item.condition.tagId)?.name,
                    })}
                  </button>
                ))}
              </div>
            ) : null}

            {saved.length > 0 ? (
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-[11px] text-[var(--ink-subtle)]">收藏的搜索</span>
                {saved.map((item) => (
                  <span
                    key={item.id}
                    className="flex items-center gap-1 rounded-full bg-[var(--brand-soft)] px-2 py-0.5 text-[11px] text-[var(--brand)]"
                  >
                    <button
                      type="button"
                      data-testid="saved-search"
                      className="max-w-[220px] truncate"
                      title={describeSearch(item, {
                        categoryName: categories.find((c) => c.id === item.categoryId)?.name,
                        tagName: tags.find((t) => t.id === item.tagId)?.name,
                      })}
                      onClick={() => applyCondition(item)}
                    >
                      {item.name}
                    </button>
                    <button
                      type="button"
                      aria-label={`删除收藏的搜索：${item.name}`}
                      className="opacity-70 hover:opacity-100"
                      onClick={() => removeSaved(item.id)}
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </span>
                ))}
              </div>
            ) : null}

            {isMeaningfulSearch(condition) ? (
              <button
                type="button"
                data-testid="save-search"
                className="text-[11px] text-[var(--brand)] underline decoration-dotted"
                onClick={() => setSaveDialogOpen(true)}
              >
                保存当前搜索条件
              </button>
            ) : null}
          </div>
        ) : null}

        {(recent.length > 0 || saved.length > 0 || isMeaningfulSearch(condition)) ? (
          <div className="space-y-1.5">
            {recent.length > 0 ? (
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-[11px] text-[var(--ink-subtle)]">最近搜索</span>
                {recent.slice(0, 6).map((item) => {
                  const label = describeSearch(item.condition, {
                    categoryName: categories.find((c) => c.id === item.condition.categoryId)?.name,
                    tagName: tags.find((t) => t.id === item.condition.tagId)?.name,
                  });
                  return (
                    <button
                      key={`${item.at}-${item.condition.keyword}-${item.condition.categoryId}-${item.condition.tagId}-${item.condition.favoriteOnly}`}
                      type="button"
                      data-testid="recent-search"
                      className="max-w-[240px] truncate rounded-full border border-[var(--border-subtle)] px-2 py-0.5 text-[11px] text-[var(--ink-muted)] transition-colors hover:border-[var(--brand)] hover:text-[var(--brand)]"
                      title={label}
                      onClick={() => applyCondition(item.condition)}
                    >
                      {label}
                    </button>
                  );
                })}
              </div>
            ) : null}

            {saved.length > 0 ? (
              <div className="flex flex-wrap items-center gap-1.5">
                <span className="text-[11px] text-[var(--ink-subtle)]">收藏的搜索</span>
                {saved.map((item) => (
                  <span
                    key={item.id}
                    className="flex items-center gap-1 rounded-full bg-[var(--brand-soft)] px-2 py-0.5 text-[11px] text-[var(--brand)]"
                  >
                    <button
                      type="button"
                      data-testid="saved-search"
                      className="max-w-[220px] truncate"
                      title={describeSearch(item, {
                        categoryName: categories.find((c) => c.id === item.categoryId)?.name,
                        tagName: tags.find((t) => t.id === item.tagId)?.name,
                      })}
                      onClick={() => applyCondition(item)}
                    >
                      {item.name}
                    </button>
                    <button
                      type="button"
                      aria-label={`删除收藏的搜索：${item.name}`}
                      className="opacity-70 hover:opacity-100"
                      onClick={() => removeSaved(item.id)}
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </span>
                ))}
              </div>
            ) : null}

            {isMeaningfulSearch(condition) ? (
              <button
                type="button"
                data-testid="save-search"
                className="text-[11px] text-[var(--brand)] underline decoration-dotted"
                onClick={() => setSaveDialogOpen(true)}
              >
                保存当前搜索条件
              </button>
            ) : null}
          </div>
        ) : null}

        {showFilters ? (
          <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
            <select
              aria-label="按分类筛选"
              value={categoryId ?? ''}
              onChange={(event) => setFilters({ categoryId: event.target.value || null })}
              className="h-9 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm"
            >
              <option value="">全部分类</option>
              {categories.map((category) => (
                <option key={category.id} value={category.id}>
                  {pathOf(category.id)}
                </option>
              ))}
            </select>
            <select
              aria-label="按标签筛选"
              value={tagId ?? ''}
              onChange={(event) => setFilters({ tagId: event.target.value || null })}
              className="h-9 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm"
            >
              <option value="">全部标签</option>
              {tags.map((tag) => (
                <option key={tag.id} value={tag.id}>
                  {tag.name}（{tag.noteCount}）
                </option>
              ))}
            </select>
            <select
              aria-label="排序方式"
              value={sort}
              onChange={(event) =>
                setFilters({ sort: event.target.value as typeof sort })
              }
              className="h-9 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm"
            >
              <option value="relevance">相关度优先</option>
              <option value="updatedDesc">最近修改</option>
              <option value="createdDesc">创建时间</option>
              <option value="titleAsc">标题</option>
            </select>
            <div className="flex items-center gap-2">
              <label className="flex items-center gap-2 text-xs text-[var(--ink-muted)]">
                <input
                  type="checkbox"
                  checked={favoriteOnly}
                  onChange={(event) => setFilters({ favoriteOnly: event.target.checked })}
                />
                仅收藏
              </label>
              <Button size="sm" variant="ghost" onClick={resetFilters}>
                重置
              </Button>
            </div>
          </div>
        ) : null}

        <div className="flex flex-wrap items-center gap-3 text-[11px] text-[var(--ink-subtle)]">
          <span>
            {response ? `共 ${response.total} 条结果，用时 ${response.tookMs} ms` : '输入关键词开始搜索'}
          </span>
          <span>索引：{status?.state ?? 'ready'}</span>
          {response?.indexStale ? (
            <span className="text-amber-600">索引可能落后于数据，建议重建</span>
          ) : null}
          <Button
            size="sm"
            variant="ghost"
            className="ml-auto"
            onClick={() =>
              void rebuildIndex().then((ok) =>
                reportAction(ok, '索引已重建', useSearchStore.getState().error, '重建索引失败'),
              )
            }
          >
            <RefreshCw className="h-3.5 w-3.5" />
            重建索引
          </Button>
        </div>
        {error ? <p className="text-xs text-red-500">{error}</p> : null}
      </div>

      <div className="min-h-0 flex-1 overflow-auto p-3">
        {loading && !response ? (
          <p className="p-4 text-xs text-[var(--ink-subtle)]">正在搜索…</p>
        ) : !response || response.hits.length === 0 ? (
          <EmptyState
            icon={SearchIcon}
            title={keyword ? '没有找到匹配的笔记' : '开始搜索你的知识库'}
            description={
              keyword
                ? '可以尝试更短的关键词、去掉筛选条件，或重建索引后重试。'
                : '支持中文连续词、英文与数字混合；结果按标题命中与相关度排序。'
            }
            className="h-full"
          />
        ) : (
          <ul className="space-y-1.5">
            {response.hits.map((hit) => (
              <li key={hit.note.id}>
                <SearchResultItem hit={hit} keyword={keyword} />
              </li>
            ))}
          </ul>
        )}
      </div>

      {response && response.total > pageSize ? (
        <div className="flex items-center justify-between border-t border-[var(--border-subtle)] px-4 py-2 text-xs text-[var(--ink-muted)]">
          <span>
            第 {page} / {totalPages} 页
          </span>
          <span className="flex gap-2">
            <Button size="sm" variant="secondary" disabled={page <= 1} onClick={() => setPage(page - 1)}>
              上一页
            </Button>
            <Button
              size="sm"
              variant="secondary"
              disabled={page >= totalPages}
              onClick={() => setPage(page + 1)}
            >
              下一页
            </Button>
          </span>
          <span>最近更新：{response.hits[0] ? formatRelative(response.hits[0].note.updatedAt) : '—'}</span>
        </div>
      ) : null}

      {saveDialogOpen ? (
        <SaveSearchDialog
          condition={condition}
          onClose={() => setSaveDialogOpen(false)}
          onSave={(name) => {
            const item: SavedSearch = {
              ...condition,
              id: newSavedId(),
              name,
              createdAt: new Date().toISOString(),
            };
            setSaved((previous) => {
              const next = [item, ...previous];
              saveSaved(workspaceId, next);
              return next;
            });
            setSaveDialogOpen(false);
            toast.success(`已收藏「${name}」`);
          }}
        />
      ) : null}
    </div>
  );
}

/** 收藏搜索条件：一个名字即可。 */
function SaveSearchDialog({
  condition,
  onClose,
  onSave,
}: {
  condition: SearchCondition;
  onClose: () => void;
  onSave: (name: string) => void;
}) {
  const [name, setName] = useState('');
  const fallback = condition.keyword.trim() || '未命名搜索';
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4">
      <div className="w-[min(420px,100%)] space-y-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-raised)] p-4">
        <h2 className="text-sm font-semibold text-[var(--ink)]">收藏当前搜索条件</h2>
        <p className="text-[11px] text-[var(--ink-subtle)]">{describeSearch(condition, {})}</p>
        <input
          autoFocus
          value={name}
          aria-label="搜索名称"
          placeholder={fallback}
          className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)]"
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') onSave(name.trim() || fallback);
            if (event.key === 'Escape') onClose();
          }}
        />
        <div className="flex justify-end gap-2">
          <Button variant="secondary" onClick={onClose}>
            取消
          </Button>
          <Button onClick={() => onSave(name.trim() || fallback)}>收藏</Button>
        </div>
      </div>
    </div>
  );
}
