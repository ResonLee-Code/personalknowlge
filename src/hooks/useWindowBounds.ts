import { PhysicalPosition, PhysicalSize, availableMonitors, getCurrentWindow } from '@tauri-apps/api/window';
import { useEffect } from 'react';

import {
  parseWindowBounds,
  planWindowRestore,
  serializeWindowBounds,
  type MonitorArea,
} from '@/lib/windowBounds';
import { logAppFailure } from '@/lib/appLog';
import { isTauriRuntime } from '@/services/api';

/** 持久化键（放在 webview 自己的存储里：与安装绑定，不进工作区、不进备份）。 */
export const WINDOW_BOUNDS_KEY = 'pkos.windowBounds';
/** 拖动/缩放过程中的事件很密，防抖后再写。 */
const SAVE_DEBOUNCE_MS = 600;

/**
 * 记住窗口的尺寸与位置（Track B4）。
 *
 * 两个容易踩的点：
 * 1. **恢复期间不得回写**：`setSize` / `setPosition` 会触发 `onResized` / `onMoved`，
 *    如果那时也在保存，就会把「恢复动作本身」当成用户调整写回去（越存越偏）；
 * 2. **一律用物理像素**：`outerSize` / `availableMonitors` 都是物理值，
 *    混用逻辑像素会在高 DPI 屏上把窗口越恢复越小。
 */
export function useWindowBounds(): void {
  useEffect(() => {
    // 浏览器预览模式没有原生窗口，别动；真机上才接管
    if (!isTauriRuntime()) return;

    let disposed = false;
    let restoring = true;
    let timer: number | null = null;
    const unlisteners: Array<() => void> = [];

    const win = getCurrentWindow();

    const save = () => {
      if (restoring || disposed) return;
      void (async () => {
        try {
          const [position, size] = await Promise.all([win.outerPosition(), win.outerSize()]);
          localStorage.setItem(
            WINDOW_BOUNDS_KEY,
            serializeWindowBounds({
              x: position.x,
              y: position.y,
              width: size.width,
              height: size.height,
            }),
          );
        } catch {
          // 记不住就算了：这只是体验项，绝不该影响使用
        }
      })();
    };

    const scheduleSave = () => {
      if (restoring) return;
      if (timer !== null) window.clearTimeout(timer);
      timer = window.setTimeout(save, SAVE_DEBOUNCE_MS);
    };

    void (async () => {
      try {
        const stored = parseWindowBounds(localStorage.getItem(WINDOW_BOUNDS_KEY));
        if (!stored) {
          // 首次启动本来就没有记录：这种「正常缺失」用 info 级（默认可见但不刺眼）。
          // 注意这里**不能 return**：一旦提前退出，下面的监听就不会注册，
          // 于是「第一次记住位置」永远发生不了——也就永远没有记录可恢复（B4 死循环）。
          void logAppFailure('window_bounds', new Error('没有可用的窗口位置记录，按默认尺寸打开'), 'info');
        } else {
          const monitors: MonitorArea[] = (await availableMonitors().catch(() => [])).map(
            (monitor) => ({
              x: monitor.position.x,
              y: monitor.position.y,
              width: monitor.size.width,
              height: monitor.size.height,
            }),
          );
          const plan = planWindowRestore(stored, monitors);
          if (plan) {
            try {
              await win.setSize(new PhysicalSize(plan.size[0], plan.size[1]));
              if (plan.position) {
                await win.setPosition(new PhysicalPosition(plan.position[0], plan.position[1]));
              }
              void logAppFailure(
                'window_bounds',
                new Error(
                  `已恢复窗口：${plan.size.join('x')} @ ${plan.position ? plan.position.join(',') : '系统摆放'}（记录 ${JSON.stringify(stored)}，显示器 ${monitors.length} 块）`,
                ),
                'info',
              );
            } catch (error) {
              // 恢复失败不该影响使用，但必须留下原因
              void logAppFailure('window_bounds', error, 'warn');
            }
          }
        }
      } finally {
        restoring = false;
      }
      if (disposed) return;
      unlisteners.push(await win.onResized(scheduleSave));
      unlisteners.push(await win.onMoved(scheduleSave));
    })();

    return () => {
      disposed = true;
      if (timer !== null) window.clearTimeout(timer);
      unlisteners.forEach((fn) => fn());
    };
  }, []);
}
