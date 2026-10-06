import { useCallback, useEffect, useRef, useState } from 'react';

import { cn } from '@/lib/utils';

interface ResizeHandleProps {
  /** 把手所在位置：left 表示被调整的面板在左侧（向右拖动变大） */
  side: 'left' | 'right';
  /** 当前值（像素或比例，与 scale 配合） */
  value: number;
  min: number;
  max: number;
  onChange: (next: number) => void;
  /** 像素 → 数值 的换算系数，默认 1（数值即像素） */
  scale?: number;
  /** 拖动结束 / 键盘调整后触发，用于持久化 */
  onCommit?: () => void;
  /** 双击复位 */
  onReset?: () => void;
  label: string;
}

const KEYBOARD_STEP = 16;

/**
 * 可拖拽的分栏把手。
 *
 * 手感要点：
 * - `setPointerCapture` 接管指针，鼠标移出窗口或划过其它元素都不会丢事件；
 * - `requestAnimationFrame` 合并移动事件，每帧最多触发一次更新；
 * - 传入当前值与范围，由把手负责换算与夹取，避免「增量 / 绝对值」用错；
 * - 拖动结束才提交持久化；双击复位、方向键微调。
 */
export function ResizeHandle({
  side,
  value,
  min,
  max,
  onChange,
  scale = 1,
  onCommit,
  onReset,
  label,
}: ResizeHandleProps) {
  const [active, setActive] = useState(false);
  const dragging = useRef(false);
  const lastX = useRef(0);
  const pendingPx = useRef(0);
  const frame = useRef<number | null>(null);
  const latestValue = useRef(value);

  // 在副作用里同步最新值，避免在渲染期间读写 ref
  useEffect(() => {
    latestValue.current = value;
  }, [value]);

  const apply = useCallback(
    (deltaPx: number) => {
      if (deltaPx === 0) return;
      const direction = side === 'left' ? 1 : -1;
      const next = latestValue.current + direction * deltaPx * scale;
      onChange(Math.min(max, Math.max(min, next)));
    },
    [max, min, onChange, scale, side],
  );

  const flush = useCallback(() => {
    frame.current = null;
    if (pendingPx.current !== 0) {
      const delta = pendingPx.current;
      pendingPx.current = 0;
      apply(delta);
    }
  }, [apply]);

  const schedule = useCallback(
    (deltaPx: number) => {
      pendingPx.current += deltaPx;
      if (frame.current === null) {
        frame.current = window.requestAnimationFrame(flush);
      }
    },
    [flush],
  );

  const stop = useCallback(() => {
    if (!dragging.current) return;
    dragging.current = false;
    setActive(false);
    if (frame.current !== null) {
      window.cancelAnimationFrame(frame.current);
      frame.current = null;
    }
    if (pendingPx.current !== 0) {
      const delta = pendingPx.current;
      pendingPx.current = 0;
      apply(delta);
    }
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    onCommit?.();
  }, [apply, onCommit]);

  // 卸载时恢复全局样式，避免拖动中途关闭面板留下 col-resize 光标
  useEffect(() => {
    return () => {
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, []);

  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation="vertical"
      aria-valuenow={Math.round(value)}
      tabIndex={0}
      data-active={active ? '' : undefined}
      title={`${label}（拖动调整，双击复位）`}
      className={cn(
        'group relative z-10 flex w-2 shrink-0 cursor-col-resize items-center justify-center',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--brand)]',
      )}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        dragging.current = true;
        setActive(true);
        lastX.current = event.clientX;
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
      }}
      onPointerMove={(event) => {
        if (!dragging.current) return;
        const delta = event.clientX - lastX.current;
        lastX.current = event.clientX;
        schedule(delta);
      }}
      onPointerUp={stop}
      onPointerCancel={stop}
      onLostPointerCapture={stop}
      onDoubleClick={() => {
        onReset?.();
        onCommit?.();
      }}
      onKeyDown={(event) => {
        if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
        event.preventDefault();
        const direction = event.key === 'ArrowRight' ? 1 : -1;
        apply(direction * KEYBOARD_STEP);
        onCommit?.();
      }}
    >
      <span
        className={cn(
          'h-full w-px bg-[var(--border-subtle)] transition-colors',
          'group-hover:w-0.5 group-hover:bg-[var(--brand)]',
          'group-data-[active]:w-0.5 group-data-[active]:bg-[var(--brand)]',
        )}
      />
    </div>
  );
}
