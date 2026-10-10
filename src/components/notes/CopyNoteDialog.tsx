import { Copy } from 'lucide-react';
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
import { errorMessage } from '@/lib/format';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import type { NoteSummary } from '@/types';

/**
 * 单篇「复制到指定分类」（对标语雀条目菜单里的「复制…」）。
 *
 * 与「创建副本」的区别：副本落到指定分类，原作保持不动；
 * 与批量「复制到」共用同一条语义（先 duplicate 再 move，标题带「副本」后缀）。
 */
export function CopyNoteDialog({ note, onClose }: { note: NoteSummary; onClose: () => void }) {
  const categories = useCategoriesStore((state) => state.flat);
  const pathOf = useCategoriesStore((state) => state.pathOf);
  const [categoryId, setCategoryId] = useState(note.categoryId ?? '');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      // 复用仓储动作：duplicate + move，且不删除原作
      const ok = await useNotesStore
        .getState()
        .copyTo(note.id, categoryId || null);
      if (ok) {
        toast.success(
          categoryId ? `已复制到「${pathOf(categoryId)}」（原作保留）` : '已复制到 Inbox（原作保留）',
        );
        onClose();
      } else {
        toast.error(useNotesStore.getState().error ?? '复制失败');
      }
    } catch (error) {
      toast.error(errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(open) => (!open ? onClose() : undefined)}>
      <DialogContent className="w-[min(520px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-1.5">
            <Copy className="h-4 w-4 text-[var(--brand)]" />
            复制到分类
          </DialogTitle>
          <DialogDescription>
            复制「{note.title}」到指定分类；**原作保留在原位**，副本标题带「副本」后缀，
            标签与父文档层级一并保留。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-1.5">
          <Label htmlFor="copy-note-category">目标分类</Label>
          <select
            id="copy-note-category"
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
          <Button onClick={() => void submit()} disabled={busy} data-testid="copy-note-confirm">
            <Copy className="h-3.5 w-3.5" />
            复制副本
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
