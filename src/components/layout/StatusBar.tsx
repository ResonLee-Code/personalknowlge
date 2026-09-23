import { AlertTriangle, Database, FolderOpen, HardDrive } from 'lucide-react';

import { formatCount } from '@/lib/format';
import { isTauriRuntime } from '@/services/api';
import { useNotesStore } from '@/stores/notesStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';

const STATE_LABEL: Record<string, string> = {
  ready: '索引就绪',
  stale: '索引待更新',
  rebuilding: '索引重建中',
  failed: '索引异常',
};

export function StatusBar() {
  const stats = useWorkspaceStore((state) => state.stats);
  const indexStatus = useWorkspaceStore((state) => state.indexStatus);
  const workspace = useWorkspaceStore((state) => state.workspace);
  const total = useNotesStore((state) => state.total);
  const saveStatus = useNotesStore((state) => state.saveStatus);
  const notesError = useNotesStore((state) => state.error);
  const workspaceError = useWorkspaceStore((state) => state.error);
  const error = notesError ?? workspaceError;

  const state = indexStatus?.state ?? 'ready';

  return (
    <footer className="flex h-7 shrink-0 items-center gap-3 border-t border-[var(--border-subtle)] px-3 text-[11px] text-[var(--ink-subtle)]">
      <span className="flex items-center gap-1">
        <Database className="h-3 w-3" />
        {STATE_LABEL[state] ?? state}
        {indexStatus ? ` (${indexStatus.indexedNotes}/${indexStatus.totalNotes})` : ''}
      </span>
      <span className="flex items-center gap-1">
        <HardDrive className="h-3 w-3" />
        笔记 {formatCount(stats.noteCount)} · 分类 {formatCount(stats.categoryCount)} · 标签{' '}
        {formatCount(stats.tagCount)}
      </span>
      <span>当前列表 {formatCount(total)} 条</span>
      {saveStatus === 'error' ? (
        <span className="flex items-center gap-1 text-red-500">
          <AlertTriangle className="h-3 w-3" />
          保存失败
        </span>
      ) : null}
      {error ? (
        <span className="flex items-center gap-1 text-red-500" title={error}>
          <AlertTriangle className="h-3 w-3" />
          {error.slice(0, 40)}
        </span>
      ) : null}
      <span className="ml-auto flex items-center gap-1">
        <FolderOpen className="h-3 w-3" />
        {isTauriRuntime() ? 'Tauri 本地模式' : '浏览器预览模式'}
        {workspace?.rootPath ? ` · ${workspace.rootPath}` : ''}
      </span>
    </footer>
  );
}
