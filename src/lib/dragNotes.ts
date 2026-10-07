/** 拖拽移动分类（V1.1.2）：笔记行 → 侧边栏分类 / Inbox 的统一数据交换格式。 */

export const NOTE_DRAG_TYPE = 'application/x-pkos-note';

/** 把待移动的笔记写入 drag 数据（多选时带上整批 id）。 */
export function setNoteDragData(
  transfer: DataTransfer,
  ids: string[],
  label: string,
): void {
  if (ids.length === 0) return;
  transfer.effectAllowed = 'move';
  transfer.setData(NOTE_DRAG_TYPE, JSON.stringify(ids));
  // 纯文本兜底：便于未来支持拖到外部应用（也方便调试）
  transfer.setData('text/plain', label);
}

/** 读取 drag 数据；不是笔记拖拽时返回空数组。 */
export function readNoteDragIds(transfer: DataTransfer | null): string[] {
  if (!transfer) return [];
  const raw = transfer.getData(NOTE_DRAG_TYPE);
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((item): item is string => typeof item === 'string' && item.length > 0);
  } catch {
    return [];
  }
}

/** 是否是笔记拖拽（用于 dragover 时决定是否接受）。 */
export function isNoteDrag(transfer: DataTransfer | null): boolean {
  if (!transfer) return false;
  if (transfer.types?.includes(NOTE_DRAG_TYPE)) return true;
  return Boolean(transfer.getData?.(NOTE_DRAG_TYPE));
}
