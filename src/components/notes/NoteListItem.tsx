import { MoreHorizontal, Star, StarOff, Trash2, Undo2 } from 'lucide-react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { formatRelative } from '@/lib/format';
import { splitHighlight } from '@/lib/highlight';
import { cn } from '@/lib/utils';
import { useNotesStore } from '@/stores/notesStore';
import type { NoteSummary } from '@/types';

export function NoteListItem({
  note,
  selected,
  basePath,
  onOpen,
}: {
  note: NoteSummary;
  selected: boolean;
  basePath: string;
  onOpen: () => void;
}) {
  const navigate = useNavigate();
  const keyword = useNotesStore((state) => state.keyword);
  const toggleFavorite = useNotesStore((state) => state.toggleFavorite);
  const remove = useNotesStore((state) => state.remove);
  const restore = useNotesStore((state) => state.restore);
  const purge = useNotesStore((state) => state.purge);
  const duplicate = useNotesStore((state) => state.duplicate);

  const titleSegments = keyword ? splitHighlight(note.title, [keyword]) : [{ text: note.title, matched: false }];
  const snippet = note.snippet ?? [];

  return (
    <li>
      <div
        role="button"
        tabIndex={0}
        onClick={onOpen}
        onKeyDown={(event) => {
          if (event.key === 'Enter') onOpen();
        }}
        className={cn(
          'group cursor-pointer rounded-lg border border-transparent px-2.5 py-2 transition-colors',
          selected
            ? 'border-[var(--brand)]/40 bg-[var(--brand-soft)]'
            : 'hover:bg-[var(--surface-muted)]',
        )}
      >
        <div className="flex items-start gap-1.5">
          <span className="min-w-0 flex-1 truncate text-sm font-medium text-[var(--ink)]">
            {titleSegments.map((segment, index) => (
              <span key={index} className={segment.matched ? 'search-highlight' : undefined}>
                {segment.text}
              </span>
            ))}
          </span>
          {note.favorite ? <Star className="mt-0.5 h-3.5 w-3.5 shrink-0 text-amber-500" /> : null}
          <DropdownMenu>
            <DropdownMenuTrigger asChild>
              <button
                type="button"
                aria-label="笔记操作"
                onClick={(event) => event.stopPropagation()}
                className="rounded p-0.5 text-[var(--ink-subtle)] opacity-0 transition-opacity group-hover:opacity-100 data-[state=open]:opacity-100"
              >
                <MoreHorizontal className="h-3.5 w-3.5" />
              </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" onClick={(event) => event.stopPropagation()}>
              <DropdownMenuItem onSelect={() => onOpen()}>打开</DropdownMenuItem>
              <DropdownMenuItem
                onSelect={() =>
                  void toggleFavorite(note.id).then(() =>
                    toast.success(note.favorite ? '已取消收藏' : '已收藏'),
                  )
                }
              >
                {note.favorite ? (
                  <>
                    <StarOff className="h-3.5 w-3.5" /> 取消收藏
                  </>
                ) : (
                  <>
                    <Star className="h-3.5 w-3.5" /> 收藏
                  </>
                )}
              </DropdownMenuItem>
              <DropdownMenuItem
                onSelect={() =>
                  void duplicate(note.id).then(() => toast.success('已创建副本'))
                }
              >
                创建副本
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              {note.status === 'deleted' ? (
                <>
                  <DropdownMenuItem onSelect={() => void restore(note.id)}>
                    <Undo2 className="h-3.5 w-3.5" /> 恢复
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    destructive
                    onSelect={() => void purge(note.id).then(() => toast.success('已永久删除'))}
                  >
                    <Trash2 className="h-3.5 w-3.5" /> 永久删除
                  </DropdownMenuItem>
                </>
              ) : (
                <DropdownMenuItem
                  destructive
                  onSelect={() => {
                    void remove(note.id).then(() => {
                      toast.success('已移入回收站');
                      if (basePath !== '/trash') navigate(basePath);
                    });
                  }}
                >
                  <Trash2 className="h-3.5 w-3.5" /> 移到回收站
                </DropdownMenuItem>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>

        {snippet.length > 0 ? (
          <p className="mt-1 line-clamp-2 text-xs leading-relaxed text-[var(--ink-muted)]">
            {snippet.map((segment, index) => (
              <span key={index} className={segment.matched ? 'search-highlight' : undefined}>
                {segment.text}
              </span>
            ))}
          </p>
        ) : null}

        <div className="mt-1.5 flex flex-wrap items-center gap-1.5 text-[11px] text-[var(--ink-subtle)]">
          <span>{formatRelative(note.updatedAt)}</span>
          {note.categoryPath ? <span>· {note.categoryPath}</span> : null}
          {note.tags.slice(0, 3).map((tag) => (
            <span key={tag} className="rounded-full bg-[var(--surface-muted)] px-1.5 py-px">
              {tag}
            </span>
          ))}
          {note.tags.length > 3 ? <span>+{note.tags.length - 3}</span> : null}
        </div>
      </div>
    </li>
  );
}
