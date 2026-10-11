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
import { useNotesStore } from '@/stores/notesStore';

/**
 * 外部修改冲突提示（设计文档 19.3）。
 * 提供「重新加载」与「保留当前内容」两种选择，不会静默覆盖用户内容。
 */
export function ConflictDialog({
  onKeepLocal,
}: {
  onKeepLocal: (overwrite: boolean) => void;
}) {
  const conflict = useNotesStore((state) => state.conflict);
  const clearConflict = useNotesStore((state) => state.clearConflict);
  const openDetail = useNotesStore((state) => state.openDetail);

  if (!conflict) return null;

  return (
    <Dialog open onOpenChange={(open) => (!open ? clearConflict() : undefined)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>文件已被外部修改</DialogTitle>
          <DialogDescription>
            磁盘上的文件与编辑开始时的版本不一致。为了避免丢失内容，请选择处理方式。
          </DialogDescription>
        </DialogHeader>
        <pre className="max-h-40 overflow-auto rounded-md bg-[var(--surface-muted)] p-3 text-xs text-[var(--ink-muted)]">
          {conflict.currentContent.slice(0, 1200) || '（磁盘内容为空）'}
        </pre>
        <DialogFooter>
          <Button
            variant="secondary"
            onClick={() => {
              clearConflict();
              // 用户明确选择「以磁盘为准」：跳过「切换前落盘」，不要把本地内容写回去
              void openDetail(conflict.noteId, { discardLocalEdits: true }).then(() =>
                toast.info('已重新加载磁盘内容'),
              );
            }}
          >
            重新加载磁盘内容
          </Button>
          <Button
            onClick={() => {
              clearConflict();
              onKeepLocal(true);
            }}
          >
            用当前内容覆盖
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
