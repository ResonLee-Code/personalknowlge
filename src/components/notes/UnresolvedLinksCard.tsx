import { Plus, Unlink2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { Skeleton } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { ROUTES } from '@/lib/constants';
import { errorMessage } from '@/lib/format';
import { linkService } from '@/services/linkService';
import { useNotesStore } from '@/stores/notesStore';
import type { UnresolvedLink } from '@/types';

/** 首页「未解析链接」卡片：全库范围内指向不存在笔记的 `[[…]]`（M1-3 / F3）。 */
export function UnresolvedLinksCard({ limit = 5 }: { limit?: number }) {
  const navigate = useNavigate();
  const [links, setLinks] = useState<UnresolvedLink[] | null>(null);
  const [creating, setCreating] = useState<string | null>(null);
  const loadRef = useRef<() => Promise<void>>(async () => {});

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      const next = await linkService.unresolved().catch((error: unknown) => {
        console.warn('[links] 加载未解析链接失败', errorMessage(error));
        return [] as UnresolvedLink[];
      });
      if (!cancelled) setLinks(next);
    };
    loadRef.current = run;
    void run();
    return () => {
      cancelled = true;
    };
  }, []);

  const createNote = async (link: UnresolvedLink) => {
    setCreating(link.targetTitle);
    try {
      const created = await useNotesStore.getState().create({ title: link.targetTitle });
      if (!created) {
        toast.error('创建笔记失败');
        return;
      }
      toast.success(`已创建「${created.note.title}」并建立链接`);
      await loadRef.current();
    } catch (error) {
      toast.error(errorMessage(error));
    } finally {
      setCreating(null);
    }
  };

  const visible = (links ?? []).slice(0, limit);
  const hidden = (links?.length ?? 0) - visible.length;

  return (
    <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4">
      <div className="mb-2 flex items-center gap-2">
        <Unlink2 className="h-3.5 w-3.5 text-amber-600" />
        <h2 className="text-sm font-semibold text-[var(--ink)]">未解析链接</h2>
        {links && links.length > 0 ? (
          <span className="rounded-full bg-amber-500/10 px-2 py-0.5 text-[11px] text-amber-600">
            {links.length}
          </span>
        ) : null}
      </div>

      {links === null ? (
        <div className="space-y-2">
          <Skeleton className="h-8 w-full" />
          <Skeleton className="h-8 w-2/3" />
        </div>
      ) : links.length === 0 ? (
        <p className="text-xs text-[var(--ink-subtle)]">
          没有待处理的链接。写下 [[笔记标题]] 就能把想法连起来。
        </p>
      ) : (
        <>
          <ul className="space-y-1">
            {visible.map((link, index) => (
              <li
                key={`${link.sourceNoteId}-${link.targetTitle}-${index}`}
                className="flex items-center gap-2 rounded-md px-1 py-1"
              >
                <button
                  type="button"
                  className="min-w-0 flex-1 text-left"
                  title={link.context || link.sourceTitle}
                  onClick={() => navigate(ROUTES.note(link.sourceNoteId))}
                >
                  <span className="block truncate text-xs text-[var(--ink)]">
                    [[{link.targetTitle}]]
                  </span>
                  <span className="block truncate text-[11px] text-[var(--ink-subtle)]">
                    {link.sourceTitle}
                  </span>
                </button>
                <Button
                  size="sm"
                  variant="secondary"
                  aria-label={`创建笔记：${link.targetTitle}`}
                  disabled={creating === link.targetTitle}
                  onClick={() => void createNote(link)}
                >
                  <Plus className="h-3 w-3" />
                  创建
                </Button>
              </li>
            ))}
          </ul>
          {hidden > 0 ? (
            <p className="mt-2 text-[11px] text-[var(--ink-subtle)]">
              还有 {hidden} 处未解析链接，可在对应笔记的反向链接面板中处理。
            </p>
          ) : null}
        </>
      )}
    </div>
  );
}
