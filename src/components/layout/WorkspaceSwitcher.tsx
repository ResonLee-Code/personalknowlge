import { ChevronDown, FolderOpen, Library, Plus } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Input } from '@/components/ui/input';
import { pickDirectory } from '@/services/appService';
import { isTauriRuntime } from '@/services/api';
import { useNotesStore } from '@/stores/notesStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';

export function WorkspaceSwitcher() {
  const workspace = useWorkspaceStore((state) => state.workspace);
  const workspaces = useWorkspaceStore((state) => state.workspaces);
  const switchWorkspace = useWorkspaceStore((state) => state.switchWorkspace);
  const openWorkspace = useWorkspaceStore((state) => state.openWorkspace);
  const resetNotes = useNotesStore((state) => state.reset);
  const [manualRoot, setManualRoot] = useState('');

  const switchTo = async (id: string) => {
    const ok = await switchWorkspace(id);
    if (ok) {
      resetNotes();
      toast.success('已切换知识库');
    }
  };

  const openByPath = async () => {
    const path = isTauriRuntime() ? await pickDirectory('选择知识库目录') : manualRoot.trim();
    if (!path) return;
    const ok = await openWorkspace(path);
    if (ok) {
      resetNotes();
      toast.success('已打开知识库');
    } else {
      toast.error(useWorkspaceStore.getState().error ?? '打开失败');
    }
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className="flex w-full items-center gap-2 rounded-lg border border-transparent px-2 py-2 text-left transition-colors hover:border-[var(--border-subtle)] hover:bg-[var(--surface-muted)]"
        >
          <span className="flex h-7 w-7 items-center justify-center rounded-md bg-[var(--brand)] text-white">
            <Library className="h-4 w-4" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block truncate text-sm font-medium text-[var(--ink)]">
              {workspace?.name ?? '未打开知识库'}
            </span>
            <span className="block truncate text-[11px] text-[var(--ink-subtle)]">
              {workspace?.rootPath ?? '请选择一个本地目录'}
            </span>
          </span>
          <ChevronDown className="h-4 w-4 shrink-0 text-[var(--ink-subtle)]" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-72">
        <DropdownMenuLabel>最近打开</DropdownMenuLabel>
        {workspaces.length === 0 ? (
          <p className="px-2 py-1.5 text-xs text-[var(--ink-subtle)]">暂无记录</p>
        ) : (
          workspaces.map((item) => (
            <DropdownMenuItem
              key={item.workspace.id}
              onSelect={() => void switchTo(item.workspace.id)}
              className="flex-col items-start gap-0.5"
            >
              <span className="flex w-full items-center gap-2">
                <span className="flex-1 truncate">{item.workspace.name}</span>
                {item.isActive ? (
                  <span className="text-[11px] text-[var(--brand)]">当前</span>
                ) : null}
                {!item.isAvailable ? (
                  <span className="text-[11px] text-red-500">不可用</span>
                ) : null}
              </span>
              <span className="w-full truncate text-[11px] text-[var(--ink-subtle)]">
                {item.workspace.rootPath}
              </span>
            </DropdownMenuItem>
          ))
        )}
        <DropdownMenuSeparator />
        <div className="space-y-2 px-2 py-1.5">
          {!isTauriRuntime() ? (
            <Input
              value={manualRoot}
              placeholder="输入目录路径"
              onChange={(event) => setManualRoot(event.target.value)}
              className="h-8 text-xs"
            />
          ) : null}
          <div className="flex gap-2">
            <Button size="sm" variant="secondary" className="flex-1" onClick={() => void openByPath()}>
              <FolderOpen className="h-3.5 w-3.5" />
              打开知识库
            </Button>
            <Button
              size="sm"
              className="flex-1"
              onClick={() => useWorkspaceStore.setState({ workspace: null, settings: null })}
            >
              <Plus className="h-3.5 w-3.5" />
              新建
            </Button>
          </div>
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
