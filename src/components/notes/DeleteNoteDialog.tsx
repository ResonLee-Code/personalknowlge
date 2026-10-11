import { AlertTriangle, Trash2 } from 'lucide-react';
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
import { errorMessage } from '@/lib/format';
import { useNotesStore } from '@/stores/notesStore';
import type { NoteSummary } from '@/types';

/**
 * 删除「有子文档」的笔记时的确认框（V1.2.1 P2 子文档约束）。
 *
 * 只删父文档时子文档不会被删：它们的 `parent` 指向已删除的笔记，
 * 列表会把它们按顶层展示（不会消失）。也可以勾选连同子文档一起移入回收站。
 */
export function DeleteNoteDialog({
  note,
  children,
  onClose,
  onDeleted,
}: {
  note: NoteSummary;
  /** 直接子文档 */
  children: NoteSummary[];
  onClose: () => void;
  onDeleted?: () => void;
}) {
  const remove = useNotesStore((state) => state.remove);
  const restore = useNotesStore((state) => state.restore);
  const [withChildren, setWithChildren] = useState(false);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setBusy(true);
    try {
      const removedOk = await remove(note.id);
      if (!removedOk) {
        toast.error(useNotesStore.getState().error ?? '移入回收站失败');
        return;
      }
      if (withChildren) {
        for (const child of children) {
          await remove(child.id);
        }
      }
      toast.success(
        withChildren ? `已移入回收站（含 ${children.length} 篇子文档）` : '已移入回收站',
        {
          action: {
            label: '撤销',
            onClick: () =>
              void (async () => {
                const targets = [note.id, ...(withChildren ? children.map((child) => child.id) : [])];
                let restored = 0;
                let firstError: string | null = null;
                for (const id of targets) {
                  const ok = await restore(id);
                  if (ok) restored += 1;
                  else firstError ??= useNotesStore.getState().error ?? '恢复失败';
                }
                if (restored === targets.length) {
                  toast.success(`已恢复 ${restored} 篇笔记`);
                } else {
                  toast.error(
                    `已恢复 ${restored}/${targets.length} 篇，${
                      targets.length - restored
                    } 篇失败${firstError ? `：${firstError}` : ''}`,
                  );
                }
              })(),
          },
        },
      );
      onDeleted?.();
      onClose();
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
            <AlertTriangle className="h-4 w-4 text-amber-500" />
            删除有子文档的笔记
          </DialogTitle>
          <DialogDescription>
            「{note.title}」下面还有 {children.length} 篇子文档。只删除父文档时，
            子文档会**保留**并回到列表顶层展示（不会消失）。
          </DialogDescription>
        </DialogHeader>

        <ul className="max-h-40 space-y-1 overflow-auto rounded-md border border-[var(--border-subtle)] p-2 text-xs text-[var(--ink-muted)]">
          {children.slice(0, 10).map((child) => (
            <li key={child.id} className="truncate">
              · {child.title}
            </li>
          ))}
          {children.length > 10 ? <li>… 另有 {children.length - 10} 篇</li> : null}
        </ul>

        <label className="flex items-center gap-2 text-xs text-[var(--ink-muted)]">
          <input
            type="checkbox"
            data-testid="delete-with-children"
            checked={withChildren}
            onChange={(event) => setWithChildren(event.target.checked)}
          />
          连同 {children.length} 篇子文档一起移入回收站
        </label>

        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={busy} data-testid="delete-confirm">
            <Trash2 className="h-3.5 w-3.5" />
            移入回收站
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
