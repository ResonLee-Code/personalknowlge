import { FolderTree, MoreHorizontal, Plus } from 'lucide-react';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { CategoryDeleteDialog, CategoryFormDialog } from '@/components/categories/CategoryDialogs';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { useCategoriesStore } from '@/stores/categoriesStore';
import type { CategoryNode } from '@/types';

/** 分类管理页面。 */
export function CategoriesPage() {
  const tree = useCategoriesStore((state) => state.tree);
  const navigate = useNavigate();
  const [dialog, setDialog] = useState<
    | { kind: 'create'; parentId: string | null }
    | { kind: 'rename'; id: string; name: string }
    | { kind: 'delete'; id: string; name: string }
    | null
  >(null);

  const rows: CategoryNode[] = [];
  const walk = (nodes: CategoryNode[]) => {
    nodes.forEach((node) => {
      rows.push(node);
      walk(node.children);
    });
  };
  walk(tree);

  return (
    <div className="h-full overflow-auto p-6">
      <div className="mx-auto max-w-3xl space-y-4">
        <header className="flex items-center justify-between">
          <div>
            <h1 className="text-lg font-semibold text-[var(--ink)]">分类管理</h1>
            <p className="mt-1 text-xs text-[var(--ink-subtle)]">
              分类与工作区目录一一对应，重命名会同步移动目录与笔记文件。
            </p>
          </div>
          <Button onClick={() => setDialog({ kind: 'create', parentId: null })}>
            <Plus className="h-4 w-4" />
            新建分类
          </Button>
        </header>

        {rows.length === 0 ? (
          <EmptyState
            icon={FolderTree}
            title="还没有分类"
            description="创建分类后，笔记会按目录整理，也便于按分类搜索。"
            action={
              <Button variant="secondary" onClick={() => setDialog({ kind: 'create', parentId: null })}>
                新建分类
              </Button>
            }
          />
        ) : (
          <ul className="divide-y divide-[var(--border-subtle)] rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)]">
            {rows.map((node) => (
              <li
                key={node.category.id}
                className="flex items-center gap-2 px-3 py-2"
                style={{ paddingLeft: `${12 + node.depth * 18}px` }}
              >
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm text-[var(--ink)]">
                    {node.category.name}
                  </span>
                  <span className="block truncate text-[11px] text-[var(--ink-subtle)]">
                    {node.category.dirName || '（无目录）'} · {node.category.noteCount} 篇
                  </span>
                </span>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => navigate(`/notes?categoryId=${node.category.id}`)}
                >
                  查看笔记
                </Button>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <Button size="icon" variant="ghost" title="分类操作">
                      <MoreHorizontal className="h-4 w-4" />
                    </Button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem
                      onSelect={() => setDialog({ kind: 'create', parentId: node.category.id })}
                    >
                      新建子分类
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      onSelect={() =>
                        setDialog({
                          kind: 'rename',
                          id: node.category.id,
                          name: node.category.name,
                        })
                      }
                    >
                      重命名
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      destructive
                      onSelect={() =>
                        setDialog({
                          kind: 'delete',
                          id: node.category.id,
                          name: node.category.name,
                        })
                      }
                    >
                      删除分类
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </li>
            ))}
          </ul>
        )}
      </div>

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
          }}
        />
      ) : null}
    </div>
  );
}
