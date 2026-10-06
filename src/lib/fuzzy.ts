/** 标题模糊匹配：按顺序包含查询字符即可（忽略大小写与空格）。 */
export function matchTitle(title: string, query: string): boolean {
  const needle = query.trim().toLowerCase().replace(/\s+/g, '');
  if (!needle) return true;
  const haystack = title.toLowerCase();
  if (haystack.includes(needle)) return true;
  let cursor = 0;
  for (const ch of haystack) {
    if (ch === needle[cursor]) cursor += 1;
    if (cursor === needle.length) return true;
  }
  return false;
}
