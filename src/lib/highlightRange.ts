/**
 * 码点区间 → UTF-16 下标（N5）。
 *
 * 后端（Rust）给的区间是**码点（Unicode 标量值）**下标；而 JS 字符串下标、CodeMirror
 * 的 position 都是 **UTF-16 码元**下标。BMP 之外的字（emoji 等）一个码点占两个码元，
 * 直接用会把高亮位置算歪——所以这里做一次显式换算，并有单测钉住。
 */

/** 把「第 N 个码点」换算成 JS 字符串下标（越界时夹到字符串长度）。 */
export function codepointToUtf16(text: string, codepointIndex: number): number {
  if (codepointIndex <= 0) return 0;
  let seen = 0;
  let offset = 0;
  for (const char of text) {
    if (seen >= codepointIndex) break;
    seen += 1;
    offset += char.length;
  }
  return offset;
}

/** 把码点区间换算成 UTF-16 区间（`to` 不小于 `from`）。 */
export function codepointRangeToUtf16(
  text: string,
  start: number,
  end: number,
): { from: number; to: number } {
  const from = codepointToUtf16(text, start);
  const to = Math.max(from, codepointToUtf16(text, end));
  return { from, to };
}
