import { ListTree } from 'lucide-react';

import { cn } from '@/lib/utils';
import type { OutlineHeading } from '@/lib/markdown';

interface OutlinePanelProps {
  headings: OutlineHeading[];
  onJump: (offset: number) => void;
  /** 当前光标所在标题的偏移（用于高亮） */
  activeOffset?: number | null;
}

/** 编辑器大纲（V1.1 M2-1）：解析 `#` 层级，点击跳转到对应位置。 */
export function OutlinePanel({ headings, onJump, activeOffset }: OutlinePanelProps) {
  return (
    <aside
      data-testid="outline-panel"
      className="flex w-[190px] shrink-0 flex-col border-l border-[var(--border-subtle)] bg-[var(--surface)]"
    >
      <header className="flex items-center gap-1.5 px-3 py-2 text-xs font-medium text-[var(--ink)]">
        <ListTree className="h-3.5 w-3.5 text-[var(--brand)]" />
        大纲
        <span className="ml-auto text-[11px] text-[var(--ink-subtle)]">{headings.length}</span>
      </header>
      <div className="min-h-0 flex-1 overflow-auto px-1.5 pb-3">
        {headings.length === 0 ? (
          <p className="px-1.5 text-[11px] leading-relaxed text-[var(--ink-subtle)]">
            正文还没有标题。用 <code># 标题</code> 开始分层，这里会自动生成大纲。
          </p>
        ) : (
          <ul className="space-y-0.5">
            {headings.map((heading, index) => {
              const active = activeOffset === heading.offset;
              return (
                <li key={`${heading.offset}-${index}`}>
                  <button
                    type="button"
                    data-testid="outline-item"
                    title={heading.text}
                    onClick={() => onJump(heading.offset)}
                    className={cn(
                      'block w-full truncate rounded px-1.5 py-1 text-left text-xs transition-colors',
                      active
                        ? 'bg-[var(--brand-soft)] text-[var(--brand)]'
                        : 'text-[var(--ink-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]',
                    )}
                    // 按层级缩进：H1 无缩进，H6 缩进 25px
                    style={{ paddingLeft: `${(heading.level - 1) * 10 + 6}px` }}
                  >
                    {heading.text}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </aside>
  );
}
