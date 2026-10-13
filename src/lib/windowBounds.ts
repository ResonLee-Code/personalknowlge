// 窗口尺寸 / 位置的记忆（Track B4）。
//
// 为什么自己做而不加 `tauri-plugin-window-state`：这个功能只需要「存四个数 + 恢复时校验
// 一下还在不在屏幕上」，前端就能做完；加插件要多一个依赖、多一处版本面。
// 代价是跨显示器这类边界要自己兜住——所以把校验写成纯函数，用测试钉住。
//
// 单位一律用**物理像素**（outerPosition / outerSize / availableMonitors 的原始值），
// 避免逻辑像素与 DPI 缩放来回换算时把窗口越放越小。

export interface WindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface MonitorArea {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** 小于这个尺寸的窗口一律不恢复（多半是异常值，恢复出来没法用）。 */
export const MIN_RESTORABLE_WIDTH = 800;
export const MIN_RESTORABLE_HEIGHT = 560;
/** 与某块屏幕至少要有这么大的重叠，才认为「这个位置还在屏幕上」（够点到标题栏）。 */
const MIN_VISIBLE_OVERLAP = { width: 160, height: 80 };

function isFiniteInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 100_000;
}

/** 解析持久化的窗口尺寸；任何不合法都返回 null（宁可让系统摆放，也不要恢复到一个坏位置）。 */
export function parseWindowBounds(raw: string | null): WindowBounds | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<WindowBounds>;
    const { x, y, width, height } = parsed;
    if (!isFiniteInteger(x) || !isFiniteInteger(y)) return null;
    if (!isFiniteInteger(width) || !isFiniteInteger(height)) return null;
    if (width < MIN_RESTORABLE_WIDTH || height < MIN_RESTORABLE_HEIGHT) return null;
    return { x, y, width, height };
  } catch {
    return null;
  }
}

/** 序列化（取整：避免 0.3px 这种抖动在每次移动后都写下不同的值）。 */
export function serializeWindowBounds(bounds: WindowBounds): string {
  return JSON.stringify({
    x: Math.round(bounds.x),
    y: Math.round(bounds.y),
    width: Math.round(bounds.width),
    height: Math.round(bounds.height),
  });
}

/** 这个窗口位置是否还落在某块屏幕上（按重叠面积判断，而不是只看左上角）。 */
export function isOnSomeMonitor(bounds: WindowBounds, monitors: MonitorArea[]): boolean {
  return monitors.some((monitor) => {
    const overlapWidth =
      Math.min(bounds.x + bounds.width, monitor.x + monitor.width) - Math.max(bounds.x, monitor.x);
    const overlapHeight =
      Math.min(bounds.y + bounds.height, monitor.y + monitor.height) - Math.max(bounds.y, monitor.y);
    return overlapWidth >= MIN_VISIBLE_OVERLAP.width && overlapHeight >= MIN_VISIBLE_OVERLAP.height;
  });
}

export interface RestorePlan {
  size: [number, number];
  /** 位置不可见时为 null：只恢复尺寸，位置交给系统摆放。 */
  position: [number, number] | null;
}

// 决定「恢复什么」。
//
// 关键取舍：拔掉外接屏之后，上次的位置很可能已经在屏幕外——那就只恢复尺寸、
// 让系统去摆位置，否则用户会得到一个点不到的窗口。
export function planWindowRestore(
  bounds: WindowBounds | null,
  monitors: MonitorArea[],
): RestorePlan | null {
  if (!bounds) return null;
  const size: [number, number] = [bounds.width, bounds.height];
  // 拿不到显示器信息时不赌位置，只恢复尺寸
  if (monitors.length === 0) return { size, position: null };
  if (!isOnSomeMonitor(bounds, monitors)) return { size, position: null };
  return { size, position: [bounds.x, bounds.y] };
}
