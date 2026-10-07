import { FolderTree, Tag } from 'lucide-react';
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
import { useNotesStore } from '@/stores/notesStore';

/** 批量移动到分类（含 Inbox）。 */
export function BatchMoveDialog({ onClose }: { onClose: () => void }) {
  const categories = useCategoriesStore((state) => state.flat);
  const pathOf = useCategoriesStore((state) => state.pathOf);
  const selectedCount = useNotesStore((state) => state.selection.length);
  const moveMany = useNotesStore((state) => state.moveMany);
  const [categoryId, setCategoryId] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    await moveMany(categoryId || null);
    setBusy(false);
    onClose();
  };

  return (
    <Dialog open onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent className="w-[min(520px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <FolderTree className="h-4 w-4 text-[var(--brand)]" />
            批量移动分类
          </DialogTitle>
          <DialogDescription>把选中的 {selectedCount} 篇笔记移动到指定分类。</DialogDescription>
        </DialogHeader>

        <div className="space-y-1.5">
          <Label htmlFor="batch-category">目标分类</Label>
          <select
            id="batch-category"
            value={categoryId}
            onChange={(event) => setCategoryId(event.target.value)}
            className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)]"
          >
            <option value="">Inbox（未分类）</option>
            {categories.map((category) => (
              <option key={category.id} value={category.id}>
                {pathOf(category.id)}
              </option>
            ))}
          </select>
        </div>

        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={busy}>
            移动 {selectedCount} 篇
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** 批量打标签（add / remove / replace）。 */
export function BatchTagDialog({ onClose }: { onClose: () => void }) {
  const selectedCount = useNotesStore((state) => state.selection.length);
  const tagMany = useNotesStore((state) => state.tagMany);
  const [tags, setTags] = useState('');
  const [mode, setMode] = useState<'add' | 'remove' | 'replace'>('add');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    const list = tags
      .split(',')
      .map((tag) => tag.trim())
      .filter(Boolean);
    if (list.length === 0) {
      toast.error('请填写至少一个标签');
      return;
    }
    setBusy(true);
    await tagMany(list, mode);
    setBusy(false);
    onClose();
  };

  return (
    <Dialog open onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent className="w-[min(520px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Tag className="h-4 w-4 text-[var(--brand)]" />
            批量打标签
          </DialogTitle>
          <DialogDescription>对选中的 {selectedCount} 篇笔记批量处理标签。</DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="batch-tags">标签（逗号分隔）</Label>
            <Input
              id="batch-tags"
              value={tags}
              placeholder="整理, 待复核"
              onChange={(event) => setTags(event.target.value)}
            />
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="batch-tag-mode">处理方式</Label>
            <select
              id="batch-tag-mode"
              value={mode}
              onChange={(event) => setMode(event.target.value as 'add' | 'remove' | 'replace')}
              className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)]"
            >
              <option value="add">添加（保留已有标签）</option>
              <option value="remove">移除这些标签</option>
              <option value="replace">替换为这些标签</option>
            </select>
          </div>
        </div>

        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={busy}>
            应用到 {selectedCount} 篇
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
