import type { NoteSummary } from '@/types';

export interface NoteTreeRow {
  note: NoteSummary;
  /** 层级深度（0 = 顶层文档） */
  depth: number;
  /** 直接子文档数量，> 0 时列表显示展开 / 折叠箭头 */
  childCount: number;
}

/**
 * 把平铺的笔记摘要组装成「父文档 → 子文档」目录树（对标语雀的文档目录）。
 *
 * - 子文档的相对顺序跟随输入顺序（即当前排序结果）；
 * - 父文档不在当前结果里时（例如按分类 / 标签筛选掉了父文档，或父文档已删除），
 *   子文档按顶层展示，不会从列表里消失；
 * - 用 `visited` 兜住 frontmatter 手写出来的环，避免死循环。
 */
export function buildNoteTree(items: NoteSummary[], collapsed: Set<string> = new Set()): NoteTreeRow[] {
  const byId = new Map(items.map((item) => [item.id, item]));
  const childrenOf = new Map<string, NoteSummary[]>();
  const roots: NoteSummary[] = [];

  for (const item of items) {
    const parentId = item.parentId ?? null;
    if (parentId && parentId !== item.id && byId.has(parentId)) {
      const bucket = childrenOf.get(parentId);
      if (bucket) bucket.push(item);
      else childrenOf.set(parentId, [item]);
    } else {
      roots.push(item);
    }
  }

  const rows: NoteTreeRow[] = [];
  // 先算一遍「从根可达」的集合（不做折叠），用于识别只可能来自环的笔记
  const reachable = new Set<string>();
  const mark = (note: NoteSummary) => {
    if (reachable.has(note.id)) return;
    reachable.add(note.id);
    (childrenOf.get(note.id) ?? []).forEach(mark);
  };
  roots.forEach(mark);

  const visited = new Set<string>();
  const walk = (note: NoteSummary, depth: number) => {
    if (visited.has(note.id)) return;
    visited.add(note.id);
    const children = childrenOf.get(note.id) ?? [];
    rows.push({ note, depth, childCount: children.length });
    if (collapsed.has(note.id)) return;
    children.forEach((child) => walk(child, depth + 1));
  };

  roots.forEach((note) => walk(note, 0));
  // 环里的笔记（从根不可达）也按顶层展示出来，避免从列表里消失
  items.forEach((note) => {
    if (!reachable.has(note.id)) walk(note, 0);
  });
  return rows;
}
