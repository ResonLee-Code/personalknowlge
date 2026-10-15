import {
  CheckSquare,
  Copy,
  ChevronsDownUp,
  ChevronsUpDown,
  FileText,
  FolderTree,
  ListFilter,
  ListTree,
  Search,
  Star,
  Tag,
  Trash2,
  TriangleAlert,
  X,
} from 'lucide-react';
import { toast } from 'sonner';
import { useEffect, useMemo, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { ScrollArea } from '@/components/ui/scroll-area';
import { reportFailure } from '@/lib/actionFeedback';
import { ROUTES } from '@/lib/constants';
import { errorMessage } from '@/lib/format';
import { buildNoteTree } from '@/lib/noteTree';
import { planNoteDrop, type NoteDropPosition } from '@/lib/noteDragDrop';
import { groupByRecency } from '@/lib/timeGroup';
import { cn } from '@/lib/utils';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import { TooltipProvider } from '@/components/ui/tooltip';
import type { NoteSort } from '@/types';

import { OutlineToggleButton } from '@/components/editor/OutlineToggleButton';

import { BatchCopyDialog, BatchMoveDialog, BatchTagDialog } from './BatchActionDialogs';
import { NoteListItem } from './NoteListItem';

const SORT_OPTIONS = [
  { value: 'updatedDesc', label: '最近修改' },
  { value: 'createdDesc', label: '创建时间' },
  { value: 'titleAsc', label: '标题' },
  { value: 'manual', label: '自定义顺序' },
] as const;

export function NoteListPanel({ hasNote = false }: { hasNote?: boolean } = {}) {
  const location = useLocation();
  const navigate = useNavigate();
  const width = useUiStore((state) => state.listWidth);
  const items = useNotesStore((state) => state.items);
  const total = useNotesStore((state) => state.total);
  const loading = useNotesStore((state) => state.loading);
  const scope = useNotesStore((state) => state.scope);
  const sort = useNotesStore((state) => state.sort);
  const keyword = useNotesStore((state) => state.keyword);
  const rootOnly = useNotesStore((state) => state.rootOnly);
  const setRootOnly = useNotesStore((state) => state.setRootOnly);
  const setSort = useNotesStore((state) => state.setSort);
  const setKeyword = useNotesStore((state) => state.setKeyword);
  const selectedId = useNotesStore((state) => state.selectedId);
  const categories = useCategoriesStore((state) => state.flat);
  const tags = useTagsStore((state) => state.tags);
  /** 工作区设置里记住的排序（含「自定义顺序」）；切换知识库后随之恢复 */
  const preferredSort = useWorkspaceStore((state) => state.settings?.listSort);

  const selectionMode = useNotesStore((state) => state.selectionMode);
  const selection = useNotesStore((state) => state.selection);
  const setSelectionMode = useNotesStore((state) => state.setSelectionMode);
  const setSelection = useNotesStore((state) => state.setSelection);
  const deleteMany = useNotesStore((state) => state.deleteMany);
  const [batchMoveOpen, setBatchMoveOpen] = useState(false);
  const [batchTagOpen, setBatchTagOpen] = useState(false);
  const [batchCopyOpen, setBatchCopyOpen] = useState(false);
  /** 折叠的父文档 id（默认全部展开） */
  const [collapsedIds, setCollapsedIds] = useState<string[]>([]);

  const collapsed = useMemo(() => new Set(collapsedIds), [collapsedIds]);
  const rows = useMemo(() => buildNoteTree(items, collapsed), [items, collapsed]);

  /** 拖拽悬停位置：{ 行 id, 落点 }，用于渲染蓝色横线 / 蓝色框 */
  const [dropHint, setDropHint] = useState<{ id: string; position: NoteDropPosition } | null>(null);

  const toggleCollapse = (id: string) =>
    setCollapsedIds((previous) =>
      previous.includes(id) ? previous.filter((item) => item !== id) : [...previous, id],
    );

  const planDrop = (targetId: string, position: NoteDropPosition, draggedIds: string[]) =>
    planNoteDrop({ items, targetId, draggedIds, position });

  const handleNoteDragOver = (targetId: string, position: NoteDropPosition, draggedIds: string[]) => {
    // 落点非法（拖进自己或自己的子树）时不给出提示，drop 时再兜一层
    const plan = planDrop(targetId, position, draggedIds);
    setDropHint(plan ? { id: targetId, position } : null);
  };

  const handleNoteDrop = async (
    targetId: string,
    position: NoteDropPosition,
    draggedIds: string[],
  ) => {
    setDropHint(null);
    const plan = planDrop(targetId, position, draggedIds);
    if (!plan) {
      toast.error('不能把笔记移动到它自己的子文档下');
      return;
    }
    const store = useNotesStore.getState();

    if (plan.changesParent) {
      for (const noteId of draggedIds) {
        const ok = await store.setParent(noteId, plan.parentId);
        if (!ok) return;
      }
    }

    if (position === 'inside') {
      // 拖进去以后自动展开，否则看不到刚归位的子文档
      setCollapsedIds((previous) => previous.filter((id) => id !== targetId));
      if (!plan.changesParent) return;
      toast.success(`已移动到「${items.find((item) => item.id === targetId)?.title ?? ''}」下`);
      return;
    }

    // 同层插入需要「自定义顺序」才有意义，否则排序会被其它排序键覆盖
    if (store.sort !== 'manual') {
      await store.setSort('manual');
      toast.info('已切换为「自定义顺序」，可继续拖拽调整');
    }
    await store.reorder(plan.siblingIds);
  };

  const basePath = `/${location.pathname.split('/').filter(Boolean)[0] ?? 'notes'}`;

  const selectable = scope.kind !== 'trash' && scope.kind !== 'missing';

  useEffect(() => {
    if (!preferredSort) return;
    useNotesStore.getState().hydrateSort(preferredSort as NoteSort);
  }, [preferredSort]);

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
          title: keyword
            ? '没有匹配的笔记'
            : rootOnly
              ? '没有不在目录中的笔记'
              : '这里还没有笔记',
          description: keyword
            ? '尝试更换关键词，或清空筛选条件。'
            : rootOnly
              ? '当前所有笔记都被某个父文档收纳了。关闭「不在目录中」可查看全部笔记。'
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
          {/* 计数紧贴标题（一眼看出"这是哪个范围的几篇"），最右侧留给大纲开关 */}
          <div className="flex min-w-0 items-baseline gap-1.5">
            <h1 className="truncate text-sm font-semibold text-[var(--ink)]">{title}</h1>
            <span
              data-testid="note-list-count"
              className="shrink-0 text-[11px] text-[var(--ink-subtle)]"
            >
              {total} 篇
            </span>
          </div>
          <OutlineToggleButton hasNote={hasNote} />
        </div>
        <div className="relative">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--ink-subtle)]" />
          <Input
            aria-label="按标题筛选笔记"
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
              data-testid="batch-copy"
              className="flex items-center gap-0.5 disabled:opacity-40"
              disabled={selection.length === 0}
              onClick={() => setBatchCopyOpen(true)}
            >
              <Copy className="h-3 w-3" />
              复制到
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

        {/* 视图开关 + 排序：窄列表下换行，但按钮文字不折断 */}
        <div className="flex flex-wrap items-center gap-x-1 gap-y-0.5">
          {selectable ? (
            <button
              type="button"
              title="多选（批量移动 / 打标签 / 删除）"
              className="flex shrink-0 items-center gap-0.5 whitespace-nowrap rounded px-1.5 py-0.5 text-[11px] text-[var(--ink-subtle)] transition-colors hover:text-[var(--brand)]"
              onClick={() => setSelectionMode(!selectionMode)}
            >
              <CheckSquare className="h-3 w-3" />
              多选
            </button>
          ) : null}
          {selectable ? (
            <button
              type="button"
              data-testid="root-only-toggle"
              title="只看没有被父文档收纳的顶层文档（对标语雀的「不在目录中」）"
              aria-pressed={rootOnly}
              className={cn(
                'flex shrink-0 items-center gap-0.5 whitespace-nowrap rounded px-1.5 py-0.5 text-[11px] transition-colors',
                rootOnly
                  ? 'bg-[var(--brand-soft)] text-[var(--brand)]'
                  : 'text-[var(--ink-subtle)] hover:text-[var(--ink)]',
              )}
              onClick={() => void setRootOnly(!rootOnly)}
            >
              <ListTree className="h-3 w-3" />
              不在目录中
            </button>
          ) : null}
          {rows.some((row) => row.childCount > 0) ? (
            <button
              type="button"
              data-testid="toggle-all-fold"
              title={collapsedIds.length > 0 ? '展开全部子文档' : '折叠全部子文档'}
              className="flex shrink-0 items-center gap-0.5 whitespace-nowrap rounded px-1.5 py-0.5 text-[11px] text-[var(--ink-subtle)] transition-colors hover:text-[var(--brand)]"
              onClick={() =>
                setCollapsedIds((previous) =>
                  previous.length > 0
                    ? []
                    : rows.filter((row) => row.childCount > 0).map((row) => row.note.id),
                )
              }
            >
              {collapsedIds.length > 0 ? (
                <ChevronsUpDown className="h-3 w-3" />
              ) : (
                <ChevronsDownUp className="h-3 w-3" />
              )}
              {collapsedIds.length > 0 ? '展开全部' : '折叠全部'}
            </button>
          ) : null}
          {SORT_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              onClick={() => void setSort(option.value)}
              className={cn(
                'shrink-0 whitespace-nowrap rounded px-1.5 py-0.5 text-[11px] transition-colors',
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
                        dropHint={dropHint?.id === note.id ? dropHint.position : null}
                        onNoteDragOver={(position, draggedIds) =>
                          handleNoteDragOver(note.id, position, draggedIds)
                        }
                        onNoteDragLeave={() =>
                          setDropHint((current) => (current?.id === note.id ? null : current))
                        }
                        onNoteDrop={(position, draggedIds) =>
                          void handleNoteDrop(note.id, position, draggedIds)
                        }
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
                  dropHint={dropHint?.id === note.id ? dropHint.position : null}
                  onNoteDragOver={(position, draggedIds) =>
                    handleNoteDragOver(note.id, position, draggedIds)
                  }
                  onNoteDragLeave={() =>
                    setDropHint((current) => (current?.id === note.id ? null : current))
                  }
                  onNoteDrop={(position, draggedIds) =>
                    void handleNoteDrop(note.id, position, draggedIds)
                  }
                  onOpen={() => navigate(`${basePath}/${note.id}`)}
                />
              ))}
            </ul>
          )}
        </ScrollArea>
      )}

      {batchMoveOpen ? <BatchMoveDialog onClose={() => setBatchMoveOpen(false)} /> : null}
      {batchTagOpen ? <BatchTagDialog onClose={() => setBatchTagOpen(false)} /> : null}
      {batchCopyOpen ? <BatchCopyDialog onClose={() => setBatchCopyOpen(false)} /> : null}

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
                    reportFailure('永久删除失败', errorMessage(error));
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
