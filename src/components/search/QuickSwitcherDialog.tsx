import { CornerDownLeft, FileText, Loader2 } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { ROUTES } from '@/lib/constants';
import { matchTitle } from '@/lib/fuzzy';
import { formatRelative } from '@/lib/format';
import { noteService } from '@/services/noteService';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';
import type { NoteSummary } from '@/types';

/**
 * 快速切换笔记（V1.1 M2-1，⌘/Ctrl + P）。
 *
 * 与全局搜索（⌘K，检索正文）分工：这里只按标题做模糊匹配，用于「我想打开某一篇」的场景。
 */
export function QuickSwitcherDialog() {
  const open = useUiStore((state) => state.quickSwitcherOpen);
  const setOpen = useUiStore((state) => state.setQuickSwitcherOpen);
  // 打开时挂载、关闭即销毁，因此无需在 effect 中重置状态
  if (!open) return null;
  return <QuickSwitcherPanel onClose={() => setOpen(false)} />;
}

function QuickSwitcherPanel({ onClose }: { onClose: () => void }) {
  const navigate = useNavigate();
  const refreshScope = useNotesStore((state) => state.refresh);
  const [keyword, setKeyword] = useState('');
  const [notes, setNotes] = useState<NoteSummary[] | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);

  useEffect(() => {
    let cancelled = false;
    const run = async () => {
      const result = await noteService
        .list({ status: 'active', sort: 'titleAsc', pageSize: 500 })
        .catch(() => null);
      if (!cancelled) setNotes(result?.items ?? []);
    };
    void run();
    return () => {
      cancelled = true;
    };
  }, []);

  const results = useMemo(() => {
    const source = notes ?? [];
    return source.filter((note) => matchTitle(note.title, keyword)).slice(0, 50);
  }, [notes, keyword]);

  const openNote = (note: NoteSummary) => {
    onClose();
    void refreshScope();
    navigate(ROUTES.note(note.id));
  };

  const target = results[Math.min(activeIndex, Math.max(results.length - 1, 0))];

  return (
    <Dialog open onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent className="w-[min(620px,calc(100vw-2rem))] p-0">
        <DialogTitle className="sr-only">快速切换笔记</DialogTitle>
        <div className="flex items-center gap-2 border-b border-[var(--border-subtle)] px-3 py-2.5">
          <FileText className="h-4 w-4 text-[var(--ink-subtle)]" />
          <Input
            autoFocus
            value={keyword}
            placeholder="输入标题快速跳转…"
            aria-label="按标题跳转"
            className="h-8 min-w-0 flex-1 border-none px-0 text-sm focus-visible:ring-0"
            onChange={(event) => {
              setKeyword(event.target.value);
              setActiveIndex(0);
            }}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown') {
                event.preventDefault();
                setActiveIndex((index) => Math.min(index + 1, Math.max(results.length - 1, 0)));
              } else if (event.key === 'ArrowUp') {
                event.preventDefault();
                setActiveIndex((index) => Math.max(index - 1, 0));
              } else if (event.key === 'Enter') {
                event.preventDefault();
                if (target) openNote(target);
              }
            }}
          />
          {notes === null ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin text-[var(--ink-subtle)]" />
          ) : (
            <span className="text-[11px] text-[var(--ink-subtle)]">{results.length} 篇</span>
          )}
        </div>

        <div className="max-h-[60vh] min-h-[80px] overflow-auto p-2">
          {results.length === 0 ? (
            <EmptyState
              icon={FileText}
              title={notes === null ? '正在加载笔记…' : '没有匹配的标题'}
              description={
                notes === null ? undefined : '换个关键词试试；需要检索正文请使用 ⌘K 全局搜索。'
              }
            />
          ) : (
            <ul className="space-y-1">
              {results.map((note, index) => (
                <li key={note.id}>
                  <button
                    type="button"
                    data-testid="quick-switch-item"
                    onMouseEnter={() => setActiveIndex(index)}
                    onClick={() => openNote(note)}
                    className={[
                      'flex w-full flex-col items-start gap-0.5 rounded-md px-3 py-2 text-left transition-colors',
                      index === activeIndex
                        ? 'bg-[var(--brand-soft)]'
                        : 'hover:bg-[var(--surface-muted)]',
                    ].join(' ')}
                  >
                    <span className="truncate text-sm font-medium text-[var(--ink)]">
                      {note.title}
                    </span>
                    <span className="truncate text-[11px] text-[var(--ink-subtle)]">
                      {note.categoryPath || 'Inbox'} · {formatRelative(note.updatedAt)}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div className="flex items-center gap-3 border-t border-[var(--border-subtle)] px-3 py-2 text-[11px] text-[var(--ink-subtle)]">
          <span>↑↓ 选择</span>
          <span className="flex items-center gap-1">
            <CornerDownLeft className="h-3 w-3" /> 打开
          </span>
          <span className="ml-auto">⌘/Ctrl + P</span>
        </div>
      </DialogContent>
    </Dialog>
  );
}
