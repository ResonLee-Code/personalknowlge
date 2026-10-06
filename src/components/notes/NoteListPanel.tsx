import { FileText, ListFilter, Search, Star, Trash2, TriangleAlert } from 'lucide-react';
import { useEffect } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { ROUTES } from '@/lib/constants';
import { cn } from '@/lib/utils';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useUiStore } from '@/stores/uiStore';

import { NoteListItem } from './NoteListItem';

const SORT_OPTIONS = [
  { value: 'updatedDesc', label: '最近修改' },
  { value: 'createdDesc', label: '创建时间' },
  { value: 'titleAsc', label: '标题' },
] as const;

export function NoteListPanel() {
  const location = useLocation();
  const navigate = useNavigate();
  const width = useUiStore((state) => state.listWidth);
  const items = useNotesStore((state) => state.items);
  const total = useNotesStore((state) => state.total);
  const loading = useNotesStore((state) => state.loading);
  const scope = useNotesStore((state) => state.scope);
  const sort = useNotesStore((state) => state.sort);
  const keyword = useNotesStore((state) => state.keyword);
  const setSort = useNotesStore((state) => state.setSort);
  const setKeyword = useNotesStore((state) => state.setKeyword);
  const selectedId = useNotesStore((state) => state.selectedId);
  const categories = useCategoriesStore((state) => state.flat);
  const tags = useTagsStore((state) => state.tags);

  const basePath = `/${location.pathname.split('/').filter(Boolean)[0] ?? 'notes'}`;

  useEffect(() => {
    void useNotesStore.getState().refresh();
  }, [scope.kind, location.search]);

  const title = (() => {
    switch (scope.kind) {
      case 'inbox':
        return 'Inbox';
      case 'favorites':
        return '收藏';
      case 'trash':
        return '回收站';
      case 'missing':
        return '文件缺失';
      case 'category': {
        const name = categories.find((category) => category.id === scope.id)?.name;
        return name ? `分类 · ${name}` : '分类';
      }
      case 'tag': {
        const name = tags.find((tag) => tag.id === scope.id)?.name;
        return name ? `标签 · ${name}` : '标签';
      }
      default:
        return '全部笔记';
    }
  })();

  const emptyState = (() => {
    switch (scope.kind) {
      case 'trash':
        return {
          icon: Trash2,
          title: '回收站是空的',
          description: '删除的笔记会先移到回收站，可以随时恢复。',
        };
      case 'favorites':
        return {
          icon: Star,
          title: '还没有收藏的笔记',
          description: '在笔记工具栏点击星标即可收藏常用内容。',
        };
      case 'missing':
        return {
          icon: TriangleAlert,
          title: '没有缺失文件',
          description: '外部删除或移动的笔记会显示在这里。',
        };
      default:
        return {
          icon: FileText,
          title: keyword ? '没有匹配的笔记' : '这里还没有笔记',
          description: keyword
            ? '尝试更换关键词，或清空筛选条件。'
            : '使用 ⌘N 新建笔记，或点击工具栏的新建按钮。',
        };
    }
  })();

  return (
    <div className="flex min-w-0 shrink-0 flex-col border-r border-[var(--border-subtle)]" style={{ width }}>
      <div className="space-y-2 border-b border-[var(--border-subtle)] p-2.5">
        <div className="flex items-center justify-between">
          <h1 className="text-sm font-semibold text-[var(--ink)]">{title}</h1>
          <span className="text-[11px] text-[var(--ink-subtle)]">{total} 篇</span>
        </div>
        <div className="relative">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--ink-subtle)]" />
          <Input
            value={keyword}
            placeholder="按标题筛选"
            className="h-8 pl-8 text-xs"
            onChange={(event) => void setKeyword(event.target.value)}
          />
        </div>
        <div className="flex items-center gap-1">
          {SORT_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              onClick={() => void setSort(option.value)}
              className={cn(
                'rounded px-1.5 py-0.5 text-[11px] transition-colors',
                sort === option.value
                  ? 'bg-[var(--brand-soft)] text-[var(--brand)]'
                  : 'text-[var(--ink-subtle)] hover:text-[var(--ink)]',
              )}
            >
              {option.label}
            </button>
          ))}
        </div>
      </div>

      {items.length === 0 ? (
        <EmptyState
          icon={emptyState.icon}
          title={loading ? '正在加载…' : emptyState.title}
          description={emptyState.description}
          className="flex-1"
          action={
            scope.kind === 'trash' || scope.kind === 'missing' ? undefined : (
              <Button size="sm" variant="secondary" onClick={() => useUiStore.getState().setNewNoteDialogOpen(true)}>
                新建笔记
              </Button>
            )
          }
        />
      ) : (
        <ScrollArea className="flex-1">
          <ul className="space-y-0.5 p-1.5">
            {items.map((note) => (
              <NoteListItem
                key={note.id}
                note={note}
                selected={selectedId === note.id}
                basePath={basePath}
                onOpen={() => navigate(`${basePath}/${note.id}`)}
              />
            ))}
          </ul>
        </ScrollArea>
      )}

      {/* 仅在按分类/标签筛选时出现：用于回到全部笔记 */}
      {scope.kind === 'category' || scope.kind === 'tag' ? (
        <div className="border-t border-[var(--border-subtle)] px-2 py-1.5">
          <button
            type="button"
            title="清除分类 / 标签筛选，回到全部笔记"
            className="flex items-center gap-1.5 text-[11px] text-[var(--ink-muted)] transition-colors hover:text-[var(--brand)]"
            onClick={() => navigate(ROUTES.notes)}
          >
            <ListFilter className="h-3 w-3" />
            清除筛选，查看全部笔记
          </button>
        </div>
      ) : null}
    </div>
  );
}
