import { ChevronDown, ChevronRight, ListTree } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { errorMessage } from '@/lib/format';
import { parseOutline, type OutlineHeading } from '@/lib/markdown';
import { ROUTES } from '@/lib/constants';
import { cn } from '@/lib/utils';
import { noteService } from '@/services/noteService';
import { useNotesStore } from '@/stores/notesStore';

/**
 * 列表侧常驻大纲（V1.2.1 界面调整，对标语雀右侧「大纲」）。
 *
 * 与「大纲预览」弹窗的区别：**不需要右键菜单**，选中哪篇就看哪篇；
 * 点击条目会打开笔记并跳到对应标题（URL 带 `?h=<偏移量>`，编辑器消费后清除）。
 * 内容按笔记 id 缓存，来回切换不会重复读取。
 */
export function ListOutlinePane() {
  const navigate = useNavigate();
  const selectedId = useNotesStore((state) => state.selectedId);
  const [open, setOpen] = useState(true);
  /** 已解析过的笔记结果（按 id 缓存，切换来回不重复读文件） */
  const [cache, setCache] = useState<Record<string, OutlineHeading[]>>({});
  const [failure, setFailure] = useState<{ id: string; message: string } | null>(null);

  // 缓存即状态：渲染期只读 state，effect 里做异步读取
  const headings = selectedId ? cache[selectedId] ?? null : null;
  const error = selectedId && failure?.id === selectedId ? failure.message : null;

  useEffect(() => {
    if (!selectedId || cache[selectedId]) return;
    let cancelled = false;
    void noteService
      .get(selectedId)
      .then((detail) => {
        const parsed = parseOutline(detail.content);
        if (!cancelled) setCache((previous) => ({ ...previous, [selectedId]: parsed }));
      })
      .catch((cause: unknown) => {
        if (cancelled) return;
        setFailure({ id: selectedId, message: errorMessage(cause) });
        setCache((previous) => ({ ...previous, [selectedId]: [] }));
      });
    return () => {
      cancelled = true;
    };
  }, [selectedId, cache]);

  const jump = (offset: number) => {
    if (!selectedId) return;
    navigate(`${ROUTES.note(selectedId)}?h=${offset}`);
  };

  return (
    <section
      data-testid="list-outline-pane"
      className="shrink-0 border-t border-[var(--border-subtle)]"
    >
      <button
        type="button"
        data-testid="list-outline-toggle"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-[11px] font-medium text-[var(--ink)] transition-colors hover:bg-[var(--surface-muted)]"
      >
        {open ? <ChevronDown className="h-3 w-3" /> : <ChevronRight className="h-3 w-3" />}
        <ListTree className="h-3.5 w-3.5 text-[var(--brand)]" />
        大纲
        {headings ? (
          <span className="ml-auto text-[10px] text-[var(--ink-subtle)]">{headings.length}</span>
        ) : null}
      </button>

      {open ? (
        <div className="max-h-[38vh] overflow-auto px-1.5 pb-2">
          {!selectedId ? (
            <p className="px-1.5 py-1 text-[11px] leading-relaxed text-[var(--ink-subtle)]">
              选中列表里的笔记，这里会显示它的标题大纲；点击标题可直接跳转。
            </p>
          ) : error ? (
            <p className="px-1.5 py-1 text-[11px] text-red-500">{error}</p>
          ) : headings === null ? (
            <p className="px-1.5 py-1 text-[11px] text-[var(--ink-subtle)]">正在读取大纲…</p>
          ) : headings.length === 0 ? (
            <p className="px-1.5 py-1 text-[11px] leading-relaxed text-[var(--ink-subtle)]">
              这篇笔记还没有标题。用 <code># 标题</code> 建分层后，这里会自动生成大纲。
            </p>
          ) : (
            <ul className="space-y-0.5">
              {headings.map((heading, index) => (
                <li key={`${heading.offset}-${index}`}>
                  <button
                    type="button"
                    data-testid="list-outline-item"
                    title={heading.text}
                    onClick={() => jump(heading.offset)}
                    className={cn(
                      'block w-full truncate rounded px-1.5 py-1 text-left text-[11px] text-[var(--ink-muted)]',
                      'transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--brand)]',
                    )}
                    // 按层级缩进：H1 无缩进，H6 缩进 25px
                    style={{ paddingLeft: `${(heading.level - 1) * 10 + 6}px` }}
                  >
                    {heading.text}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </div>
      ) : null}
    </section>
  );
}
