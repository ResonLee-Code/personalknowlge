/**
 * 搜索历史与收藏的搜索条件（V1.1.2）。
 *
 * 纯逻辑与持久化分开：`pushRecent` / `sameSearch` 可在单测里直接验证；
 * localStorage 键按工作区分隔，换库不会串数据。
 */

export interface SearchCondition {
  keyword: string;
  categoryId: string | null;
  tagId: string | null;
  favoriteOnly: boolean;
}

export interface RecentSearch {
  condition: SearchCondition;
  at: string;
}

export interface SavedSearch extends SearchCondition {
  id: string;
  name: string;
  createdAt: string;
}

export function emptyCondition(): SearchCondition {
  return { keyword: '', categoryId: null, tagId: null, favoriteOnly: false };
}

/** 条件是否「有意义」（能作为历史或收藏保存）。 */
export function isMeaningfulSearch(condition: SearchCondition): boolean {
  return Boolean(
    condition.keyword.trim() || condition.categoryId || condition.tagId || condition.favoriteOnly,
  );
}

/** 两个条件是否等价（用于去重）。 */
export function sameSearch(left: SearchCondition, right: SearchCondition): boolean {
  return (
    left.keyword.trim() === right.keyword.trim() &&
    (left.categoryId ?? null) === (right.categoryId ?? null) &&
    (left.tagId ?? null) === (right.tagId ?? null) &&
    left.favoriteOnly === right.favoriteOnly
  );
}

/** 最近搜索：新的排在最前、去重、最多保留 max 条。 */
export function pushRecent(
  list: RecentSearch[],
  condition: SearchCondition,
  at: string,
  max = 8,
): RecentSearch[] {
  if (!isMeaningfulSearch(condition)) return list;
  const rest = list.filter((item) => !sameSearch(item.condition, condition));
  return [{ condition, at }, ...rest].slice(0, max);
}

const RECENT_PREFIX = 'pkos.search.recent.';
const SAVED_PREFIX = 'pkos.search.saved.';

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // localStorage 不可用时静默忽略（不影响搜索本身）
  }
}

export function loadRecent(workspaceId: string): RecentSearch[] {
  const list = readJson<RecentSearch[]>(`${RECENT_PREFIX}${workspaceId}`, []);
  return Array.isArray(list) ? list : [];
}

export function saveRecent(workspaceId: string, list: RecentSearch[]): void {
  writeJson(`${RECENT_PREFIX}${workspaceId}`, list);
}

export function loadSaved(workspaceId: string): SavedSearch[] {
  const list = readJson<SavedSearch[]>(`${SAVED_PREFIX}${workspaceId}`, []);
  return Array.isArray(list) ? list : [];
}

export function saveSaved(workspaceId: string, list: SavedSearch[]): void {
  writeJson(`${SAVED_PREFIX}${workspaceId}`, list);
}

/** 用 id 生成稳定但不重复的收藏标识。 */
export function newSavedId(): string {
  return `saved_${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

/** 条件 → 可读标签（用于 chip 文案）。 */
export function describeSearch(
  condition: SearchCondition,
  resolve: { categoryName?: string | null; tagName?: string | null },
): string {
  const parts: string[] = [];
  if (condition.keyword.trim()) parts.push(`“${condition.keyword.trim()}”`);
  if (resolve.categoryName) parts.push(`分类：${resolve.categoryName}`);
  if (resolve.tagName) parts.push(`标签：${resolve.tagName}`);
  if (condition.favoriteOnly) parts.push('仅收藏');
  return parts.join(' · ') || '全部笔记';
}
