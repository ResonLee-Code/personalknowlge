import type { NoteSummary } from '@/types';

/** 拖拽落点语义：插到目标前 / 后（同层排序），或成为目标的子文档 */
export type NoteDropPosition = 'before' | 'after' | 'inside';

export interface NoteDropPlan {
  /** 拖拽结束后所在层级：父文档 id，null 表示顶层 */
  parentId: string | null;
  /** 目标层级下的完整兄弟顺序（含被拖拽的笔记），用于写入 `order` */
  siblingIds: string[];
  /** 被拖拽笔记原来的父文档（多选时取第一篇） */
  fromParentId: string | null;
  /** 是否发生了跨层级移动（需要调用 note_set_parent） */
  changesParent: boolean;
}

/**
 * 落点在行内的位置 → 意图（对标语雀：中间是「成为子级」，上下边缘是「插到前后」）。
 * 上下各留 30% 作为插入区，中间 40% 作为「成为子文档」区域。
 */
export function noteDropPosition(offsetY: number, height: number): NoteDropPosition {
  if (!Number.isFinite(height) || height <= 0) return 'inside';
  const ratio = offsetY / height;
  if (ratio < 0.3) return 'before';
  if (ratio > 0.7) return 'after';
  return 'inside';
}

/** 目标是否是某个被拖拽笔记的后代（或者就是它自己）——用于拦截「拖进自己的子树」。 */
function isSelfOrDescendant(
  targetId: string,
  dragged: Set<string>,
  parentOf: (id: string) => string | null,
): boolean {
  let cursor: string | null = targetId;
  for (let hops = 0; cursor && hops < 64; hops += 1) {
    if (dragged.has(cursor)) return true;
    cursor = parentOf(cursor);
  }
  return false;
}

/**
 * 计算一次拖拽的落库方案：
 *
 * - `position = 'inside'`：被拖拽的笔记成为目标笔记的子文档（追加到其子文档末尾）；
 * - `position = 'before' | 'after'`：被拖拽的笔记插入目标笔记的同层前后；
 * - 拖到自己 / 自己的子树上 → 返回 null（界面据此显示禁止光标）。
 *
 * 纯函数：`items` 是当前查询结果（含被折叠隐藏的行），因此折叠不会影响顺序计算。
 */
export function planNoteDrop({
  items,
  targetId,
  draggedIds,
  position,
}: {
  items: NoteSummary[];
  targetId: string;
  draggedIds: string[];
  position: NoteDropPosition;
}): NoteDropPlan | null {
  const byId = new Map(items.map((item) => [item.id, item]));
  const target = byId.get(targetId);
  if (!target) return null;

  const parentOf = (id: string): string | null => byId.get(id)?.parentId ?? null;
  const moved = draggedIds.filter((id) => id !== targetId && byId.has(id));
  if (moved.length === 0) return null;
  // 不能把笔记拖进自己或自己的后代里（否则父子链成环）
  if (isSelfOrDescendant(targetId, new Set(moved), parentOf)) return null;

  const parentId = position === 'inside' ? target.id : target.parentId ?? null;
  const movedSet = new Set(moved);
  const siblings = items
    .filter((item) => (item.parentId ?? null) === parentId && !movedSet.has(item.id))
    .map((item) => item.id);

  if (position === 'inside') {
    siblings.push(...moved);
  } else {
    const targetIndex = siblings.indexOf(target.id);
    if (targetIndex === -1) return null;
    siblings.splice(position === 'before' ? targetIndex : targetIndex + 1, 0, ...moved);
  }

  const fromParentId = byId.get(moved[0]!)?.parentId ?? null;
  return {
    parentId,
    siblingIds: siblings,
    fromParentId,
    changesParent: moved.some((id) => (byId.get(id)?.parentId ?? null) !== parentId),
  };
}
