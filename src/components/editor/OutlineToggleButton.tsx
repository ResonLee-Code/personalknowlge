import { ListTree } from 'lucide-react';

import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { useUiStore } from '@/stores/uiStore';

/**
 * 大纲开关（V1.3.0 第二轮界面调整）。
 *
 * 位置：**笔记列表栏的头部**。原来的入口在编辑器工具栏，但那个工具栏只在打开笔记时才存在；
 * 列表栏一直都在，开关放在这里更稳定（也更符合"这一栏放什么由这一栏自己管"）。
 *
 * 语义随「大纲位置」而变（两者都是同一件事：让大纲可见 / 不可见）：
 * - 位置 = 列表栏（默认）：切换**这一栏**显示列表还是大纲——同一栏切换，不多占一列；
 * - 位置 = 正文右侧：显示 / 隐藏右边那一列（原来工具栏按钮的行为）。
 *
 * 没有选中笔记时按钮**置灰**：那会儿没有大纲可看，点上去不该什么都不发生（会让人以为坏了）。
 */
export function OutlineToggleButton({ hasNote }: { hasNote: boolean }) {
  const placement = useUiStore((state) => state.outlinePlacement);
  const listPaneMode = useUiStore((state) => state.listPaneMode);
  const setListPaneMode = useUiStore((state) => state.setListPaneMode);
  const outlineMode = useUiStore((state) => state.outlineMode);
  const setOutlineMode = useUiStore((state) => state.setOutlineMode);

  const active = placement === 'list' ? listPaneMode === 'outline' : outlineMode !== 'hidden';
  const toggle = () => {
    if (placement === 'list') {
      setListPaneMode(listPaneMode === 'outline' ? 'list' : 'outline');
      return;
    }
    setOutlineMode(outlineMode === 'hidden' ? 'pinned' : 'hidden');
  };

  return (
    <Button
      size="icon"
      variant="ghost"
      data-testid="outline-toggle"
      disabled={!hasNote}
      title={
        !hasNote ? '显示大纲（先选中一篇笔记）' : active ? '隐藏大纲' : '显示大纲'
      }
      aria-pressed={active}
      className={cn('h-7 w-7 shrink-0', active && 'text-[var(--brand)]')}
      onClick={toggle}
    >
      <ListTree className="h-3.5 w-3.5" />
    </Button>
  );
}
