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
import { Label } from '@/components/ui/input';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';

/** 移动笔记到其它分类（设计文档 FR-N07）。 */
export function MoveNoteDialog({
  noteId,
  currentCategoryId,
  onClose,
}: {
  noteId: string;
  currentCategoryId?: string | null;
  onClose: () => void;
}) {
  const categories = useCategoriesStore((state) => state.flat);
  const pathOf = useCategoriesStore((state) => state.pathOf);
  const move = useNotesStore((state) => state.move);
  const [categoryId, setCategoryId] = useState(currentCategoryId ?? '');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    await move(noteId, categoryId || null);
    setBusy(false);
    toast.success('笔记已移动');
    onClose();
  };

  return (
    <Dialog open onOpenChange={(open) => (!open ? onClose() : undefined)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>移动笔记</DialogTitle>
          <DialogDescription>
            文件会移动到目标分类目录，Markdown 中的 category 字段会同步更新。
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-1.5">
          <Label htmlFor="move-note-category">目标分类</Label>
          <select
            id="move-note-category"
            value={categoryId}
            onChange={(event) => setCategoryId(event.target.value)}
            className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm"
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
            移动
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
