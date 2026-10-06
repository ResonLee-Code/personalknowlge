/**
 * Markdown 轻量解析工具（与 Rust 端 `core/text.rs` 保持同一套规则）。
 *
 * 目前服务于两处：
 * - 浏览器预览后端的双向链接解析（忽略代码区域）；
 * - 编辑器大纲（解析标题层级）。
 */

/** 链接上下文摘要的最大长度（与 Rust 端一致）。 */
export const LINK_CONTEXT_CHARS = 120;

/**
 * 标记不可解析的位置：围栏代码块（``` / ~~~，允许更长围栏与 info string）与行内代码。
 * 返回与字符一一对应的布尔数组，`true` 表示该位置可解析。
 */
export function maskCodeRegions(text: string): boolean[] {
  const chars = [...text];
  const parsable = chars.map(() => true);

  // 1) 围栏代码块
  let lineStart = 0;
  let inFence = false;
  let fenceChar = '`';
  let fenceLen = 0;
  for (let index = 0; index <= chars.length; index += 1) {
    if (index === chars.length || chars[index] === '\n') {
      const trimmed = chars.slice(lineStart, index).join('').trim();
      const marker = trimmed[0];
      const fenceRun = trimmed.match(/^[`~]+/)?.[0].length ?? 0;
      const isFence =
        fenceRun >= 3 &&
        (marker === '`' || marker === '~') &&
        [...trimmed].every(
          (ch) => ch === '`' || ch === '~' || /\s/.test(ch) || /[A-Za-z0-9]/.test(ch),
        );
      if (isFence) {
        if (!inFence) {
          inFence = true;
          fenceChar = marker;
          fenceLen = fenceRun;
        } else if (marker === fenceChar && fenceRun >= fenceLen) {
          inFence = false;
        }
        for (let offset = lineStart; offset < index; offset += 1) parsable[offset] = false;
      } else if (inFence) {
        for (let offset = lineStart; offset < index; offset += 1) parsable[offset] = false;
      }
      lineStart = index + 1;
    }
  }

  // 2) 行内代码（成对反引号，不跨行）
  let cursor = 0;
  while (cursor < chars.length) {
    if (chars[cursor] !== '`' || !parsable[cursor]) {
      cursor += 1;
      continue;
    }
    let run = 1;
    while (cursor + run < chars.length && chars[cursor + run] === '`' && parsable[cursor + run]) {
      run += 1;
    }
    let probe = cursor + run;
    let closing: number | null = null;
    while (probe < chars.length) {
      if (chars[probe] === '\n') break;
      if (chars[probe] === '`' && parsable[probe]) {
        let closeRun = 1;
        while (
          probe + closeRun < chars.length &&
          chars[probe + closeRun] === '`' &&
          parsable[probe + closeRun]
        ) {
          closeRun += 1;
        }
        if (closeRun === run) {
          closing = probe;
          break;
        }
        probe += closeRun;
        continue;
      }
      probe += 1;
    }
    if (closing === null) {
      cursor += run;
    } else {
      for (let offset = cursor; offset < closing + run; offset += 1) parsable[offset] = false;
      cursor = closing + run;
    }
  }

  return parsable;
}

/** 大纲条目。 */
export interface OutlineHeading {
  /** 1–6 */
  level: number;
  text: string;
  /** 标题行首在正文中的字符偏移（用于编辑器滚动定位） */
  offset: number;
}

/**
 * 解析 `#` 标题层级，忽略代码块中的 `#`。
 * 与预览渲染保持一致：要求 `#` 与标题之间至少有一个空格。
 */
export function parseOutline(content: string): OutlineHeading[] {
  if (!content.trim()) return [];
  const chars = [...content];
  const parsable = maskCodeRegions(content);
  const headings: OutlineHeading[] = [];
  let lineStart = 0;

  for (let index = 0; index <= chars.length; index += 1) {
    if (index !== chars.length && chars[index] !== '\n') continue;
    const line = chars.slice(lineStart, index).join('');
    if (parsable[lineStart]) {
      const match = /^(#{1,6})[ \t]+(.+?)[ \t]*#*[ \t]*$/.exec(line.trim());
      if (match) {
        const marker = match[1] ?? '';
        const text = (match[2] ?? '').trim();
        if (marker && text) {
          headings.push({ level: marker.length, text, offset: lineStart });
        }
      }
    }
    lineStart = index + 1;
  }

  return headings;
}
