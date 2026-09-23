import { CornerDownLeft, Loader2, Search } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { Dialog, DialogContent } from '@/components/ui/dialog';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { useDebouncedValue } from '@/hooks/useDebouncedValue';
import { ROUTES } from '@/lib/constants';
import { searchService } from '@/services/searchService';
import { useUiStore } from '@/stores/uiStore';
import type { NoteSummary } from '@/types';

/** 全局搜索面板（设计文档 FR-S01，快捷键 ⌘/Ctrl + K）。 */
export function GlobalSearchDialog() {
  const open = useUiStore((state) => state.commandPaletteOpen);
  const setOpen = useUiStore((state) => state.setCommandPaletteOpen);
  // 面板在打开时挂载，关闭即销毁，因此无需在 effect 中重置状态
  if (!open) return null;
  return <SearchPanel onClose={() => setOpen(false)} />;
}

function SearchPanel({ onClose }: { onClose: () => void }) {
  const navigate = useNavigate();
  const [keyword, setKeyword] = useState('');
  const debounced = useDebouncedValue(keyword, 180);
  const [results, setResults] = useState<NoteSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      setLoading(true);
      try {
        const items = await searchService.quick(debounced, 8);
        if (!cancelled) {
          setResults(items);
          setActiveIndex(0);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    };
    void run();
    return () => {
      cancelled = true;
    };
  }, [debounced]);

  const hint = useMemo(() => (debounced.trim() ? '输入以搜索标题与正文' : '输入关键词开始搜索'), [debounced]);

  const openSearchPage = () => {
    onClose();
    navigate(`${ROUTES.search}?q=${encodeURIComponent(keyword)}`);
  };

  return (
    <Dialog open onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent className="w-[min(680px,calc(100vw-2rem))] p-0">
        <div className="flex items-center gap-2 border-b border-[var(--border-subtle)] px-3 py-2.5">
          <Search className="h-4 w-4 text-[var(--ink-subtle)]" />
          <Input
            autoFocus
            value={keyword}
            placeholder={hint}
            className="h-8 border-none px-0 text-sm focus-visible:ring-0"
            onChange={(event) => setKeyword(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                setActiveIndex((index) => Math.min(index + 1, Math.max(results.length - 1, 0)));
              } else if (event.key === 'ArrowUp') {
                event.preventDefault();
                setActiveIndex((index) => Math.max(index - 1, 0));
              } else if (event.key === 'Enter') {
                event.preventDefault();
                const target = results[activeIndex];
                if (target) {
                  onClose();
                  navigate(`/notes/${target.id}`);
                } else {
                  openSearchPage();
                }
              }
            }}
          />
          {loading ? <Loader2 className="h-3.5 w-3.5 animate-spin text-[var(--ink-subtle)]" /> : null}
        </div>

        <div className="max-h-[60vh] min-h-[160px] overflow-auto p-2">
          {results.length === 0 ? (
            <EmptyState
              icon={Search}
              title={debounced.trim() ? '没有找到匹配的笔记' : '搜索你的全部笔记'}
              description={
                debounced.trim()
                  ? '尝试更换关键词，或使用搜索页面按分类、标签、时间过滤。'
                  : '支持中文短语、英文与数字混合查询，结果会显示匹配摘要与高亮。'
              }
            />
          ) : (
            <ul className="space-y-1">
              {results.map((note, index) => (
                <li key={note.id}>
                  <button
                    type="button"
                    onMouseEnter={() => setActiveIndex(index)}
                    onClick={() => {
                      onClose();
                      navigate(`/notes/${note.id}`);
                    }}
                    className={[
                      'flex w-full flex-col items-start gap-0.5 rounded-md px-3 py-2 text-left transition-colors',
                      index === activeIndex ? 'bg-[var(--brand-soft)]' : 'hover:bg-[var(--surface-muted)]',
                    ].join(' ')}
                  >
                    <span className="truncate text-sm font-medium text-[var(--ink)]">{note.title}</span>
                    <span className="truncate text-[11px] text-[var(--ink-subtle)]">
                      {note.categoryPath || 'Inbox'} · {note.filePath}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="flex items-center justify-between border-t border-[var(--border-subtle)] px-3 py-2 text-[11px] text-[var(--ink-subtle)]">
          <span className="flex items-center gap-2">
            <span>↑↓ 选择</span>
            <span className="flex items-center gap-1">
              <CornerDownLeft className="h-3 w-3" /> 打开
            </span>
          </span>
          <button type="button" className="text-[var(--brand)]" onClick={openSearchPage}>
            查看全部结果
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
