import { Moon, Search, Settings, Sun } from 'lucide-react';
import { useLocation, useNavigate } from 'react-router-dom';

import { SaveStatusBadge } from '@/components/editor/SaveStatusBadge';
import { NoteActions } from '@/components/notes/NoteActions';
import { Button } from '@/components/ui/button';
import { ROUTES } from '@/lib/constants';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';

export function TopBar() {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const setCommandPaletteOpen = useUiStore((state) => state.setCommandPaletteOpen);
  const theme = useUiStore((state) => state.theme);
  const setTheme = useUiStore((state) => state.setTheme);
  const workspace = useWorkspaceStore((state) => state.workspace);
  const saveStatus = useNotesStore((state) => state.saveStatus);
  const detail = useNotesStore((state) => state.detail);
  /**
   * 当前是否真的打开着一篇笔记（顶栏的笔记动作只在此时出现）。
   * 顶栏在路由树之外，拿不到 `useParams`，所以按路径判断，并且要求
   * `detail` 与路径里的 id **一致**——否则切页时会短暂显示上一篇的收藏状态。
   */
  const routeNoteId =
    pathname.match(/^\/(?:notes|inbox|favorites|trash|missing)\/([^/]+)$/)?.[1] ?? null;
  const openNote = routeNoteId && detail?.note.id === routeNoteId ? detail.note : null;

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
        // 搜索栏与「已同步」靠在一起、整体偏左（2026-10-17 按用户要求）：
        // 以前搜索框 `mx-auto` 居中、保存状态贴在右端，中间空一大段
        className="hidden h-8 w-full max-w-md items-center gap-2 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-3 text-left text-sm text-[var(--ink-subtle)] transition-colors hover:border-[var(--brand)] hover:text-[var(--ink)] md:flex"
      >
        <Search className="h-3.5 w-3.5" />
        <span className="flex-1">搜索笔记内容、标题或标签…</span>
        <kbd className="rounded border border-[var(--border-subtle)] px-1 text-[10px]">⌘K</kbd>
      </button>

      {/* 「已同步」紧挨搜索栏 */}
      <SaveStatusBadge status={saveStatus} />

      <div className="ml-auto flex items-center gap-1.5">
        {/* 打开着笔记时：收藏 / 移动分类 / 更多操作（2026-10-17 从编辑器工具栏搬到顶栏） */}
        {openNote ? (
          <NoteActions
            noteId={openNote.id}
            favorite={openNote.favorite}
            deleted={openNote.status === 'deleted'}
          />
        ) : null}
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
