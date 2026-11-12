/** 把 `pkos://capture?...` 转换成内容发现路由；非法链接返回 null。 */
export function contentDiscoveryPathFromDeepLink(raw: string): string | null {
  try {
    const url = new URL(raw);
    if (url.protocol !== 'pkos:' || url.hostname !== 'capture') return null;
    const shared = url.searchParams.get('url') ?? url.searchParams.get('text') ?? '';
    const note = url.searchParams.get('note') ?? '';
    if (!shared.trim() && !note.trim()) return null;
    const params = new URLSearchParams();
    if (shared.trim()) params.set('url', shared.trim());
    if (note.trim()) params.set('note', note.trim());
    return `/discover?${params.toString()}`;
  } catch {
    return null;
  }
}
