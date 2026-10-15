import { Moon, Search, Settings, Sun } from 'lucide-react';
import { useNavigate } from 'react-router-dom';

import { SaveStatusBadge } from '@/components/editor/SaveStatusBadge';
import { Button } from '@/components/ui/button';
import { ROUTES } from '@/lib/constants';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';

export function TopBar() {
  const navigate = useNavigate();
  const setCommandPaletteOpen = useUiStore((state) => state.setCommandPaletteOpen);
  const theme = useUiStore((state) => state.theme);
  const setTheme = useUiStore((state) => state.setTheme);
  const workspace = useWorkspaceStore((state) => state.workspace);
  const saveStatus = useNotesStore((state) => state.saveStatus);

  return (
    <header className="flex h-12 shrink-0 items-center gap-3 px-3">
      {/* 侧边栏开关放在侧边栏自己身上（收起后也在同一个位置，见 Sidebar.tsx）：
          这里再来一个只会让人犹豫"两个是不是不一样" */}
      <div className="flex min-w-0 items-baseline gap-2">
        <span className="truncate text-sm font-semibold text-[var(--ink)]">
          {workspace?.name ?? 'Personal Knowledge OS'}
        </span>
        <span className="hidden truncate text-[11px] text-[var(--ink-subtle)] lg:inline">
          {workspace?.rootPath}
        </span>
      </div>

      <button
        type="button"
        onClick={() => setCommandPaletteOpen(true)}
        className="mx-auto hidden h-8 w-full max-w-md items-center gap-2 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-3 text-left text-sm text-[var(--ink-subtle)] transition-colors hover:border-[var(--brand)] hover:text-[var(--ink)] md:flex"
      >
        <Search className="h-3.5 w-3.5" />
        <span className="flex-1">搜索笔记内容、标题或标签…</span>
        <kbd className="rounded border border-[var(--border-subtle)] px-1 text-[10px]">⌘K</kbd>
      </button>

      <div className="ml-auto flex items-center gap-1.5">
        <SaveStatusBadge status={saveStatus} />
        <Button
          variant="ghost"
          size="icon"
          title={theme === 'dark' ? '切换为浅色主题' : '切换为深色主题'}
          onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
        >
          {theme === 'dark' ? <Sun className="h-4 w-4" /> : <Moon className="h-4 w-4" />}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title="设置"
          onClick={() => navigate(ROUTES.settings)}
        >
          <Settings className="h-4 w-4" />
        </Button>
      </div>
    </header>
  );
}
