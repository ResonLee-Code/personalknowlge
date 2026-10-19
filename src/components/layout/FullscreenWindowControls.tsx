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
  const reveal = () => {
    cancelHide();
    setVisible(true);
  };
  const win = getCurrentWindow();

  return (
    <>
      <div
        data-testid="fullscreen-hover-sensor"
        aria-hidden
        className="fixed inset-x-0 top-0 z-[70] h-2.5"
        onMouseEnter={reveal}
        onMouseMove={reveal}
      />
      <div
        data-testid="fullscreen-window-controls"
        role="toolbar"
        aria-label="全屏窗口控制"
        title="鼠标移到屏幕顶部可再次显示"
        className={cn(
          'fixed left-3 top-1.5 z-[71] flex h-8 items-center gap-2 rounded-full',
          'border border-[var(--border-subtle)] bg-[var(--surface)] px-3 shadow-lg transition-opacity duration-150',
          visible ? 'opacity-100' : 'pointer-events-none opacity-0',
        )}
        onMouseEnter={cancelHide}
        onMouseLeave={scheduleHide}
      >
        <button
          type="button"
          aria-label="关闭窗口"
          title="关闭"
          className="group flex h-3.5 w-3.5 items-center justify-center rounded-full bg-[#ff5f57] text-black/55"
          onClick={() => void win.close()}
        >
          <X className="h-2.5 w-2.5 opacity-0 group-hover:opacity-100" />
        </button>
        <button
          type="button"
          aria-label="最小化窗口"
          title="最小化"
          className="group flex h-3.5 w-3.5 items-center justify-center rounded-full bg-[#febc2e] text-black/55"
          onClick={() => void win.minimize()}
        >
          <Minus className="h-2.5 w-2.5 opacity-0 group-hover:opacity-100" />
        </button>
        <button
          type="button"
          aria-label="退出全屏"
          title="退出全屏"
          className="group flex h-3.5 w-3.5 items-center justify-center rounded-full bg-[#28c840] text-black/55"
          onClick={() => void win.setFullscreen(false)}
        >
          <Maximize2 className="h-2.5 w-2.5 opacity-0 group-hover:opacity-100" />
        </button>
        <span className="ml-1 text-[10px] text-[var(--ink-subtle)]">全屏</span>
      </div>
    </>
  );
}
