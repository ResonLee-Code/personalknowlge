import { OutlinePanel } from '@/components/editor/OutlinePanel';
import { useOutlineStore } from '@/stores/outlineStore';

interface OutlinePaneProps {
  /** 与笔记列表同一栏位，宽度沿用列表宽度（拖拽调整的那条分隔线仍然生效） */
  width: number;
  onBack: () => void;
  onMoveToSide: () => void;
}

/**
 * 「同一栏位里的大纲」（2026-10-14 布局调整）。
 *
 * 单独拆成组件的理由：它要订阅大纲仓库（滚动时 activeOffset 会变），
 * 如果直接写在 `NotesSection` 里，笔记列表也会跟着一起重渲染——
 * 而那时候列表根本没显示，纯属白跑。
 */
export function OutlinePane({ width, onBack, onMoveToSide }: OutlinePaneProps) {
  const headings = useOutlineStore((state) => state.headings);
  const activeOffset = useOutlineStore((state) => state.activeOffset);
  const title = useOutlineStore((state) => state.title);
  const jump = useOutlineStore((state) => state.jump);

  return (
    <div
      data-testid="list-pane-outline"
      className="flex min-h-0 min-w-0 shrink-0 flex-col"
      style={{ width }}
    >
      <OutlinePanel
        headings={headings}
        activeOffset={activeOffset}
        onJump={(offset) => jump?.(offset)}
        title={title}
        onBack={onBack}
        onMoveToSide={onMoveToSide}
        className="w-full border-l-0 border-r border-[var(--border-subtle)]"
      />
    </div>
  );
}
