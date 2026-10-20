import { FlaskConical, MessageCircleQuestion } from 'lucide-react';
import { useEffect } from 'react';

interface SelectionContextMenuProps {
  x: number;
  y: number;
  onAsk: () => void;
  onResearch: () => void;
  onClose: () => void;
}

/** 编辑器选中文字后的轻量右键菜单：只提供 Ask 与 Research 两个上下文动作。 */
export function SelectionContextMenu({
  x,
  y,
  onAsk,
  onResearch,
  onClose,
}: SelectionContextMenuProps) {
  useEffect(() => {
    const close = () => onClose();
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('pointerdown', close);
    window.addEventListener('keydown', keydown);
    return () => {
      window.removeEventListener('pointerdown', close);
      window.removeEventListener('keydown', keydown);
    };
  }, [onClose]);

  const left = Math.min(x, Math.max(8, window.innerWidth - 196));
  const top = Math.min(y, Math.max(8, window.innerHeight - 104));

  return (
    <div
      role="menu"
      aria-label="选中文字操作"
      data-testid="selection-context-menu"
      className="fixed z-[90] w-48 rounded-lg border border-[var(--border-subtle)] bg-[var(--surface)] p-1 shadow-xl"
      style={{ left, top }}
      onPointerDown={(event) => event.stopPropagation()}
    >
      <button
        type="button"
        role="menuitem"
        className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-xs text-[var(--ink)] hover:bg-[var(--brand-soft)] hover:text-[var(--brand)]"
        onClick={onAsk}
      >
        <MessageCircleQuestion className="h-3.5 w-3.5" />
        就选中文字提问
      </button>
      <button
        type="button"
        role="menuitem"
        className="flex w-full items-center gap-2 rounded-md px-2.5 py-2 text-left text-xs text-[var(--ink)] hover:bg-[var(--brand-soft)] hover:text-[var(--brand)]"
        onClick={onResearch}
      >
        <FlaskConical className="h-3.5 w-3.5" />
        就选中文字调研
      </button>
    </div>
  );
}
