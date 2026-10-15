import { ChevronDown, ChevronRight, FolderTree, MoreHorizontal, Plus } from 'lucide-react';
import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { EmptyState } from '@/components/ui/empty-state';
import { isNoteDrag, readNoteDragIds } from '@/lib/dragNotes';
import { cn } from '@/lib/utils';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';
import type { CategoryNode } from '@/types';

import { CategoryDeleteDialog, CategoryFormDialog } from './CategoryDialogs';

type DialogState =
  | { kind: 'create'; parentId: string | null }
  | { kind: 'rename'; id: string; name: string }
  | { kind: 'delete'; id: string; name: string }
  | null;

export function CategoryTree() {
  const tree = useCategoriesStore((state) => state.tree);
  const navigate = useNavigate();
  const categoriesCollapsed = useUiStore((state) => state.categoriesCollapsed);
  const toggleCategoriesCollapsed = useUiStore((state) => state.toggleCategoriesCollapsed);
  const [params] = useSearchParams();
  const activeId = params.get('categoryId');
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [dialog, setDialog] = useState<DialogState>(null);
  /** 正在拖拽悬停的分类（用于高亮提示可放置） */
  const [dropTarget, setDropTarget] = useState<string | null>(null);

  const handleDrop = (categoryId: string, event: React.DragEvent) => {
    const ids = readNoteDragIds(event.dataTransfer);
    setDropTarget(null);
    if (ids.length === 0) return;
    event.preventDefault();
    void useNotesStore.getState().moveNotes(ids, categoryId);
  };

  const toggle = (id: string) => {
    setExpanded((previous) => {
      const next = new Set(previous);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const select = (id: string) => navigate(`/notes?categoryId=${encodeURIComponent(id)}`);

  const renderNode = (node: CategoryNode) => {
    const hasChildren = node.children.length > 0;
    const isExpanded = expanded.has(node.category.id);
    return (
      <li key={node.category.id}>
        <div
          data-testid={`category-drop-${node.category.id}`}
          className={cn(
            'group flex items-center gap-1 rounded-md pr-1 text-sm transition-colors',
            activeId === node.category.id
              ? 'bg-[var(--brand-soft)] text-[var(--brand)]'
              : 'text-[var(--ink-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]',
            dropTarget === node.category.id && 'ring-2 ring-[var(--brand)]/60',
          )}
          style={{ paddingLeft: `${node.depth * 10}px` }}
          onDragOver={(event) => {
            if (!isNoteDrag(event.dataTransfer)) return;
            // 必须 preventDefault 才会触发 drop
            event.preventDefault();
            event.dataTransfer.dropEffect = 'move';
            if (dropTarget !== node.category.id) setDropTarget(node.category.id);
          }}
          onDragLeave={() => setDropTarget((current) => (current === node.category.id ? null : current))}
          onDrop={(event) => handleDrop(node.category.id, event)}
        >
          <button
            type="button"
            aria-label={isExpanded ? '折叠' : '展开'}
            className={cn(
              'flex h-5 w-5 items-center justify-center rounded transition-transform',
              !hasChildren && 'opacity-0',
              isExpanded && 'rotate-90',
            )}
            onClick={() => toggle(node.category.id)}
          >
            <ChevronRight className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            className="flex min-w-0 flex-1 items-center gap-1.5 py-1 text-left"
            onClick={() => select(node.category.id)}
          >
            <span className="truncate">{node.category.name}</span>
            {node.category.noteCount > 0 ? (
              <span className="text-[11px] tabular-nums text-[var(--ink-subtle)]">
                {node.category.noteCount}
              </span>
            ) : null}
          </button>
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button
                type="button"
                aria-label="分类操作"
                className="rounded p-0.5 opacity-0 transition-opacity group-hover:opacity-100 data-[state=open]:opacity-100"
              >
                <MoreHorizontal className="h-3.5 w-3.5" />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              <DropdownMenuItem
                onSelect={() => setDialog({ kind: 'create', parentId: node.category.id })}
              >
                新建子分类
              </DropdownMenuItem>
              <DropdownMenuItem
                onSelect={() =>
                  setDialog({ kind: 'rename', id: node.category.id, name: node.category.name })
                }
              >
                重命名
              </DropdownMenuItem>
              <DropdownMenuItem
                destructive
                onSelect={() =>
                  setDialog({ kind: 'delete', id: node.category.id, name: node.category.name })
                }
              >
                删除分类
              </DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
        {hasChildren && isExpanded ? (
          <ul className="mt-0.5 space-y-0.5">{node.children.map(renderNode)}</ul>
        ) : null}
      </li>
    );
  };

  return (
    <section className="space-y-1">
      <header className="flex items-center justify-between px-2">
        {/* 区块可折叠：分类多了以后侧边栏会被顶得很长，折叠起来给别的东西腾地方 */}
        <button
          type="button"
          data-testid="categories-toggle"
          aria-expanded={!categoriesCollapsed}
          // 无障碍名要说清"点它是折叠还是展开"：光有「分类」两个字，读屏听不出这是个开关
          aria-label={categoriesCollapsed ? '展开分类' : '折叠分类'}
          title={categoriesCollapsed ? '展开分类' : '折叠分类'}
          className="flex min-w-0 items-center gap-0.5 rounded text-[11px] font-semibold uppercase tracking-wide text-[var(--ink-subtle)] transition-colors hover:text-[var(--ink)]"
          onClick={toggleCategoriesCollapsed}
        >
          {categoriesCollapsed ? (
            <ChevronRight className="h-3 w-3" />
          ) : (
            <ChevronDown className="h-3 w-3" />
          )}
          分类
        </button>
        <Button
          variant="ghost"
          size="icon"
          title="新建分类"
          className="h-6 w-6"
          onClick={() => setDialog({ kind: 'create', parentId: null })}
        >
          <Plus className="h-3.5 w-3.5" />
        </Button>
      </header>
      {categoriesCollapsed ? null : tree.length === 0 ? (
        <EmptyState
          icon={FolderTree}
          title="还没有分类"
          description="创建分类后，笔记会按目录自动整理。"
          className="py-6"
          action={
            <Button size="sm" variant="secondary" onClick={() => setDialog({ kind: 'create', parentId: null })}>
              <Plus className="h-3.5 w-3.5" />
              新建分类
            </Button>
          }
        />
      ) : (
        <ul className="space-y-0.5">{tree.map(renderNode)}</ul>
      )}

      {dialog?.kind === 'create' ? (
        <CategoryFormDialog
          mode="create"
          parentId={dialog.parentId}
          onClose={() => setDialog(null)}
        />
      ) : null}
      {dialog?.kind === 'rename' ? (
        <CategoryFormDialog
          mode="rename"
          categoryId={dialog.id}
          initialName={dialog.name}
          onClose={() => setDialog(null)}
        />
      ) : null}
      {dialog?.kind === 'delete' ? (
        <CategoryDeleteDialog
          categoryId={dialog.id}
          name={dialog.name}
          onClose={() => setDialog(null)}
          onDeleted={(moved) => {
            toast.success(`分类已删除，处理了 ${moved} 篇笔记`);
            setDialog(null);
            navigate('/notes');
          }}
        />
      ) : null}
    </section>
  );
}
