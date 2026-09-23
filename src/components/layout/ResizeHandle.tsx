import { useCallback, useEffect, useRef } from 'react';

interface ResizeHandleProps {
  side: 'left' | 'right';
  onResize: (delta: number) => void;
  label: string;
}

/** 可拖拽的分栏把手。 */
export function ResizeHandle({ side, onResize, label }: ResizeHandleProps) {
  const dragging = useRef(false);
  const lastX = useRef(0);

  const onPointerMove = useCallback(
    (event: PointerEvent) => {
      if (!dragging.current) return;
      const delta = event.clientX - lastX.current;
      lastX.current = event.clientX;
      onResize(side === 'left' ? delta : -delta);
    },
    [onResize, side],
  );

  const stop = useCallback(() => {
    dragging.current = false;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  }, []);

  useEffect(() => {
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', stop);
    return () => {
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', stop);
    };
  }, [onPointerMove, stop]);

  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation="vertical"
      className="group relative hidden w-1.5 shrink-0 cursor-col-resize md:block"
      onPointerDown={(event) => {
        dragging.current = true;
        lastX.current = event.clientX;
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
      }}
    >
      <span className="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-transparent transition-colors group-hover:bg-[var(--brand)]" />
    </div>
  );
}
