/**
 * 双向链接解析（V1.1 M1/M2）。
 *
 * 与 Rust 端 `core/text.rs` 的 `parse_wikilinks` 保持同一套规则：
 * `[[标题]]`、`[[标题|别名]]`、`[[标题#小节]]`、`\|` `\#` `\]` 转义，
 * 忽略围栏代码块与行内代码中的链接，不跨行。
 */

import { LINK_CONTEXT_CHARS, maskCodeRegions } from './markdown';

export interface WikiLinkToken {
  /** 链接目标（`[[标题|别名]]` 取「标题」） */
  target: string;
  alias: string | null;
  anchor: string | null;
  /** 命中行上下文摘要 */
  context: string;
  /** `[[` 在正文中的起始偏移 */
  start: number;
  /** 结束于 `]]` 之后的偏移 */
  end: number;
}

function splitUnescaped(value: string, separator: string): [string, string | null] {
  const chars = [...value];
  for (let index = 0; index < chars.length; index += 1) {
    if (chars[index] === '\\') {
      index += 1;
      continue;
    }
    if (chars[index] === separator) {
      return [chars.slice(0, index).join(''), chars.slice(index + 1).join('')];
    }
  }
  return [value, null];
}

function unescapeLinkText(value: string): string {
  return value.split('\\|').join('|').split('\\#').join('#').split('\\]').join(']');
}

function parseInner(
  inner: string,
): { target: string; alias: string | null; anchor: string | null } | null {
  const [targetPart, aliasPart] = splitUnescaped(inner, '|');
  const [titlePart, anchorPart] = splitUnescaped(targetPart, '#');
  const target = unescapeLinkText(titlePart.trim());
  if (!target) return null;
  return {
    target,
    alias: aliasPart ? unescapeLinkText(aliasPart.trim()) || null : null,
    anchor: anchorPart ? unescapeLinkText(anchorPart.trim()) || null : null,
  };
}

function findEnd(chars: string[], parsable: boolean[], start: number): number | null {
  let index = start;
  while (index < chars.length) {
    if (chars[index] === '\n') return null;
    if (chars[index] === ']' && chars[index + 1] === ']' && parsable[index]) return index;
    if (chars[index] === '[' && chars[index + 1] === '[') return null;
    index += 1;
  }
  return null;
}

/** 取链接所在行的上下文摘要（过长时以链接为中心截断）。 */
function lineContext(chars: string[], at: number): string {
  let start = 0;
  for (let index = at - 1; index >= 0; index -= 1) {
    if (chars[index] === '\n') {
      start = index + 1;
      break;
    }
  }
  let end = chars.length;
  for (let index = at; index < chars.length; index += 1) {
    if (chars[index] === '\n') {
      end = index;
      break;
    }
  }
  const line = chars.slice(start, end);
  if (line.length <= LINK_CONTEXT_CHARS) return line.join('').trim();

  const offset = at - start;
  const half = Math.floor(LINK_CONTEXT_CHARS / 2);
  let from = Math.max(0, offset - half);
  const to = Math.min(line.length, from + LINK_CONTEXT_CHARS);
  if (to - from < LINK_CONTEXT_CHARS) from = Math.max(0, to - LINK_CONTEXT_CHARS);
  const body = line.slice(from, to).join('').trim();
  return `${from > 0 ? '…' : ''}${body}${to < line.length ? '…' : ''}`;
}

/** 解析正文中的全部双向链接。 */
export function parseWikilinks(content: string): WikiLinkToken[] {
  const chars = [...content];
  const parsable = maskCodeRegions(content);
  const tokens: WikiLinkToken[] = [];
  let index = 0;

  while (index + 1 < chars.length) {
    if (parsable[index] && chars[index] === '[' && chars[index + 1] === '[') {
      const end = findEnd(chars, parsable, index + 2);
      if (end !== null) {
        const parsed = parseInner(chars.slice(index + 2, end).join(''));
        if (parsed) {
          tokens.push({
            ...parsed,
            context: lineContext(chars, index),
            start: index,
            end: end + 2,
          });
        }
        index = end + 2;
        continue;
      }
    }
    index += 1;
  }

  return tokens;
}

const HREF_PREFIX = '#wikilink/';

/** 生成预览里可点击的 chip 链接。 */
export function wikilinkHref(target: string, anchor?: string | null): string {
  const base = `${HREF_PREFIX}${encodeURIComponent(target)}`;
  return anchor ? `${base}#${encodeURIComponent(anchor)}` : base;
}

/** 从 chip 链接里解析出目标（与锚点）。 */
export function parseWikilinkHref(
  href?: string | null,
): { target: string; anchor: string | null } | null {
  if (!href || !href.startsWith(HREF_PREFIX)) return null;
  const rest = href.slice(HREF_PREFIX.length);
  const [rawTarget, rawAnchor] = rest.split('#', 2);
  try {
    const target = decodeURIComponent(rawTarget ?? '');
    if (!target) return null;
    return { target, anchor: rawAnchor ? decodeURIComponent(rawAnchor) : null };
  } catch {
    return null;
  }
}

/**
 * 把 `[[…]]` 转换为标准 Markdown 链接，交给预览渲染成 chip。
 * 代码块/行内代码中的链接不会被替换。
 */
export function preprocessWikilinks(content: string): string {
  const tokens = parseWikilinks(content);
  if (tokens.length === 0) return content;

  let output = '';
  let cursor = 0;
  for (const token of tokens) {
    output += content.slice(cursor, token.start);
    const label = (token.alias ?? token.target).replace(/[[\]]/g, '').trim() || token.target;
    output += `[${label}](${wikilinkHref(token.target, token.anchor)})`;
    cursor = token.end;
  }
  return output + content.slice(cursor);
}
