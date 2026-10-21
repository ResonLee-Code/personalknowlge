import { useMemo, useRef, useState } from 'react';

import { SearchResultItem } from '@/components/search/SearchResultItem';
import { computeVirtualWindow } from '@/lib/virtualWindow';
import type { SearchHit } from '@/types';

const ROW_HEIGHT = 126;
const OVERSCAN = 3;

interface VirtualSearchResultsProps {
  hits: SearchHit[];
  keyword: string;
}

/** 固定行高检索结果虚拟列表：100 条结果只挂载视口附近的节点。 */
export function VirtualSearchResults({ hits, keyword }: VirtualSearchResultsProps) {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(640);
  const window = useMemo(
    () => computeVirtualWindow(hits.length, scrollTop, viewportHeight, ROW_HEIGHT, OVERSCAN),
    [hits.length, scrollTop, viewportHeight],
  );
  const visible = hits.slice(window.start, window.end);

  return (
    <div
      ref={(node) => {
        viewportRef.current = node;
        const height = node?.clientHeight ?? 0;
        if (height > 0 && height !== viewportHeight) setViewportHeight(height);
      }}
      className="h-full min-h-0 overflow-auto"
      data-testid="search-virtual-list"
      data-rendered-count={visible.length}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <div className="relative" style={{ height: window.totalHeight }}>
        <ul>
          {visible.map((hit, offset) => (
            <li
              key={hit.note.id}
              className="absolute left-0 right-0 overflow-hidden"
              style={{
                top: (window.start + offset) * ROW_HEIGHT,
                height: ROW_HEIGHT - 6,
              }}
            >
              <SearchResultItem hit={hit} keyword={keyword} />
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
