import { splitHighlight } from '@/lib/highlight';
import type { TextSegment } from '@/types';

/** 渲染高亮片段；后端返回的摘要片段优先使用。 */
export function HighlightSegments({
  segments,
  fallbackText,
  keyword,
}: {
  segments?: TextSegment[];
  fallbackText?: string;
  keyword?: string;
}) {
  const resolved =
    segments && segments.length > 0
      ? segments
      : fallbackText
        ? splitHighlight(fallbackText, keyword ? [keyword] : [])
        : [];

  return (
    <>
      {resolved.map((segment, index) => (
        <span key={index} className={segment.matched ? 'search-highlight' : undefined}>
          {segment.text}
        </span>
      ))}
    </>
  );
}
