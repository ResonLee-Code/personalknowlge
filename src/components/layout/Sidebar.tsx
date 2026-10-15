import { NotebookPen, PanelLeftClose, PanelLeftOpen, RefreshCw } from 'lucide-react';
import { useEffect } from 'react';
import { NavLink } from 'react-router-dom';
import { toast } from 'sonner';

import { CategoryTree } from '@/components/categories/CategoryTree';
import { TagList } from '@/components/tags/TagList';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { ROUTES } from '@/lib/constants';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useUiStore } from '@/stores/uiStore';
import { LAYOUT_DEFAULTS } from '@/stores/uiStore';
import { persistLayoutPreferences, useWorkspaceStore } from '@/stores/workspaceStore';

import { NavigationMenu } from './NavigationMenu';
import { buildNavItems } from './navItems';
import { ResizeHandle } from './ResizeHandle';
import { WorkspaceSwitcher } from './WorkspaceSwitcher';

export function Sidebar() {
  const width = useUiStore((state) => state.sidebarWidth);
  const collapsed = useUiStore((state) => state.sidebarCollapsed);
  const setWidth = useUiStore((state) => state.setSidebarWidth);
  const toggleSidebar = useUiStore((state) => state.toggleSidebar);
  const setNewNoteDialogOpen = useUiStore((state) => state.setNewNoteDialogOpen);
  const refreshCategories = useCategoriesStore((state) => state.refresh);
  const refreshTags = useTagsStore((state) => state.refresh);
  const workspaceId = useWorkspaceStore((state) => state.workspace?.id ?? null);
  const scan = useWorkspaceStore((state) => state.scan);
  const refreshList = useWorkspaceStore((state) => state.refreshList);
  const stats = useWorkspaceStore((state) => state.stats);

  useEffect(() => {
    if (!workspaceId) return;
    void refreshCategories();
    void refreshTags();
    void refreshList();
  }, [workspaceId, refreshCategories, refreshTags, refreshList]);

  if (collapsed) {
    /**
     * 收起态（44px 图标栏）——V1.3.0 第二轮界面调整。
     *
     * 以前收起后只剩「新建笔记」和一个展开按钮：想切页面只能先展开，而且整条栏空得发慌。
     * 现在把导航项一个个摆进来（对 Codex 桌面端的图标栏）：选中项用品牌色底，
     * 悬停出名字（tooltip），滚动条照旧——**收起不等于失去导航**。
     */
    const items = buildNavItems(stats);
    return (
      /*
       * 收起态：宽度压到 44px，但**竖直结构照旧**——顶上是新建笔记，底部是「展开侧边栏」。
       * 为什么把开关放在底部（V1.3.0 第二轮界面调整）：展开态那个「收起侧边栏」就在底部同一行，
       * 这样开关在两个状态下**停在同一个高度**，点它不用重新找位置（以前展开在上、收起在下）。
       */
      <div className="flex w-11 shrink-0 flex-col items-center pt-3">
        <Button
          variant="ghost"
          size="icon"
          title="新建笔记"
          aria-label="新建笔记"
          className="h-8 w-8 shrink-0"
          onClick={() => setNewNoteDialogOpen(true)}
        >
          <NotebookPen className="h-4 w-4" />
        </Button>
        <TooltipProvider delayDuration={250}>
          <ScrollArea className="mt-1 w-full flex-1">
            <nav aria-label="主导航" className="flex flex-col items-center gap-0.5 py-1">
              {items.map((item) => (
                <Tooltip key={item.to}>
                  <TooltipTrigger asChild>
                    <NavLink
                      to={item.to}
                      end={item.to === ROUTES.dashboard}
                      aria-label={item.label}
                      /**
                       * 选中态用 `aria-current` 变体表达，而不是 `className={({isActive}) => …}`：
                       * Radix 的 `TooltipTrigger asChild` 合并 props 时会把**函数**形式的 className
                       * 直接拼成字符串（实测：class 里真的出现了函数源码，高亮整条失效）。
                       * NavLink 自己会给当前项加 `aria-current="page"`，用属性选择器最稳。
                       */
                      className="flex h-8 w-8 items-center justify-center rounded-md text-[var(--ink-subtle)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)] aria-[current=page]:bg-[var(--brand-soft)] aria-[current=page]:text-[var(--brand)]"
                    >
                      <item.icon className="h-4 w-4" />
                    </NavLink>
                  </TooltipTrigger>
                  {/* 只显示图标，名字靠悬停补上（与 Codex 桌面端的图标栏一致） */}
                  <TooltipContent side="right">
                    {item.label}
                    {typeof item.count === 'number' && item.count > 0 ? ` · ${item.count}` : ''}
                  </TooltipContent>
                </Tooltip>
              ))}
            </nav>
          </ScrollArea>
        </TooltipProvider>
        {/* 与展开态的底栏同款（border-t + py-1.5）：两个状态下的开关落在同一高度 */}
        <div className="mt-auto flex w-full justify-center border-t border-[var(--border-subtle)] py-1.5">
          <Button variant="ghost" size="icon" onClick={toggleSidebar} title="展开侧边栏">
            <PanelLeftOpen className="h-4 w-4" />
          </Button>
        </div>
      </div>
    );
  }

  const rescan = async () => {
    const report = await scan();
    if (report) {
      toast.success(
        `扫描完成：新增 ${report.added}，更新 ${report.updated}，缺失 ${report.missing}`,
      );
    }
    await refreshCategories();
    await refreshTags();
    await useWorkspaceStore.getState().refreshStats();
  };

  return (
    <div className="flex shrink-0" style={{ width }}>
      <aside data-testid="sidebar" className="flex min-w-0 flex-1 flex-col">
        <div className="px-2 pt-2">
          <WorkspaceSwitcher />
        </div>
        <div className="px-2 pt-2">
          <Button
            className="w-full justify-start"
            size="sm"
            onClick={() => setNewNoteDialogOpen(true)}
          >
            <NotebookPen className="h-3.5 w-3.5" />
            新建笔记
          </Button>
        </div>
        <ScrollArea className="mt-2 flex-1">
          <div className="space-y-3 px-2 pb-4">
            <NavigationMenu />
            <CategoryTree />
            <TagList />
          </div>
        </ScrollArea>
        <div className="flex items-center justify-between border-t border-[var(--border-subtle)] px-2 py-1.5">
          <Button variant="ghost" size="sm" onClick={() => void rescan()}>
            <RefreshCw className="h-3.5 w-3.5" />
            重新扫描
          </Button>
          <Button variant="ghost" size="icon" onClick={toggleSidebar} title="收起侧边栏">
            <PanelLeftClose className="h-4 w-4" />
          </Button>
        </div>
      </aside>
      <ResizeHandle
        side="left"
        label="调整侧边栏宽度"
        value={width}
        min={180}
        max={420}
        onChange={setWidth}
        onCommit={persistLayoutPreferences}
        onReset={() => setWidth(LAYOUT_DEFAULTS.sidebarWidth)}
      />
    </div>
  );
}
