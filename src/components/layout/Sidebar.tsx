import { NotebookPen, PanelLeftClose, PanelLeftOpen, RefreshCw } from 'lucide-react';
import { useEffect } from 'react';
import { toast } from 'sonner';

import { CategoryTree } from '@/components/categories/CategoryTree';
import { TagList } from '@/components/tags/TagList';
import { Button } from '@/components/ui/button';
import { ScrollArea } from '@/components/ui/scroll-area';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';

import { NavigationMenu } from './NavigationMenu';
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

  useEffect(() => {
    if (!workspaceId) return;
    void refreshCategories();
    void refreshTags();
    void refreshList();
  }, [workspaceId, refreshCategories, refreshTags, refreshList]);

  if (collapsed) {
    return (
      <div className="flex w-11 shrink-0 flex-col items-center gap-2 py-3">
        <Button variant="ghost" size="icon" onClick={toggleSidebar} title="展开侧边栏">
          <PanelLeftOpen className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title="新建笔记"
          onClick={() => setNewNoteDialogOpen(true)}
        >
          <NotebookPen className="h-4 w-4" />
        </Button>
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
      <aside className="flex min-w-0 flex-1 flex-col">
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
      <ResizeHandle side="left" label="调整侧边栏宽度" onResize={setWidth} />
    </div>
  );
}
