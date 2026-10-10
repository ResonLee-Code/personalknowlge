import {
  ChevronDown,
  ChevronRight,
  Copy,
  CornerLeftUp,
  Download,
  FilePlus2,
  FileCode2,
  FolderTree,
  Heading1,
  MoreHorizontal,
  Pin,
  PinOff,
  PencilLine,
  Star,
  StarOff,
  Trash2,
  Undo2,
} from 'lucide-react';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { ExportNoteDialog } from '@/components/notes/ExportNoteDialog';
import { CreateChildNoteDialog } from '@/components/notes/CreateChildNoteDialog';
import { DeleteNoteDialog } from '@/components/notes/DeleteNoteDialog';
import { MoveNoteDialog } from '@/components/notes/MoveNoteDialog';
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { RenameFileDialog } from '@/components/editor/RenameFileDialog';
import { isNoteDrag, readNoteDragIds, setNoteDragData } from '@/lib/dragNotes';
import { noteDropPosition, type NoteDropPosition } from '@/lib/noteDragDrop';
import { splitHighlight } from '@/lib/highlight';
import { cn } from '@/lib/utils';
import { useNotesStore } from '@/stores/notesStore';
import type { NoteSummary } from '@/types';

type MenuVariant = 'dropdown' | 'context';

/** 笔记条目上的操作集合：`⋮` 与右键菜单共用同一份定义（对标语雀）。 */
function NoteMenuItems({
  variant,
  note,
  onOpen,
  onToggleFavorite,
  onTogglePinned,
  onCreateChild,
  onPromote,
  onCopyPath,
  onCopyTitle,
  onRename,
  onMove,
  onExport,
  onDuplicate,
  onDelete,
  onRestore,
  onPurge,
}: {
  variant: MenuVariant;
  note: NoteSummary;
  onOpen: () => void;
  onToggleFavorite: () => void;
  onTogglePinned: () => void;
  onCreateChild: () => void;
  onPromote: () => void;
  onCopyPath: () => void;
  onCopyTitle: () => void;
  onRename: () => void;
  onMove: () => void;
  onExport: () => void;
  onDuplicate: () => void;
  onDelete: () => void;
  onRestore: () => void;
  onPurge: () => void;
}) {
  const Item = variant === 'context' ? ContextMenuItem : DropdownMenuItem;
  const Separator = variant === 'context' ? ContextMenuSeparator : DropdownMenuSeparator;

  return (
    <>
      <Item onSelect={onOpen} data-testid="note-menu-open">
        打开
      </Item>
      <Item onSelect={onToggleFavorite} data-testid="note-menu-favorite">
        {note.favorite ? (
          <>
            <StarOff className="h-3.5 w-3.5" /> 取消收藏
          </>
        ) : (
          <>
            <Star className="h-3.5 w-3.5" /> 收藏
          </>
        )}
      </Item>
      <Item onSelect={onTogglePinned} data-testid="note-menu-pin">
        {note.pinned ? (
          <>
            <PinOff className="h-3.5 w-3.5" /> 取消置顶
          </>
        ) : (
          <>
            <Pin className="h-3.5 w-3.5" /> 置顶
          </>
        )}
      </Item>
      <Separator />
      {/* 回收站中的笔记不能作为父文档 */}
      {note.status === 'active' ? (
        <Item onSelect={onCreateChild} data-testid="note-menu-create-child">
          <FilePlus2 className="h-3.5 w-3.5" /> 创建子文档
        </Item>
      ) : null}
      {note.parentId && note.status === 'active' ? (
        <Item onSelect={onPromote} data-testid="note-menu-promote">
          <CornerLeftUp className="h-3.5 w-3.5" /> 升级为顶层文档
        </Item>
      ) : null}
      {/* 对标语雀「复制链接」：本地知识库里对应的是相对路径与标题 */}
      <Item onSelect={onCopyPath} data-testid="note-menu-copy-path">
        <FileCode2 className="h-3.5 w-3.5" /> 复制相对路径
      </Item>
      <Item onSelect={onCopyTitle} data-testid="note-menu-copy-title">
        <Heading1 className="h-3.5 w-3.5" /> 复制标题
      </Item>
      <Separator />
      <Item onSelect={onRename} data-testid="note-menu-rename">
        <PencilLine className="h-3.5 w-3.5" /> 重命名…
      </Item>
      <Item onSelect={onMove} data-testid="note-menu-move">
        <FolderTree className="h-3.5 w-3.5" /> 移动分类…
      </Item>
      <Item onSelect={onExport} data-testid="note-menu-export">
        <Download className="h-3.5 w-3.5" /> 导出（MD / HTML / Word / PDF）
      </Item>
      <Item onSelect={onDuplicate} data-testid="note-menu-duplicate">
        <Copy className="h-3.5 w-3.5" /> 创建副本
      </Item>
      <Separator />
      {note.status === 'deleted' ? (
        <>
          <Item onSelect={onRestore} data-testid="note-menu-restore">
            <Undo2 className="h-3.5 w-3.5" /> 恢复
          </Item>
          <Item onSelect={onPurge} destructive data-testid="note-menu-purge">
            <Trash2 className="h-3.5 w-3.5" /> 永久删除
          </Item>
        </>
      ) : (
        <Item onSelect={onDelete} destructive data-testid="note-menu-delete">
          <Trash2 className="h-3.5 w-3.5" /> 移到回收站
        </Item>
      )}
    </>
  );
}

