import { CheckSquare, FileText, FolderTree, ListFilter, Search, Star, Tag, Trash2, TriangleAlert, X } from 'lucide-react';
import { toast } from 'sonner';
import { useEffect, useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { ROUTES } from '@/lib/constants';
import { errorMessage } from '@/lib/format';
import { buildNoteTree } from '@/lib/noteTree';
import { groupByRecency } from '@/lib/timeGroup';
import { cn } from '@/lib/utils';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useUiStore } from '@/stores/uiStore';
import { TooltipProvider } from '@/components/ui/tooltip';

import { BatchMoveDialog, BatchTagDialog } from './BatchActionDialogs';
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

  const selectionMode = useNotesStore((state) => state.selectionMode);
  const selection = useNotesStore((state) => state.selection);
  const setSelectionMode = useNotesStore((state) => state.setSelectionMode);
  const setSelection = useNotesStore((state) => state.setSelection);
  const deleteMany = useNotesStore((state) => state.deleteMany);
  const [batchMoveOpen, setBatchMoveOpen] = useState(false);
  const [batchTagOpen, setBatchTagOpen] = useState(false);
  /** 折叠的父文档 id（默认全部展开） */
  const [collapsedIds, setCollapsedIds] = useState<string[]>([]);

  const collapsed = useMemo(() => new Set(collapsedIds), [collapsedIds]);
  const rows = useMemo(() => buildNoteTree(items, collapsed), [items, collapsed]);

  const toggleCollapse = (id: string) =>
    setCollapsedIds((previous) =>
      previous.includes(id) ? previous.filter((item) => item !== id) : [...previous, id],
    );

  const basePath = `/${location.pathname.split('/').filter(Boolean)[0] ?? 'notes'}`;

  const selectable = scope.kind !== 'trash' && scope.kind !== 'missing';

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
    // 列表条目在标题被截断时需要 tooltip 显示完整标题，这里提供一个局部 provider
    <TooltipProvider delayDuration={250}>
      <div
        data-testid="note-list-panel"
        className="flex min-w-0 shrink-0 flex-col border-r border-[var(--border-subtle)]"
        style={{ width }}
      >
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
        {selectionMode ? (
          <div
            data-testid="batch-toolbar"
            className="flex flex-wrap items-center gap-1.5 rounded-md bg-[var(--brand-soft)] px-2 py-1.5 text-[11px] text-[var(--brand)]"
          >
            <CheckSquare className="h-3.5 w-3.5" />
            已选 {selection.length}
            <button
              type="button"
              className="ml-1 underline decoration-dotted"
              onClick={() => setSelection(items.map((item) => item.id))}
            >
              全选
            </button>
            <button
              type="button"
              className="underline decoration-dotted"
              onClick={() => setSelection([])}
            >
              清除
            </button>
            <span className="mx-0.5 h-3 w-px bg-[var(--brand)]/30" />
            <button
              type="button"
              className="flex items-center gap-0.5 disabled:opacity-40"
              disabled={selection.length === 0}
              onClick={() => setBatchMoveOpen(true)}
            >
              <FolderTree className="h-3 w-3" />
              移动
            </button>
            <button
              type="button"
              className="flex items-center gap-0.5 disabled:opacity-40"
              disabled={selection.length === 0}
              onClick={() => setBatchTagOpen(true)}
            >
              <Tag className="h-3 w-3" />
              标签
            </button>
            <button
              type="button"
              className="flex items-center gap-0.5 text-red-600 disabled:opacity-40"
              disabled={selection.length === 0}
              onClick={() => {
                if (
                  window.confirm(`将选中的 ${selection.length} 篇笔记移入回收站？（可恢复）`)
                ) {
                  void deleteMany();
                }
              }}
            >
              <Trash2 className="h-3 w-3" />
              删除
            </button>
            <button
              type="button"
              className="ml-auto flex items-center gap-0.5"
              onClick={() => setSelectionMode(false)}
            >
              <X className="h-3 w-3" />
              退出
            </button>
          </div>
        ) : null}

        <div className="flex items-center gap-1">
          {selectable ? (
            <button
              type="button"
              title="多选（批量移动 / 打标签 / 删除）"
              className="flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[11px] text-[var(--ink-subtle)] transition-colors hover:text-[var(--brand)]"
              onClick={() => setSelectionMode(!selectionMode)}
            >
              <CheckSquare className="h-3 w-3" />
              多选
            </button>
          ) : null}
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
          {scope.kind === 'trash' ? (
            // 回收站按时间分组，便于按批次清理
            <div className="p-1.5">
              {groupByRecency(items, (note) => note.updatedAt).map((group) => (
                <section key={group.bucket} className="mb-1.5">
                  <p className="px-1.5 py-1 text-[11px] font-medium text-[var(--ink-subtle)]">
                    {group.label}
                    <span className="ml-1 text-[var(--ink-subtle)]/70">{group.items.length}</span>
                  </p>
                  <ul className="space-y-0.5">
                    {group.items.map((note) => (
                      <NoteListItem
                        key={note.id}
                        note={note}
                        selected={selectedId === note.id}
                        basePath={basePath}
                        onOpen={() => navigate(`${basePath}/${note.id}`)}
                      />
                    ))}
                  </ul>
                </section>
              ))}
            </div>
          ) : (
            <ul className="space-y-0.5 p-1.5">
              {rows.map(({ note, depth, childCount }) => (
                <NoteListItem
                  key={note.id}
                  note={note}
                  selected={selectedId === note.id}
                  basePath={basePath}
                  depth={depth}
                  childCount={childCount}
                  collapsed={collapsed.has(note.id)}
                  onToggleCollapse={() => toggleCollapse(note.id)}
                  onOpen={() => navigate(`${basePath}/${note.id}`)}
                />
              ))}
            </ul>
          )}
        </ScrollArea>
      )}

      {batchMoveOpen ? <BatchMoveDialog onClose={() => setBatchMoveOpen(false)} /> : null}
      {batchTagOpen ? <BatchTagDialog onClose={() => setBatchTagOpen(false)} /> : null}

      {/* 回收站：批量清理（永久删除，不可恢复） */}
      {scope.kind === 'trash' && items.length > 0 ? (
        <div className="border-t border-[var(--border-subtle)] px-2 py-1.5">
          <button
            type="button"
            className="flex items-center gap-1.5 text-[11px] text-[var(--ink-muted)] transition-colors hover:text-red-500"
            onClick={() => {
              if (
                !window.confirm(
                  `将永久删除回收站中的 ${items.length} 篇笔记，此操作不可恢复。是否继续？`,
                )
              ) {
                return;
              }
              void (async () => {
                const purge = useNotesStore.getState().purge;
                let removed = 0;
                for (const note of items) {
                  try {
                    await purge(note.id);
                    removed += 1;
                  } catch (error) {
                    toast.error(errorMessage(error));
                    break;
                  }
                }
                await useNotesStore.getState().refresh();
                if (removed > 0) toast.success(`已永久删除 ${removed} 篇笔记`);
              })();
            }}
          >
            <Trash2 className="h-3 w-3" />
            清空回收站（{items.length}）
          </button>
        </div>
      ) : null}

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
    </TooltipProvider>
  );
}
