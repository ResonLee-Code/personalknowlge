import type { TextSegment } from '@/types';

/**
 * 把一段文本按关键词切分为高亮片段。
 * 用于标题高亮（摘要高亮由后端直接返回片段）。
 */
export function splitHighlight(text: string, terms: string[]): TextSegment[] {
  const cleaned = terms.map((term) => term.trim()).filter((term) => term.length > 0);
  if (cleaned.length === 0) return [{ text, matched: false }];

  const lowerText = text.toLowerCase();
  const ranges: Array<{ start: number; end: number }> = [];

  for (const term of cleaned) {
    const lowerTerm = term.toLowerCase();
    let index = lowerText.indexOf(lowerTerm);
    while (index !== -1) {
      ranges.push({ start: index, end: index + term.length });
      index = lowerText.indexOf(lowerTerm, index + Math.max(term.length, 1));
    }
  }

  if (ranges.length === 0) return [{ text, matched: false }];

  ranges.sort((a, b) => a.start - b.start);
  const merged: Array<{ start: number; end: number }> = [];
  for (const range of ranges) {
    const last = merged[merged.length - 1];
    if (last && range.start <= last.end) {
      last.end = Math.max(last.end, range.end);
    } else {
      merged.push({ ...range });
    }
  }

  const segments: TextSegment[] = [];
  let cursor = 0;
  for (const range of merged) {
    if (range.start > cursor) {
      segments.push({ text: text.slice(cursor, range.start), matched: false });
    }
    segments.push({ text: text.slice(range.start, range.end), matched: true });
    cursor = range.end;
  }
  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor), matched: false });
  }
  return segments;
}

/** 关键词是否出现在文本中（大小写不敏感）。 */
export function containsKeyword(text: string, keyword: string): boolean {
  const trimmed = keyword.trim();
  if (!trimmed) return false;
  return text.toLowerCase().includes(trimmed.toLowerCase());
}

/** 从 Markdown 中提取纯文本预览（用于列表摘要兜底）。 */
export function markdownPreview(markdown: string, max = 120): string {
  const plain = markdown
    .replace(/^---[\s\S]*?---\n?/, '')
    .replace(/```[\s\S]*?```/g, ' ')
    .replace(/!\[[^\]]*\]\([^)]*\)/g, ' ')
    .replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
    .replace(/^\s{0,3}#{1,6}\s*/gm, '')
    .replace(/^\s*[-*+]\s+/gm, '')
    .replace(/[`*_>~]/g, '')
    .replace(/\s+/g, ' ')
    .trim();
  return plain.length > max ? `${plain.slice(0, max)}…` : plain;
}
