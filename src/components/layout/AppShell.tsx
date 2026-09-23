import { useCallback, useState } from 'react';
import { Outlet, useNavigate } from 'react-router-dom';

import { GlobalSearchDialog } from '@/components/search/GlobalSearchDialog';
import { NewNoteDialog } from '@/components/notes/NewNoteDialog';
import { useKeyboardShortcuts } from '@/hooks/useKeyboardShortcuts';
import { ROUTES } from '@/lib/constants';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';

import { Sidebar } from './Sidebar';
import { StatusBar } from './StatusBar';
import { TopBar } from './TopBar';

/** 三栏工作台外壳（设计文档 16.1）。 */
export function AppShell() {
  const navigate = useNavigate();
  const setCommandPaletteOpen = useUiStore((state) => state.setCommandPaletteOpen);
  const setNewNoteDialogOpen = useUiStore((state) => state.setNewNoteDialogOpen);
  const selectedId = useNotesStore((state) => state.selectedId);
  const removeNote = useNotesStore((state) => state.remove);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);

  const openSearch = useCallback(() => setCommandPaletteOpen(true), [setCommandPaletteOpen]);
  const openNewNote = useCallback(() => setNewNoteDialogOpen(true), [setNewNoteDialogOpen]);

  useKeyboardShortcuts({
    onSearch: openSearch,
    onNewNote: openNewNote,
    onDelete: () => {
      if (selectedId) setDeleteTarget(selectedId);
    },
    onCloseNote: () => navigate(ROUTES.notes),
  });

  return (
    <div className="flex h-full min-h-0 flex-col bg-[var(--surface-muted)]">
      <TopBar />
      <div className="flex min-h-0 flex-1 overflow-hidden">
        <Sidebar />
        <main className="flex min-h-0 min-w-0 flex-1 overflow-hidden rounded-tl-lg border-l border-t border-[var(--border-subtle)] bg-[var(--surface)]">
          <Outlet />
        </main>
      </div>
      <StatusBar />
      <GlobalSearchDialog />
      <NewNoteDialog />
      {deleteTarget ? (
        <ConfirmDeleteDialog
          noteId={deleteTarget}
          onCancel={() => setDeleteTarget(null)}
          onConfirm={async () => {
            await removeNote(deleteTarget);
            setDeleteTarget(null);
            navigate(ROUTES.notes);
          }}
        />
      ) : null}
    </div>
  );
}

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';

function ConfirmDeleteDialog({
  noteId,
  onCancel,
  onConfirm,
}: {
  noteId: string;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <AlertDialog open onOpenChange={(open) => (!open ? onCancel() : undefined)}>
      <AlertDialogContent>
        <AlertDialogTitle>删除这篇笔记？</AlertDialogTitle>
        <AlertDialogDescription>
          笔记会移动到回收站（<code>.trash</code>），可以随时恢复。
          对应笔记 ID：{noteId}
        </AlertDialogDescription>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={onCancel}>取消</AlertDialogCancel>
          <AlertDialogAction destructive onClick={onConfirm}>
            移到回收站
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
