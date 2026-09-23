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
import { useNotesStore } from '@/stores/notesStore';

/** 重命名笔记标题与文件名（设计文档 FR-N06）。 */
export function RenameFileDialog({
  noteId,
  initialTitle,
  onClose,
}: {
  noteId: string;
  initialTitle: string;
  onClose: () => void;
}) {
  const [title, setTitle] = useState(initialTitle);
  const [renameFile, setRenameFile] = useState(true);
  const [busy, setBusy] = useState(false);
  const rename = useNotesStore((state) => state.rename);

  const submit = async () => {
    const trimmed = title.trim();
    if (!trimmed) {
      toast.error('标题不能为空');
      return;
    }
    setBusy(true);
    await rename(noteId, trimmed, renameFile);
    setBusy(false);
    toast.success('已重命名');
    onClose();
  };

  return (
    <Dialog open onOpenChange={(open) => (!open ? onClose() : undefined)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>重命名笔记</DialogTitle>
          <DialogDescription>
            标题会写入 frontmatter；勾选同步文件名时会保持原有时间戳前缀并避免重名冲突。
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="rename-title">标题</Label>
            <Input
              id="rename-title"
              autoFocus
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') void submit();
              }}
            />
          </div>
          <label className="flex items-center gap-2 text-sm text-[var(--ink-muted)]">
            <input
              type="checkbox"
              checked={renameFile}
              onChange={(event) => setRenameFile(event.target.checked)}
            />
            同步修改 Markdown 文件名
          </label>
        </div>
        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={busy}>
            保存
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
