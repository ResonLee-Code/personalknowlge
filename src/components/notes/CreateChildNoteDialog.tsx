import { FilePlus2 } from 'lucide-react';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
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
import { ROUTES } from '@/lib/constants';
import { useNotesStore } from '@/stores/notesStore';
import type { NoteSummary } from '@/types';

/**
 * 为某篇（父）文档创建子文档（对标语雀「+ 新建子文档」）。
 * 子文档的分类默认继承父文档，父子关系写入子文档 frontmatter 的 `parent` 字段。
 */
export function CreateChildNoteDialog({
  parent,
  onClose,
}: {
  parent: NoteSummary;
  onClose: () => void;
}) {
  const create = useNotesStore((state) => state.create);
  /** 同层重名提示（不阻断创建，只提醒） */
  const siblings = useNotesStore((state) => state.items).filter(
    (item) => item.parentId === parent.id,
  );
  const navigate = useNavigate();
  const [title, setTitle] = useState('');
  const [busy, setBusy] = useState(false);

  const duplicated = title.trim()
    ? siblings.some((item) => item.title.trim().toLowerCase() === title.trim().toLowerCase())
    : false;

  const submit = async () => {
    const trimmed = title.trim();
    if (!trimmed) {
      toast.error('请填写子文档标题');
      return;
    }
    setBusy(true);
    const detail = await create({ title: trimmed, parentId: parent.id });
    setBusy(false);
    if (detail) {
      onClose();
      toast.success(`已创建子文档`);
      navigate(`${ROUTES.notes}/${detail.note.id}`);
    } else {
      toast.error(useNotesStore.getState().error ?? '创建失败');
    }
  };

  return (
    <Dialog open onOpenChange={(open) => (!open ? onClose() : undefined)}>
      <DialogContent className="w-[min(480px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-1.5">
            <FilePlus2 className="h-4 w-4 text-[var(--brand)]" />
            新建子文档
          </DialogTitle>
          <DialogDescription>
            父文档：<span className="text-[var(--ink)]">{parent.title}</span>
            {parent.categoryPath ? `（分类 ${parent.categoryPath}）` : ''}
            。子文档会继承父文档的分类，可在笔记列表中展开 / 折叠；
            父文档也可以只当**分组**用（正文留空、标题写分组名，把文档都挂在它下面）。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-1.5">
          <Label htmlFor="child-note-title">子文档标题</Label>
          <Input
            id="child-note-title"
            autoFocus
            value={title}
            placeholder="例如：1.2 范围"
            onChange={(event) => setTitle(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') void submit();
            }}
          />
          {duplicated ? (
            <p data-testid="child-title-duplicate" className="text-[11px] text-amber-600">
              同一父文档下已有同名子文档（语雀允许重名，这里只做提醒）。
            </p>
          ) : null}
        </div>

        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={busy}>
            创建子文档
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
