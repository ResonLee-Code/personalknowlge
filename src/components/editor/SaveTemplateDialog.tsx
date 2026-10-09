import { FilePlus2, Loader2 } from 'lucide-react';
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
import { errorMessage } from '@/lib/format';
import { isTauriRuntime } from '@/services/api';
import { templateService } from '@/services/templateService';

/**
 * 把当前笔记正文另存为模板（对标语雀「另存为模板」）。
 * 模板是 `.knowledge/templates/<name>.md` 里的普通 Markdown 文件，用户也可直接编辑。
 */
export function SaveTemplateDialog({
  defaultName,
  content,
  onClose,
  onSaved,
}: {
  defaultName: string;
  /** 笔记正文（不含 frontmatter） */
  content: string;
  onClose: () => void;
  onSaved?: () => void;
}) {
  const [name, setName] = useState(defaultName);
  const [overwrite, setOverwrite] = useState(false);
  const [busy, setBusy] = useState(false);
  const desktop = isTauriRuntime();

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      toast.error('请填写模板名');
      return;
    }
    if (!content.trim()) {
      toast.error('笔记正文是空的，没有可保存为模板的内容');
      return;
    }
    setBusy(true);
    try {
      const saved = await templateService.save({ name: trimmed, content, overwrite });
      toast.success(`已保存为模板「${saved.name}」`);
      onSaved?.();
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
            <FilePlus2 className="h-4 w-4 text-[var(--brand)]" />
            另存为模板
          </DialogTitle>
          <DialogDescription>
            把当前正文存成模板，下次「新建笔记 → 模板」即可复用；文件落在
            <code className="mx-1">.knowledge/templates</code>，可以直接用其它编辑器修改。
            正文中的 <code>{'{{title}}'}</code> 等占位符会在套用时替换。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="template-name">模板名</Label>
            <Input
              id="template-name"
              autoFocus
              value={name}
              placeholder="例如：项目周报"
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') void submit();
              }}
            />
          </div>
          <label className="flex items-center gap-2 text-xs text-[var(--ink-muted)]">
            <input
              type="checkbox"
              checked={overwrite}
              onChange={(event) => setOverwrite(event.target.checked)}
            />
            覆盖同名模板
          </label>
          {!desktop ? (
            <p className="text-[11px] text-amber-600">
              浏览器预览模式没有工作区目录，请在桌面应用中使用该功能。
            </p>
          ) : null}
        </div>

        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={busy || !desktop}>
            {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null}
            保存为模板
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
