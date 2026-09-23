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
import { Input, Label } from '@/components/ui/input';
import { useCategoriesStore } from '@/stores/categoriesStore';
import type { CategoryDeleteStrategy } from '@/types';

export function CategoryFormDialog({
  mode,
  parentId = null,
  categoryId,
  initialName = '',
  onClose,
}: {
  mode: 'create' | 'rename';
  parentId?: string | null;
  categoryId?: string;
  initialName?: string;
  onClose: () => void;
}) {
  const [name, setName] = useState(initialName);
  const [busy, setBusy] = useState(false);
  const create = useCategoriesStore((state) => state.create);
  const rename = useCategoriesStore((state) => state.rename);

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      toast.error('请填写分类名称');
      return;
    }
    setBusy(true);
    const ok =
      mode === 'create'
        ? (await create(trimmed, parentId)) !== null
        : await rename(categoryId ?? '', trimmed);
    setBusy(false);
    if (ok) {
      toast.success(mode === 'create' ? '分类已创建' : '分类已重命名');
      onClose();
    } else {
      toast.error(useCategoriesStore.getState().error ?? '操作失败');
    }
  };

  return (
    <Dialog open onOpenChange={(open) => (!open ? onClose() : undefined)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{mode === 'create' ? '新建分类' : '重命名分类'}</DialogTitle>
          <DialogDescription>
            分类会映射到工作区中的目录，重命名会同步修改目录名与笔记路径。
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-1.5">
          <Label htmlFor="category-name">分类名称</Label>
          <Input
            id="category-name"
            autoFocus
            value={name}
            placeholder="例如：数据库"
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') void submit();
            }}
          />
        </div>
        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={busy}>
            {mode === 'create' ? '创建' : '保存'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

const STRATEGIES: Array<{ value: CategoryDeleteStrategy; label: string; hint: string }> = [
  { value: 'moveToInbox', label: '移动到 Inbox', hint: '笔记移入 00_Inbox 并取消分类' },
  { value: 'moveToParent', label: '移动到父分类', hint: '保留在上级分类目录中' },
  { value: 'unassign', label: '仅取消分类', hint: '文件保持原位置，不再属于任何分类' },
];

export function CategoryDeleteDialog({
  categoryId,
  name,
  onClose,
  onDeleted,
}: {
  categoryId: string;
  name: string;
  onClose: () => void;
  onDeleted: (movedNotes: number) => void;
}) {
  const [strategy, setStrategy] = useState<CategoryDeleteStrategy>('moveToInbox');
  const [busy, setBusy] = useState(false);
  const remove = useCategoriesStore((state) => state.remove);

  const submit = async () => {
    setBusy(true);
    const report = await remove(categoryId, strategy);
    setBusy(false);
    if (report) {
      onDeleted(report.movedNotes + report.unassignedNotes);
    } else {
      toast.error(useCategoriesStore.getState().error ?? '删除失败');
    }
  };

  return (
    <Dialog open onOpenChange={(open) => (!open ? onClose() : undefined)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>删除分类「{name}」</DialogTitle>
          <DialogDescription>
            子分类会一并删除，请选择如何处理其中的笔记。该操作不会删除笔记正文。
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-2">
          {STRATEGIES.map((option) => (
            <label
              key={option.value}
              className="flex cursor-pointer items-start gap-2 rounded-md border border-[var(--border-subtle)] p-3 text-sm hover:bg-[var(--surface-muted)]"
            >
              <input
                type="radio"
                name="category-delete-strategy"
                className="mt-1"
                checked={strategy === option.value}
                onChange={() => setStrategy(option.value)}
              />
              <span>
                <span className="block font-medium text-[var(--ink)]">{option.label}</span>
                <span className="block text-xs text-[var(--ink-subtle)]">{option.hint}</span>
              </span>
            </label>
          ))}
        </div>
        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button variant="danger" onClick={() => void submit()} disabled={busy}>
            删除分类
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
