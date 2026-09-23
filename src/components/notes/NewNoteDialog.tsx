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
import { Input, Label, Textarea } from '@/components/ui/input';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';

/** 新建笔记（设计文档 5.2 FR-N01）。 */
export function NewNoteDialog() {
  const open = useUiStore((state) => state.newNoteDialogOpen);
  const setOpen = useUiStore((state) => state.setNewNoteDialogOpen);
  // 表单在打开时挂载，关闭即销毁，因此无需在 effect 中重置状态
  if (!open) return null;
  return <NewNoteForm onClose={() => setOpen(false)} />;
}

function NewNoteForm({ onClose }: { onClose: () => void }) {
  const categories = useCategoriesStore((state) => state.flat);
  const pathOf = useCategoriesStore((state) => state.pathOf);
  const create = useNotesStore((state) => state.create);
  const navigate = useNavigate();

  const [title, setTitle] = useState('');
  const [categoryId, setCategoryId] = useState<string>('');
  const [tags, setTags] = useState('');
  const [content, setContent] = useState('');
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    const trimmed = title.trim();
    if (!trimmed) {
      toast.error('请填写标题');
      return;
    }
    setBusy(true);
    const detail = await create({
      title: trimmed,
      categoryId: categoryId || null,
      initialContent: content,
      tags: tags
        .split(',')
        .map((tag) => tag.trim())
        .filter(Boolean),
    });
    setBusy(false);
    if (detail) {
      onClose();
      toast.success('已创建笔记');
      navigate(`/notes/${detail.note.id}`);
    } else {
      toast.error(useNotesStore.getState().error ?? '创建失败');
    }
  };

  return (
    <Dialog open onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>新建笔记</DialogTitle>
          <DialogDescription>
            笔记会立即写入工作区目录（默认 Inbox），并建立全文索引。
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="new-note-title">标题</Label>
            <Input
              id="new-note-title"
              autoFocus
              value={title}
              placeholder="例如：SQLite 全文搜索学习笔记"
              onChange={(event) => setTitle(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) void submit();
              }}
            />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1.5">
              <Label htmlFor="new-note-category">分类</Label>
              <select
                id="new-note-category"
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
            <div className="space-y-1.5">
              <Label htmlFor="new-note-tags">标签（逗号分隔）</Label>
              <Input
                id="new-note-tags"
                value={tags}
                placeholder="sqlite, search"
                onChange={(event) => setTags(event.target.value)}
              />
            </div>
          </div>
          <div className="space-y-1.5">
            <Label htmlFor="new-note-content">初始内容（可选）</Label>
            <Textarea
              id="new-note-content"
              value={content}
              rows={5}
              placeholder="支持 Markdown 语法"
              onChange={(event) => setContent(event.target.value)}
            />
          </div>
        </div>
        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            取消
          </Button>
          <Button onClick={() => void submit()} disabled={busy}>
            创建笔记
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
