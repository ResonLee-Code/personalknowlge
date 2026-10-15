import { ChevronDown, ChevronRight, Plus, Tag as TagIcon } from 'lucide-react';
import { useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { reportFailure } from '@/lib/actionFeedback';
import { cn } from '@/lib/utils';
import { useTagsStore } from '@/stores/tagsStore';
import { useUiStore } from '@/stores/uiStore';

export function TagList() {
  const tags = useTagsStore((state) => state.tags);
  const create = useTagsStore((state) => state.create);
  const navigate = useNavigate();
  const [params] = useSearchParams();
  const activeId = params.get('tagId');
  const [open, setOpen] = useState(false);
  const [name, setName] = useState('');
  const tagsCollapsed = useUiStore((state) => state.tagsCollapsed);
  const toggleTagsCollapsed = useUiStore((state) => state.toggleTagsCollapsed);

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    const tag = await create(trimmed);
    if (tag) {
      toast.success('标签已创建');
      setOpen(false);
      setName('');
    } else {
      reportFailure('创建标签失败', useTagsStore.getState().error);
    }
  };

  return (
    <section className="space-y-1">
      <header className="flex items-center justify-between px-2">
        {/* 区块可折叠：与「分类」同一套交互（点标题折叠 / 展开） */}
        <button
          type="button"
          data-testid="tags-toggle"
          aria-expanded={!tagsCollapsed}
          aria-label={tagsCollapsed ? '展开标签' : '折叠标签'}
          title={tagsCollapsed ? '展开标签' : '折叠标签'}
          className="flex min-w-0 items-center gap-0.5 rounded text-[11px] font-semibold uppercase tracking-wide text-[var(--ink-subtle)] transition-colors hover:text-[var(--ink)]"
          onClick={toggleTagsCollapsed}
        >
          {tagsCollapsed ? (
            <ChevronRight className="h-3 w-3" />
          ) : (
            <ChevronDown className="h-3 w-3" />
          )}
          标签
        </button>
        <Button variant="ghost" size="icon" className="h-6 w-6" title="新建标签" onClick={() => setOpen(true)}>
          <Plus className="h-3.5 w-3.5" />
        </Button>
      </header>
      {tagsCollapsed ? null : tags.length === 0 ? (
        <p className="px-2 py-1 text-xs text-[var(--ink-subtle)]">暂无标签，可在笔记中添加。</p>
      ) : (
        <ul className="flex flex-wrap gap-1 px-1">
          {tags.map((tag) => (
            <li key={tag.id}>
              <button
                type="button"
                onClick={() => navigate(`/notes?tagId=${encodeURIComponent(tag.id)}`)}
                className={cn(
                  'flex items-center gap-1 rounded-full border px-2 py-0.5 text-xs transition-colors',
                  activeId === tag.id
                    ? 'border-[var(--brand)] bg-[var(--brand-soft)] text-[var(--brand)]'
                    : 'border-[var(--border-subtle)] text-[var(--ink-muted)] hover:border-[var(--brand)] hover:text-[var(--brand)]',
                )}
              >
                <TagIcon className="h-3 w-3" />
                {tag.name}
                <span className="tabular-nums text-[10px] text-[var(--ink-subtle)]">
                  {tag.noteCount}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>新建标签</DialogTitle>
          </DialogHeader>
          <Input
            aria-label="标签名称"
            autoFocus
            value={name}
            placeholder="标签名称"
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') void submit();
            }}
          />
          <DialogFooter>
            <Button variant="secondary" onClick={() => setOpen(false)}>
              取消
            </Button>
            <Button onClick={() => void submit()}>创建</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </section>
  );
}
