import {
  FileText,
  FolderTree,
  HardDrive,
  Inbox,
  ListChecks,
  LayoutDashboard,
  Network,
  FlaskConical,
  Settings,
  Star,
  Tags,
  Trash2,
} from 'lucide-react';
import { useState } from 'react';
import { NavLink } from 'react-router-dom';

import { ROUTES } from '@/lib/constants';
import { isNoteDrag, readNoteDragIds } from '@/lib/dragNotes';
import { cn } from '@/lib/utils';
import { useNotesStore } from '@/stores/notesStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';

interface NavItem {
  to: string;
  label: string;
  icon: typeof FileText;
  count?: number;
}

export function NavigationMenu() {
  const stats = useWorkspaceStore((state) => state.stats);
  /** 拖拽悬停在 Inbox 上时高亮 */
  const [inboxDropActive, setInboxDropActive] = useState(false);

  const items: NavItem[] = [
    { to: ROUTES.dashboard, label: '首页', icon: LayoutDashboard },
    { to: ROUTES.notes, label: '全部笔记', icon: FileText, count: stats.noteCount },
    { to: ROUTES.inbox, label: 'Inbox', icon: Inbox, count: stats.inboxCount },
    { to: ROUTES.favorites, label: '收藏', icon: Star },
    { to: ROUTES.graph, label: '知识图谱', icon: Network },
    { to: ROUTES.research, label: '研究助手', icon: FlaskConical },
    { to: ROUTES.attachments, label: '附件', icon: HardDrive },
    { to: ROUTES.manage, label: '目录管理', icon: ListChecks },
    { to: ROUTES.categories, label: '分类', icon: FolderTree, count: stats.categoryCount },
    { to: ROUTES.tags, label: '标签', icon: Tags, count: stats.tagCount },
    { to: ROUTES.trash, label: '回收站', icon: Trash2, count: stats.trashCount },
    { to: ROUTES.settings, label: '设置', icon: Settings },
  ];

  return (
    <nav className="flex flex-col gap-0.5" aria-label="主导航">
      {items.map((item) => (
        <NavLink
          key={item.to}
          to={item.to}
          end={item.to === ROUTES.dashboard}
          data-testid={item.to === ROUTES.inbox ? 'inbox-drop' : undefined}
          onDragOver={(event) => {
            // Inbox = 未分类：把笔记拖到这里即移出分类
            if (item.to !== ROUTES.inbox || !isNoteDrag(event.dataTransfer)) return;
            event.preventDefault();
            event.dataTransfer.dropEffect = 'move';
            setInboxDropActive(true);
          }}
          onDragLeave={() => setInboxDropActive(false)}
          onDrop={(event) => {
            if (item.to !== ROUTES.inbox) return;
            const ids = readNoteDragIds(event.dataTransfer);
            setInboxDropActive(false);
            if (ids.length === 0) return;
            event.preventDefault();
            void useNotesStore.getState().moveNotes(ids, null);
          }}
          className={({ isActive }) =>
            cn(
              'flex items-center gap-2 rounded-md px-2 py-1.5 text-sm transition-colors',
              isActive
                ? 'bg-[var(--brand-soft)] font-medium text-[var(--brand)]'
                : 'text-[var(--ink-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]',
              item.to === ROUTES.inbox && inboxDropActive && 'ring-2 ring-[var(--brand)]/60',
            )
          }
        >
          <item.icon className="h-4 w-4" />
          <span className="flex-1 truncate">{item.label}</span>
          {typeof item.count === 'number' && item.count > 0 ? (
            <span className="text-[11px] tabular-nums text-[var(--ink-subtle)]">{item.count}</span>
          ) : null}
        </NavLink>
      ))}
    </nav>
  );
}
