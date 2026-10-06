import {
  Check,
  Columns2,
  Eye,
  FileText,
  FolderInput,
  FolderOpen,
  Download,
  MoreHorizontal,
  Pencil,
  Save,
  Star,
  StarOff,
  Trash2,
  Undo2,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { ConflictDialog } from '@/components/editor/ConflictDialog';
import { MarkdownEditor } from '@/components/editor/MarkdownEditor';
import { MarkdownPreview } from '@/components/editor/MarkdownPreview';
import { RenameFileDialog } from '@/components/editor/RenameFileDialog';
import { ExportNoteDialog } from '@/components/notes/ExportNoteDialog';
import { SaveStatusBadge } from '@/components/editor/SaveStatusBadge';
import { TagEditor } from '@/components/editor/TagEditor';
import { ResizeHandle } from '@/components/layout/ResizeHandle';
import { MoveNoteDialog } from '@/components/notes/MoveNoteDialog';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { Skeleton } from '@/components/ui/badge';
import { useAutosave } from '@/hooks/useAutosave';
import { formatDateTime, formatRelative } from '@/lib/format';
import { cn } from '@/lib/utils';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { isTauriRuntime } from '@/services/api';
import { useNotesStore } from '@/stores/notesStore';
import { LAYOUT_DEFAULTS, useUiStore } from '@/stores/uiStore';
import { persistLayoutPreferences, useWorkspaceStore } from '@/stores/workspaceStore';

/** 笔记编辑器（设计文档 16.5）。 */
export function NoteEditorPage() {
  const navigate = useNavigate();
  const detail = useNotesStore((state) => state.detail);
  const detailLoading = useNotesStore((state) => state.detailLoading);
  const saveStatus = useNotesStore((state) => state.saveStatus);
  const saveError = useNotesStore((state) => state.saveError);
  const save = useNotesStore((state) => state.save);
  const markDirty = useNotesStore((state) => state.markDirty);
  const toggleFavorite = useNotesStore((state) => state.toggleFavorite);
  const remove = useNotesStore((state) => state.remove);
  const restore = useNotesStore((state) => state.restore);
  const purge = useNotesStore((state) => state.purge);
  const duplicate = useNotesStore((state) => state.duplicate);
  const refreshScope = useNotesStore((state) => state.refresh);

  const settings = useWorkspaceStore((state) => state.settings);
  const theme = useUiStore((state) => state.theme);
  const editorMode = useUiStore((state) => state.editorMode);
  const setEditorMode = useUiStore((state) => state.setEditorMode);
  const fontSize = useUiStore((state) => state.fontSize);
  const editorSplitRatio = useUiStore((state) => state.editorSplitRatio);
  const setEditorSplitRatio = useUiStore((state) => state.setEditorSplitRatio);

  const [content, setContent] = useState('');
  const [title, setTitle] = useState('');
  const [tags, setTags] = useState<string[]>([]);
  const [moveOpen, setMoveOpen] = useState(false);
  const [renameOpen, setRenameOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [forceOverwrite, setForceOverwrite] = useState(false);
  const loadedId = useRef<string | null>(null);
  const baseHash = useRef<string | null>(null);
  const editorWrapper = useRef<HTMLDivElement | null>(null);
  const [splitWidth, setSplitWidth] = useState(0);
  const splitObserver = useRef<ResizeObserver | null>(null);

  const signature = `${content}\u0000${title}\u0000${tags.join(',')}`;

  const { saveNow, resetBaseline } = useAutosave({
    enabled: settings?.autosaveEnabled ?? false,
    delay: settings?.autosaveDelayMs ?? 1000,
    signature: detail ? signature : null,
    onSave: async () => {
      if (!detail) return false;
      const tagsChanged =
        tags.length !== detail.tags.length ||
        tags.some((tag) => !detail.tags.some((item) => item.name === tag));
      const overwrite = forceOverwrite;
      setForceOverwrite(false);
      const ok = await save({
        id: detail.note.id,
        content,
        baseContentHash: overwrite ? null : baseHash.current,
        title,
        tags: tagsChanged ? tags : undefined,
      });
      if (ok) {
        const latest = useNotesStore.getState().detail;
        baseHash.current = latest?.note.contentHash ?? null;
        resetBaseline(`${content}\u0000${title}\u0000${tags.join(',')}`);
      }
      return ok;
    },
  });

  useEffect(() => {
    if (!detail) return;
    if (loadedId.current !== detail.note.id) {
      loadedId.current = detail.note.id;
      // 切换笔记：把磁盘内容同步到本地可编辑状态，并清除上一次的覆盖标记
      setForceOverwrite(false);
      setContent(detail.content);
      setTitle(detail.note.title);
      setTags(detail.tags.map((tag) => tag.name));
      baseHash.current = detail.note.contentHash ?? null;
      resetBaseline(
        `${detail.content}\u0000${detail.note.title}\u0000${detail.tags
          .map((tag) => tag.name)
          .join(',')}`,
      );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detail?.note.id]);

  const focusEditor = useCallback(() => {
    const element = editorWrapper.current?.querySelector('.cm-content');
    if (element instanceof HTMLElement) element.focus();
  }, []);

  /**
   * 记录分屏容器宽度，用于把拖拽的像素增量换算成比例。
   *
   * 用回调 ref 而不是 useEffect：首次进入笔记时内容还在加载（组件会提前返回），
   * 容器此刻并不存在；若只依赖 editorMode，观察器就永远不会建立，
   * 拖拽时按 1px = 1 计算比例，会瞬间贴到最小/最大（表现为「拖动不自然」）。
   */
  const attachSplitContainer = useCallback((node: HTMLDivElement | null) => {
    splitObserver.current?.disconnect();
    splitObserver.current = null;
    if (!node) return;
    const update = () => setSplitWidth(node.clientWidth);
    update();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(update);
    observer.observe(node);
    splitObserver.current = observer;
  }, []);

  useEffect(() => () => splitObserver.current?.disconnect(), []);

  /** 切换视图后立即持久化，下次打开不会回到旧模式 */
  const changeEditorMode = useCallback(
    (mode: typeof editorMode) => {
      setEditorMode(mode);
      void useWorkspaceStore.getState().updateSettings({ editorMode: mode });
    },
    [setEditorMode],
  );

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const meta = event.metaKey || event.ctrlKey;
      if (meta && event.key.toLowerCase() === 's') {
        event.preventDefault();
        void saveNow();
      }
      if (meta && event.key.toLowerCase() === 'e') {
        event.preventDefault();
        focusEditor();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [saveNow, focusEditor]);

  const words = useMemo(() => {
    const cjk = (content.match(/[\u3400-\u9fff\u3040-\u30ff]/g) ?? []).length;
    const latin = (content.match(/[A-Za-z0-9_'\u00c0-\u024f]+/g) ?? []).length;
    return cjk + latin;
  }, [content]);

  if (detailLoading && !detail) {
    return (
      <div className="space-y-3 p-6">
        <Skeleton className="h-6 w-1/2" />
        <Skeleton className="h-64 w-full" />
      </div>
    );
  }

  if (!detail) {
    return (
      <EmptyState
        icon={FileText}
        title="无法打开这篇笔记"
        description="笔记可能已被删除或移动。请从列表中重新选择。"
        className="flex-1"
      />
    );
  }

  const note = detail.note;
  const readOnly = note.status === 'deleted';

  const showPath = async () => {
    try {
      await navigator.clipboard.writeText(note.filePath);
      toast.success('已复制相对路径');
    } catch {
      toast.error('复制失败');
    }
  };

  const reveal = async () => {
    if (!isTauriRuntime()) {
      toast.info('浏览器预览模式不支持打开目录');
      return;
    }
    const root = await useWorkspaceStore.getState().workspace?.rootPath;
    if (!root) return;
    await revealItemInDir(`${root}/${note.filePath}`);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-2 border-b border-[var(--border-subtle)] px-3 py-2">
        <div className="flex items-center gap-0.5 rounded-md border border-[var(--border-subtle)] p-0.5">
          {(
            [
              { value: 'edit', label: '编辑', icon: Pencil },
              { value: 'split', label: '分屏', icon: Columns2 },
              { value: 'preview', label: '预览', icon: Eye },
            ] as const
          ).map((option) => (
            <button
              key={option.value}
              type="button"
              title={option.label}
              onClick={() => changeEditorMode(option.value)}
              className={cn(
                'flex items-center gap-1 rounded px-2 py-1 text-xs transition-colors',
                editorMode === option.value
                  ? 'bg-[var(--brand-soft)] text-[var(--brand)]'
                  : 'text-[var(--ink-subtle)] hover:text-[var(--ink)]',
              )}
            >
              <option.icon className="h-3.5 w-3.5" />
              <span className="hidden sm:inline">{option.label}</span>
            </button>
          ))}
        </div>

        <Button
          size="sm"
          variant="ghost"
          onClick={() => void saveNow()}
          disabled={readOnly || saveStatus === 'saving'}
        >
          <Save className="h-3.5 w-3.5" />
          保存
        </Button>
        <SaveStatusBadge status={saveStatus} detail={saveError} />

        <div className="ml-auto flex items-center gap-1">
          <Button
            size="icon"
            variant="ghost"
            title={note.favorite ? '取消收藏' : '收藏'}
            onClick={() => void toggleFavorite(note.id)}
          >
            {note.favorite ? (
              <Star className="h-4 w-4 text-amber-500" />
            ) : (
              <StarOff className="h-4 w-4" />
            )}
          </Button>
          <Button
            size="icon"
            variant="ghost"
            title="移动分类"
            onClick={() => setMoveOpen(true)}
            disabled={readOnly}
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
              <DropdownMenuItem onSelect={() => setRenameOpen(true)} disabled={readOnly}>
                重命名 / 重命名文件
              </DropdownMenuItem>
              <DropdownMenuItem
                onSelect={() => void duplicate(note.id).then(() => toast.success('已创建副本'))}
              >
                创建副本
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => void showPath()}>复制相对路径</DropdownMenuItem>
              <DropdownMenuItem onSelect={() => void reveal()}>在文件夹中显示</DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem onSelect={() => setExportOpen(true)}>
                <Download className="h-3.5 w-3.5" /> 导出（Markdown / HTML / Word / PDF）
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              {note.status === 'deleted' ? (
                <>
                  <DropdownMenuItem
                    onSelect={() =>
                      void restore(note.id).then(() => {
                        toast.success('已恢复');
                        void refreshScope();
                      })
                    }
                  >
                    <Undo2 className="h-3.5 w-3.5" /> 从回收站恢复
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    destructive
                    onSelect={() =>
                      void purge(note.id).then(() => {
                        toast.success('已永久删除');
                        navigate('/trash');
                      })
                    }
                  >
                    <Trash2 className="h-3.5 w-3.5" /> 永久删除
                  </DropdownMenuItem>
                </>
              ) : (
                <DropdownMenuItem
                  destructive
                  onSelect={() =>
                    void remove(note.id).then(() => {
                      toast.success('已移入回收站');
                      navigate('/notes');
                    })
                  }
                >
                  <Trash2 className="h-3.5 w-3.5" /> 移到回收站
                </DropdownMenuItem>
              )}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>

      <div className="space-y-2 border-b border-[var(--border-subtle)] px-4 py-3">
        <Input
          value={title}
          readOnly={readOnly}
          placeholder="笔记标题"
          className="h-auto border-none bg-transparent px-0 text-lg font-semibold focus-visible:ring-0"
          onChange={(event) => {
            setTitle(event.target.value);
            markDirty();
          }}
        />
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-[11px] text-[var(--ink-subtle)]">
          <span className="flex items-center gap-1">
            <FolderOpen className="h-3 w-3" />
            {detail.categoryPath || 'Inbox'} · {note.filePath}
          </span>
          <span>更新于 {formatDateTime(note.updatedAt)}</span>
          <span>{words} 字</span>
          {note.status === 'deleted' ? (
            <span className="text-red-500">位于回收站（只读）</span>
          ) : null}
          {note.lastIndexedAt ? (
            <span className="flex items-center gap-1 text-emerald-600">
              <Check className="h-3 w-3" />
              已索引 {formatRelative(note.lastIndexedAt)}
            </span>
          ) : null}
        </div>
        <TagEditor
          tags={tags}
          onChange={(next) => {
            setTags(next);
            markDirty();
          }}
        />
      </div>

      <div ref={attachSplitContainer} className="flex min-h-0 flex-1">
        {editorMode !== 'preview' ? (
          <div
            ref={editorWrapper}
            data-testid="editor-pane"
            className={cn(
              'min-w-0 overflow-hidden',
              editorMode === 'split' ? 'shrink-0' : 'flex-1',
            )}
            style={editorMode === 'split' ? { width: `${editorSplitRatio * 100}%` } : undefined}
          >
            <MarkdownEditor
              value={content}
              theme={theme}
              fontSize={fontSize}
              readOnly={readOnly}
              onChange={(value) => {
                setContent(value);
                markDirty();
              }}
            />
          </div>
        ) : null}
        {editorMode === 'split' ? (
          <ResizeHandle
            side="left"
            label="调整编辑 / 预览比例"
            value={editorSplitRatio}
            min={0.3}
            max={0.7}
            scale={splitWidth > 0 ? 1 / splitWidth : 0}
            onChange={setEditorSplitRatio}
            onCommit={persistLayoutPreferences}
            onReset={() => setEditorSplitRatio(LAYOUT_DEFAULTS.editorSplitRatio)}
          />
        ) : null}
        {editorMode !== 'edit' ? (
          <div data-testid="preview-pane" className="min-w-0 flex-1 overflow-auto">
            <MarkdownPreview content={content} />
          </div>
        ) : null}
      </div>

      <ConflictDialog onKeepLocal={() => setForceOverwrite(true)} />
      {moveOpen ? (
        <MoveNoteDialog
          noteId={note.id}
          currentCategoryId={note.categoryId}
          onClose={() => setMoveOpen(false)}
        />
      ) : null}
      {renameOpen ? (
        <RenameFileDialog
          noteId={note.id}
          initialTitle={note.title}
          onClose={() => setRenameOpen(false)}
        />
      ) : null}
      {exportOpen ? (
        <ExportNoteDialog
          noteId={note.id}
          title={note.title}
          onClose={() => setExportOpen(false)}
        />
      ) : null}
    </div>
  );
}
