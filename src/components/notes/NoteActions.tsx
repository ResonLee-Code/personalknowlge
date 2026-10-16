import { FolderInput, MoreHorizontal, Star, StarOff } from 'lucide-react';

import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore, type NoteActionKind } from '@/stores/uiStore';

/**
 * 笔记级动作：收藏 / 移动分类 / 更多操作（2026-10-17 按用户要求挪到**顶栏**）。
 *
 * 只有"当前确实打开着一篇笔记"时才渲染（见 `TopBar` 里的路由判断）。
 * 真正的对话框挂在编辑器里——这里只发请求（`requestNoteAction`），那边接住并打开，
 * 对话框状态就不用从编辑器里搬出来（搬出来等于把十几个 state 提到全局）。
 */
export function NoteActions({
  noteId,
  favorite,
  deleted = false,
}: {
  noteId: string;
  favorite: boolean;
  /** 回收站里的笔记：菜单里显示「恢复 / 永久删除」，而不是「移到回收站」 */
  deleted?: boolean;
}) {
  const toggleFavorite = useNotesStore((state) => state.toggleFavorite);
  const requestNoteAction = useUiStore((state) => state.requestNoteAction);

  const item = (label: string, kind: NoteActionKind, danger?: boolean) => (
    <DropdownMenuItem
      key={kind}
      onSelect={() => requestNoteAction(kind)}
      // 历史版本给个 testid：E2E 一直这么点它（菜单从编辑器搬到顶栏，契约保持不变）
      data-testid={kind === 'history' ? 'note-menu-history' : undefined}
    >
      <span className={danger ? 'text-red-500' : undefined}>{label}</span>
    </DropdownMenuItem>
  );

  return (
    <div className="flex items-center gap-0.5">
      <Button
        size="icon"
        variant="ghost"
        title={favorite ? '取消收藏' : '收藏'}
        aria-pressed={favorite}
        data-testid="favorite-toggle"
        onClick={() => void toggleFavorite(noteId)}
      >
        {favorite ? (
          // 收藏后**橙色实心**：只描边太弱，一眼看不出状态
          <Star className="h-4 w-4 fill-amber-400 text-amber-500" />
        ) : (
          <StarOff className="h-4 w-4" />
        )}
      </Button>
      <Button
        size="icon"
        variant="ghost"
        title="移动分类"
        onClick={() => requestNoteAction('move')}
      >
        <FolderInput className="h-4 w-4" />
      </Button>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button size="icon" variant="ghost" title="更多操作">
            <MoreHorizontal className="h-4 w-4" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          {item('重命名 / 重命名文件', 'rename')}
          {item('创建副本', 'duplicate')}
          {item('历史版本…', 'history')}
          {item('来源与时效', 'source')}
          {item('复制相对路径', 'copyPath')}
          {item('在文件夹中显示', 'reveal')}
          <DropdownMenuSeparator />
          {item('另存为模板…', 'template')}
          {item('导出（Markdown / HTML / Word / PDF）', 'export')}
          <DropdownMenuSeparator />
          {deleted ? (
            <>
              {item('从回收站恢复', 'restore')}
              {item('永久删除', 'purge', true)}
            </>
          ) : (
            item('移到回收站', 'delete', true)
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
