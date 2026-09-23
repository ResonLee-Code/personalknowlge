import { RefreshCw, Search as SearchIcon, SlidersHorizontal } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { toast } from 'sonner';

import { SearchResultItem } from '@/components/search/SearchResultItem';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { formatRelative } from '@/lib/format';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useSearchStore } from '@/stores/searchStore';
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

        {showFilters ? (
          <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
            <select
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
              void rebuildIndex().then(() => toast.success('索引已重建'))
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
    </div>
  );
}
