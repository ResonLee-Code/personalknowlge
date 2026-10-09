import { ListTree } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Skeleton } from '@/components/ui/badge';
import { ROUTES } from '@/lib/constants';
import { errorMessage } from '@/lib/format';
import { parseOutline, type OutlineHeading } from '@/lib/markdown';
import { noteService } from '@/services/noteService';
import type { NoteSummary } from '@/types';

/**
 * 大纲目录预览（对标语雀右侧「大纲」面板）：
 * 在笔记列表里不打开文档即可查看结构；点击标题会打开该笔记并跳转到对应位置。
 */
export function NoteOutlineDialog({
  note,
  onClose,
}: {
  note: NoteSummary;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const [headings, setHeadings] = useState<OutlineHeading[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void noteService
      .get(note.id)
      .then((detail) => {
        if (!cancelled) setHeadings(parseOutline(detail.content));
      })
      .catch((cause) => {
        if (!cancelled) setError(errorMessage(cause));
      });
    return () => {
      cancelled = true;
    };
  }, [note.id]);

  const jump = (offset: number) => {
    onClose();
    // `h` 参数由编辑器消费：打开笔记后滚动到该标题（见 NoteEditorPage）
    navigate(`${ROUTES.note(note.id)}?h=${offset}`);
  };

  return (
    <Dialog open onOpenChange={(open) => (!open ? onClose() : undefined)}>
      <DialogContent className="w-[min(520px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-1.5">
            <ListTree className="h-4 w-4 text-[var(--brand)]" />
            大纲
          </DialogTitle>
          <DialogDescription className="truncate">{note.title}</DialogDescription>
        </DialogHeader>

        <div className="max-h-[60vh] min-h-[120px] overflow-auto rounded-md border border-[var(--border-subtle)] p-2">
          {error ? (
            <p className="px-1 py-2 text-xs text-red-500">{error}</p>
          ) : headings === null ? (
            <div className="space-y-1.5 p-1">
              {[0, 1, 2, 3].map((index) => (
                <Skeleton key={index} className="h-4" style={{ width: `${80 - index * 12}%` }} />
              ))}
            </div>
          ) : headings.length === 0 ? (
            <p className="px-1 py-2 text-xs leading-relaxed text-[var(--ink-subtle)]">
              这篇笔记还没有标题。用 <code># 标题</code> 建分层后，这里会自动生成大纲目录。
            </p>
          ) : (
            <ul className="space-y-0.5">
              {headings.map((heading, index) => (
                <li key={`${heading.offset}-${index}`}>
                  <button
                    type="button"
                    data-testid="outline-preview-item"
                    title={heading.text}
                    onClick={() => jump(heading.offset)}
                    className="block w-full truncate rounded px-1.5 py-1 text-left text-xs text-[var(--ink-muted)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--brand)]"
                    style={{ paddingLeft: `${(heading.level - 1) * 12 + 6}px` }}
                  >
                    {heading.text}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
        <p className="mt-1 text-[11px] text-[var(--ink-subtle)]">
          点击大纲条目可直接打开笔记并跳转到该标题；共 {headings?.length ?? 0} 个标题。
        </p>
      </DialogContent>
    </Dialog>
  );
}
