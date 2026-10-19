import { getCurrentWindow } from '@tauri-apps/api/window';
import { Maximize2, Minus, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { cn } from '@/lib/utils';
import { isTauriRuntime } from '@/services/api';

/**
 * 全屏时的窗口控制条（2026-10-19）。
 *
 * 背景：macOS 原生全屏会把标题栏与红黄绿按钮一起藏起来，正常情况下鼠标移到屏幕顶部
 * 应该把它们唤出来；但这台机器上（Codex 桌面端同样如此）鼠标扫过顶部不会出现，
 * 只有按 Esc 退出全屏才看得到。用户在正文里想最小化 / 关闭 / 退出全屏时没有入口。
 *
 * 做法：全屏时在窗口顶部放一条 **8px 的悬停感应带**，鼠标扫过就淡入一条 36px 高的
 * 窗口控制条（红黄绿三键 + 「全屏」提示），离开 600ms 后自动收起；按钮直接调用
 * Tauri 的窗口 API。非全屏时不渲染——原生标题栏本来就在。
 */

const HIDE_DELAY = 600;

export function FullscreenWindowControls() {
  const [fullscreen, setFullscreen] = useState(false);
  const [visible, setVisible] = useState(false);
  const hideTimer = useRef<number | null>(null);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    const win = getCurrentWindow();
    let disposed = false;
    let unlisten: (() => void) | undefined;

    const sync = () => {
      void win.isFullscreen().then((value) => {
        if (disposed) return;
        setFullscreen(value);
        // 退出全屏时顺手把浮层状态清掉；下次再进全屏从「隐藏」开始
        if (!value) setVisible(false);
      });
    };

    sync();
    void win
      .onResized(sync)
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch(() => {
        // 监听失败不影响主流程：仍可在下一次窗口尺寸变化时靠兜底同步
      });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(
    () => () => {
      if (hideTimer.current) window.clearTimeout(hideTimer.current);
    },
    [],
  );

  /**
   * 顶部 32px 的悬停感应（2026-10-19）。
   *
   * 不用一个透明的 fixed div 去接 mouseenter：那会挡住应用顶栏的点击（搜索、工作区名）。
   * 改成监听 document 的 mousemove，只在指针进入顶部区域时把标题栏显出来；
   * 指针在内容区时不做任何事。
   */
  useEffect(() => {
    if (!fullscreen) return;
    const onMove = (event: MouseEvent) => {
      if (event.clientY > 32) return;
      if (hideTimer.current) {
        window.clearTimeout(hideTimer.current);
        hideTimer.current = null;
      }
      setVisible(true);
    };
    window.addEventListener('mousemove', onMove, { passive: true });
    return () => window.removeEventListener('mousemove', onMove);
  }, [fullscreen]);

  if (!fullscreen) return null;

  const cancelHide = () => {
    if (hideTimer.current) {
      window.clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
  };
  const scheduleHide = () => {
    cancelHide();
    hideTimer.current = window.setTimeout(() => setVisible(false), HIDE_DELAY);
  };
  const win = getCurrentWindow();

  return (
    <>
      <div
        data-testid="fullscreen-window-controls"
        role="toolbar"
        aria-label="全屏窗口控制"
        title="鼠标移到屏幕顶部可再次显示"
        className={cn(
          'fixed inset-x-0 top-0 z-[71] flex h-12 items-center px-5',
          'border-b border-[var(--border-subtle)] bg-[var(--surface-muted)] shadow-sm',
          'transition-opacity duration-150',
          visible ? 'opacity-100' : 'pointer-events-none opacity-0',
        )}
        onMouseEnter={cancelHide}
        onMouseLeave={scheduleHide}
      >
        {/* 三个圆点作为一组：鼠标落在任意一个上，三个图标一起出现（与 macOS 原生一致） */}
        <div className="group flex items-center gap-2">
          <button
            type="button"
            aria-label="关闭窗口"
            title="关闭"
            className="flex h-3 w-3 items-center justify-center rounded-full bg-[#ff5f57] text-black/55 ring-1 ring-black/10"
            onClick={() => void win.close()}
          >
            <X className="h-2 w-2 opacity-0 group-hover:opacity-100" />
          </button>
          <button
            type="button"
            aria-label="最小化窗口"
            title="最小化"
            className="flex h-3 w-3 items-center justify-center rounded-full bg-[#febc2e] text-black/55 ring-1 ring-black/10"
            onClick={() => void win.minimize()}
          >
            <Minus className="h-2 w-2 opacity-0 group-hover:opacity-100" />
          </button>
          <button
            type="button"
            aria-label="退出全屏"
            title="退出全屏"
            className="flex h-3 w-3 items-center justify-center rounded-full bg-[#28c840] text-black/55 ring-1 ring-black/10"
            onClick={() => void win.setFullscreen(false)}
          >
            <Maximize2 className="h-2 w-2 opacity-0 group-hover:opacity-100" />
          </button>
        </div>
        {/* 居中标题：模仿原生标题栏（附图2），不额外加「全屏」等标签 */}
        <span className="pointer-events-none absolute inset-x-0 text-center text-[13px] font-semibold text-[var(--ink)]">
          Personal Knowledge OS
        </span>
      </div>
    </>
  );
}
