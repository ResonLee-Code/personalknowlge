export interface VirtualWindow {
  start: number;
  end: number;
  totalHeight: number;
  offsetTop: number;
}

/**
 * 固定行高虚拟窗口。检索结果最多每页 100 条，但 DOM 始终只渲染视口附近的一小段；
 * 行高由调用方与样式保持一致，避免滚动跳动。
 */
export function computeVirtualWindow(
  itemCount: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  overscan = 3,
): VirtualWindow {
  const count = Math.max(0, itemCount);
  const safeRow = Math.max(1, rowHeight);
  const safeViewport = Math.max(0, viewportHeight);
  const visible = Math.ceil(safeViewport / safeRow) + overscan * 2 + 1;
  const start =
    count <= visible
      ? 0
      : Math.min(count - 1, Math.max(0, Math.floor(Math.max(0, scrollTop) / safeRow) - overscan));
  const end = Math.min(count, start + visible);
  return {
    start,
    end,
    totalHeight: count * safeRow,
    offsetTop: start * safeRow,
  };
}