export function NoteListItem({
  note,
  selected,
  basePath,
  onOpen,
  depth = 0,
  childCount = 0,
  collapsed = false,
  onToggleCollapse,
  dropHint = null,
  onNoteDragOver,
  onNoteDragLeave,
  onNoteDrop,
}: {
  note: NoteSummary;
  selected: boolean;
  basePath: string;
  onOpen: () => void;
  /** 子文档层级缩进（0 = 顶层文档） */
  depth?: number;
  /** 子文档数量（> 0 时显示展开 / 折叠箭头） */
  childCount?: number;
  collapsed?: boolean;
  onToggleCollapse?: () => void;
  /** 拖拽悬停落在本行时的提示：inside = 成为子文档，before/after = 同层插入 */
  dropHint?: NoteDropPosition | null;
  onNoteDragOver?: (position: NoteDropPosition, draggedIds: string[]) => void;
  onNoteDragLeave?: () => void;
  onNoteDrop?: (position: NoteDropPosition, draggedIds: string[]) => void;
}) {
  const navigate = useNavigate();
  const keyword = useNotesStore((state) => state.keyword);
  const toggleFavorite = useNotesStore((state) => state.toggleFavorite);
  const remove = useNotesStore((state) => state.remove);
  const restore = useNotesStore((state) => state.restore);
  const purge = useNotesStore((state) => state.purge);
  const duplicate = useNotesStore((state) => state.duplicate);
  const setParent = useNotesStore((state) => state.setParent);
  const setPinned = useNotesStore((state) => state.setPinned);
  const selectionMode = useNotesStore((state) => state.selectionMode);
  const selectedForBatch = useNotesStore((state) => state.selection.includes(note.id));
  const toggleSelected = useNotesStore((state) => state.toggleSelected);
  const setSelectionMode = useNotesStore((state) => state.setSelectionMode);

  const [childOpen, setChildOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [moveOpen, setMoveOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [deleteOpen, setDeleteOpen] = useState(false);

  /** 当前列表结果里的直接子文档（用于删除确认框与提示） */
  const children = useNotesStore((state) => state.items).filter(
    (item) => item.parentId === note.id,
  );
  const titleSegments = keyword ? splitHighlight(note.title, [keyword]) : [{ text: note.title, matched: false }];
  const snippet = note.snippet ?? [];
  const deleted = note.status === 'deleted';

  const handleFavorite = () => {
    void toggleFavorite(note.id).then(() => toast.success(note.favorite ? '已取消收藏' : '已收藏'));
  };

  const handlePin = () => {
    void setPinned(note.id, !note.pinned).then(() =>
      toast.success(note.pinned ? '已取消置顶' : '已置顶（会一直排在最前）'),
    );
  };

  const copyText = async (text: string, label: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(`已复制${label}`);
    } catch {
      toast.error('复制失败，请检查系统的剪贴板权限');
    }
  };

  const handlePromote = () => {
    void setParent(note.id, null).then((ok) => {
      if (ok) toast.success('已升级为顶层文档');
      else toast.error(useNotesStore.getState().error ?? '操作失败');
    });
  };

  const handleDelete = () => {
    if (deleted) {
      void restore(note.id).then(() => toast.success('已从回收站恢复'));
      return;
    }
    // 有子文档时先确认（子文档默认保留，可勾选一起删除）
    if (children.length > 0) {
      setDeleteOpen(true);
      return;
    }
    void remove(note.id).then(() => {
      // 对标语雀：删除后可随即撤销（回收站是软删除，恢复即撤销）
      toast.success('已移入回收站', {
        action: {
          label: '撤销',
          onClick: () => void restore(note.id).then(() => toast.success('已恢复笔记')),
        },
      });
      if (basePath !== '/trash') navigate(basePath);
    });
  };

  const menuHandlers = {
    onOpen,
    onToggleFavorite: handleFavorite,
    onTogglePinned: handlePin,
    onCreateChild: () => setChildOpen(true),
    onPromote: handlePromote,
    onCopyPath: () => void copyText(note.filePath, '相对路径'),
    onCopyTitle: () => void copyText(note.title, '标题'),
    onRename: () => setRenameOpen(true),
    onMove: () => setMoveOpen(true),
    onExport: () => setExportOpen(true),
    onDuplicate: () => void duplicate(note.id).then(() => toast.success('已创建副本')),
    onDelete: handleDelete,
    onRestore: () => void restore(note.id).then(() => toast.success('已从回收站恢复')),
    onPurge: () => void purge(note.id).then(() => toast.success('已永久删除')),
  };

  return (
    <li>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <div
            role="button"
            tabIndex={0}
            data-testid="note-list-item"
            data-depth={depth}
            onClick={(event) => {
              // 多选模式下点击即勾选；Cmd/Ctrl+点击随时切换并进入多选
              if (selectionMode || event.metaKey || event.ctrlKey) {
                event.preventDefault();
                if (!selectionMode) setSelectionMode(true);
                toggleSelected(note.id);
                return;
              }
              onOpen();
            }}
            onKeyDownCapture={(event) => {
              if (selectionMode && (event.key === ' ' || event.key === 'Enter')) {
                event.preventDefault();
                event.stopPropagation();
                toggleSelected(note.id);
              }
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') onOpen();
            }}
            draggable
            onDragOver={(event) => {
              if (!isNoteDrag(event.dataTransfer)) return;
              // 必须 preventDefault，否则不会触发 drop
              event.preventDefault();
              event.dataTransfer.dropEffect = 'move';
              const rect = event.currentTarget.getBoundingClientRect();
              onNoteDragOver?.(
                noteDropPosition(event.clientY - rect.top, rect.height),
                readNoteDragIds(event.dataTransfer),
              );
            }}
            onDragLeave={() => onNoteDragLeave?.()}
            onDrop={(event) => {
              if (!isNoteDrag(event.dataTransfer)) return;
              event.preventDefault();
              const rect = event.currentTarget.getBoundingClientRect();
              onNoteDrop?.(
                noteDropPosition(event.clientY - rect.top, rect.height),
                readNoteDragIds(event.dataTransfer),
              );
            }}
            onDragStart={(event) => {
              // 多选中拖动任意一行 = 拖动整批；否则只拖这一篇
              const batch = useNotesStore.getState().selection;
              const ids = batch.includes(note.id) ? batch : [note.id];
              setNoteDragData(event.dataTransfer, ids, note.title);
            }}
            className={cn(
              'group relative cursor-pointer rounded-lg border border-transparent px-2.5 py-2 transition-colors',
              selectedForBatch
                ? 'border-[var(--brand)]/60 bg-[var(--brand-soft)]'
                : selected
                  ? 'border-[var(--brand)]/40 bg-[var(--brand-soft)]'
                  : 'hover:bg-[var(--surface-muted)]',
              // 对标语雀：蓝色框 = 拖成该节点的子级
              dropHint === 'inside' && 'ring-2 ring-[var(--brand)] bg-[var(--brand-soft)]',
            )}
          >
            {/* 对标语雀：蓝色横线 = 拖到该位置（同层排序） */}
            {dropHint === 'before' ? (
              <span
                data-testid="note-drop-line"
                aria-hidden
                className="pointer-events-none absolute inset-x-1 -top-px h-0.5 rounded-full bg-[var(--brand)]"
              />
            ) : null}
            {dropHint === 'after' ? (
              <span
                data-testid="note-drop-line"
                aria-hidden
                className="pointer-events-none absolute inset-x-1 -bottom-px h-0.5 rounded-full bg-[var(--brand)]"
              />
            ) : null}
            {/*
              语雀式目录条目：一行只有「层级 + 标题 + 状态图标」。
              分类 / 标签 / 更新时间在预览页的元信息栏里已有，列表不再重复展示；
              元数据要一览时用「目录管理」页的宽表。
            */}
            <div className="flex items-center gap-1.5">
              {selectionMode ? (
                <input
                  type="checkbox"
                  className="shrink-0"
                  checked={selectedForBatch}
                  aria-label={`选择 ${note.title}`}
                  onChange={() => toggleSelected(note.id)}
                  onClick={(event) => event.stopPropagation()}
                />
              ) : null}
              {/* 子文档层级：缩进 + 展开 / 折叠箭头 */}
              <span
                className="flex w-3.5 shrink-0 items-center justify-center"
                style={{ marginLeft: depth * 12 }}
              >
                {childCount > 0 ? (
                  <button
                    type="button"
                    data-testid="note-tree-toggle"
                    aria-label={collapsed ? `展开 ${note.title}` : `折叠 ${note.title}`}
                    aria-expanded={!collapsed}
                    className="rounded p-0.5 text-[var(--ink-subtle)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]"
                    onClick={(event) => {
                      event.preventDefault();
                      event.stopPropagation();
                      onToggleCollapse?.();
                    }}
                  >
                    {collapsed ? (
                      <ChevronRight className="h-3 w-3" />
                    ) : (
                      <ChevronDown className="h-3 w-3" />
                    )}
                  </button>
                ) : null}
              </span>
              <Tooltip>
                <TooltipTrigger asChild>
                  <span
                    data-testid="note-list-item-title"
                    className="min-w-0 flex-1 truncate text-sm text-[var(--ink)]"
                  >
                    {titleSegments.map((segment, index) => (
                      <span key={index} className={segment.matched ? 'search-highlight' : undefined}>
                        {segment.text}
                      </span>
                    ))}
                  </span>
                </TooltipTrigger>
                {/* 标题被截断时，悬停显示完整标题 */}
                <TooltipContent side="top" className="max-w-[min(420px,60vw)] break-words">
                  {note.title}
                </TooltipContent>
              </Tooltip>
              {note.pinned ? (
                <span
                  data-testid="note-pinned-marker"
                  role="img"
                  aria-label="已置顶"
                  title="已置顶"
                  className="flex shrink-0 text-[var(--brand)]"
                >
                  <Pin className="h-3 w-3" />
                </span>
              ) : null}
              {note.favorite ? (
                <span
                  data-testid="note-favorite-marker"
                  role="img"
                  aria-label="已收藏"
                  title="已收藏"
                  className="flex shrink-0 text-amber-500"
                >
                  <Star className="h-3 w-3" />
                </span>
              ) : null}
              {/* hover 才出现的快捷操作（对标语雀：⋮ 与 + 在建条目右侧） */}
              <span className="ml-auto flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
                <button
                  type="button"
                  data-testid="note-quick-child"
                  title="新建子文档"
                  aria-label={`为 ${note.title} 新建子文档`}
                  disabled={deleted}
                  onClick={(event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    setChildOpen(true);
                  }}
                  className="rounded p-0.5 text-[var(--ink-subtle)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--brand)] disabled:opacity-40"
                >
                  <FilePlus2 className="h-3.5 w-3.5" />
                </button>
                <DropdownMenu>
                  <DropdownMenuTrigger asChild>
                    <button
                      type="button"
                      aria-label="笔记操作"
                      title="更多操作"
                      onClick={(event) => event.stopPropagation()}
                      className="rounded p-0.5 text-[var(--ink-subtle)] data-[state=open]:opacity-100"
                    >
                      <MoreHorizontal className="h-3.5 w-3.5" />
                    </button>
                  </DropdownMenuTrigger>
                  <DropdownMenuContent align="start" onClick={(event) => event.stopPropagation()}>
                    <NoteMenuItems variant="dropdown" note={note} {...menuHandlers} />
                  </DropdownMenuContent>
                </DropdownMenu>
              </span>
            </div>
            {/* 搜索态才显示命中摘要：定位匹配位置用，平时保持列表紧凑 */}
            {keyword && snippet.length > 0 ? (
              <p className="mt-0.5 line-clamp-2 pl-5 text-[11px] leading-relaxed text-[var(--ink-muted)]">
                {snippet.map((segment, index) => (
                  <span key={index} className={segment.matched ? 'search-highlight' : undefined}>
                    {segment.text}
                  </span>
                ))}
              </p>
            ) : null}
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent onClick={(event) => event.stopPropagation()}>
          <NoteMenuItems variant="context" note={note} {...menuHandlers} />
        </ContextMenuContent>
      </ContextMenu>

      {childOpen ? (
        <CreateChildNoteDialog parent={note} onClose={() => setChildOpen(false)} />
      ) : null}
      {renameOpen ? (
        <RenameFileDialog
          noteId={note.id}
          initialTitle={note.title}
          onClose={() => setRenameOpen(false)}
        />
      ) : null}
      {moveOpen ? (
        <MoveNoteDialog
          noteId={note.id}
          currentCategoryId={note.categoryId}
          onClose={() => setMoveOpen(false)}
        />
      ) : null}
      {exportOpen ? (
        <ExportNoteDialog noteId={note.id} title={note.title} onClose={() => setExportOpen(false)} />
      ) : null}
      {deleteOpen ? (
        <DeleteNoteDialog
          note={note}
          children={children}
          onClose={() => setDeleteOpen(false)}
          onDeleted={() => {
            if (basePath !== '/trash') navigate(basePath);
          }}
        />
      ) : null}
    </li>
  );
}
