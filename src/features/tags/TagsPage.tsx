import { Pencil, Tag as TagIcon, Trash2 } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { reportAction } from '@/lib/actionFeedback';
import { useTagsStore } from '@/stores/tagsStore';

/** 标签管理页面（设计文档 16.2）。 */
export function TagsPage() {
  const tags = useTagsStore((state) => state.tags);
  const refresh = useTagsStore((state) => state.refresh);
  const rename = useTagsStore((state) => state.rename);
  const remove = useTagsStore((state) => state.remove);
  const navigate = useNavigate();
  const [renaming, setRenaming] = useState<{ id: string; name: string } | null>(null);
  const [name, setName] = useState('');

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const submitRename = async () => {
    if (!renaming) return;
    const trimmed = name.trim();
    if (!trimmed) {
      toast.error('标签名不能为空');
      return;
    }
    await rename(renaming.id, trimmed);
    toast.success('标签已重命名');
    setRenaming(null);
  };

  return (
    <div className="h-full w-full overflow-auto p-6">
      <div className="mx-auto w-full max-w-5xl space-y-4">
        <header>
          <h1 className="text-lg font-semibold text-[var(--ink)]">标签</h1>
          <p className="mt-1 text-xs text-[var(--ink-subtle)]">
            标签名称在同一工作区内唯一且不区分大小写；删除标签不会删除笔记正文。
          </p>
        </header>

        {tags.length === 0 ? (
          <EmptyState
            icon={TagIcon}
            title="还没有标签"
            description="在笔记编辑器中输入标签即可创建。"
          />
        ) : (
          <ul className="divide-y divide-[var(--border-subtle)] rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)]">
            {tags.map((tag) => (
              <li key={tag.id} className="flex items-center gap-3 px-3 py-2">
                <TagIcon className="h-4 w-4 text-[var(--ink-subtle)]" />
                <span className="min-w-0 flex-1 truncate text-sm text-[var(--ink)]">{tag.name}</span>
                <span className="text-[11px] text-[var(--ink-subtle)]">{tag.noteCount} 篇</span>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => navigate(`/notes?tagId=${tag.id}`)}
                >
                  查看笔记
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  title="重命名"
                  onClick={() => {
                    setRenaming({ id: tag.id, name: tag.name });
                    setName(tag.name);
                  }}
                >
                  <Pencil className="h-3.5 w-3.5" />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  title="删除标签"
                  onClick={() =>
                    void remove(tag.id).then((ok) =>
                      reportAction(ok, '标签已删除', useTagsStore.getState().error, '删除失败'),
                    )
                  }
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {renaming ? (
        <Dialog open onOpenChange={(open) => (!open ? setRenaming(null) : undefined)}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>重命名标签</DialogTitle>
            </DialogHeader>
            <Input
              autoFocus
              value={name}
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') void submitRename();
              }}
            />
            <DialogFooter>
              <Button variant="secondary" onClick={() => setRenaming(null)}>
                取消
              </Button>
              <Button onClick={() => void submitRename()}>保存</Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      ) : null}
    </div>
  );
}
