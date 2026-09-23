import { FileText } from 'lucide-react';
import { useNavigate } from 'react-router-dom';

import { HighlightSegments } from '@/components/search/HighlightSegments';
import { formatRelative } from '@/lib/format';
import { splitHighlight } from '@/lib/highlight';
import { cn } from '@/lib/utils';
import type { SearchHit } from '@/types';

export function SearchResultItem({
  hit,
  keyword,
  active,
}: {
  hit: SearchHit;
  keyword: string;
  active?: boolean;
}) {
  const navigate = useNavigate();
  const titleSegments = splitHighlight(hit.note.title, hit.matchedTerms.length > 0 ? hit.matchedTerms : [keyword]);

  return (
    <button
      type="button"
      onClick={() => navigate(`/notes/${hit.note.id}`)}
      className={cn(
        'flex w-full flex-col gap-1 rounded-lg border px-3 py-2 text-left transition-colors',
        active
          ? 'border-[var(--brand)]/40 bg-[var(--brand-soft)]'
          : 'border-transparent hover:bg-[var(--surface-muted)]',
      )}
    >
      <span className="flex items-center gap-2">
        <FileText className="h-3.5 w-3.5 shrink-0 text-[var(--ink-subtle)]" />
        <span className="min-w-0 flex-1 truncate text-sm font-medium text-[var(--ink)]">
          {titleSegments.map((segment, index) => (
            <span key={index} className={segment.matched ? 'search-highlight' : undefined}>
              {segment.text}
            </span>
          ))}
        </span>
        {hit.titleMatched ? (
          <span className="shrink-0 rounded-full bg-[var(--brand-soft)] px-1.5 py-0.5 text-[10px] text-[var(--brand)]">
            标题命中
          </span>
        ) : null}
      </span>
      {hit.snippet.length > 0 ? (
        <span className="line-clamp-2 text-xs leading-relaxed text-[var(--ink-muted)]">
          <HighlightSegments segments={hit.snippet} />
        </span>
      ) : null}
      <span className="flex flex-wrap items-center gap-2 text-[11px] text-[var(--ink-subtle)]">
        <span>{hit.note.categoryPath || 'Inbox'}</span>
        <span>· {formatRelative(hit.note.updatedAt)}</span>
        {hit.note.tags.map((tag) => (
          <span key={tag} className="rounded-full bg-[var(--surface-muted)] px-1.5 py-px">
            {tag}
          </span>
        ))}
      </span>
    </button>
  );
}
