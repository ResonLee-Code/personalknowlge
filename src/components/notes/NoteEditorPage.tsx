import {
  Check,
  Columns2,
  Download,
  AlertTriangle,
  BookMarked,
  Eye,
  FileText,
  FolderInput,
  FolderOpen,
  GitBranch,
  History,
  Link2,
  ListTree,
  MoreHorizontal,
  Pencil,
  Save,
  Star,
  StarOff,
  Trash2,
  Unlink2,
  Undo2,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'sonner';

import { BacklinkPanel } from '@/components/editor/BacklinkPanel';
import { ConflictDialog } from '@/components/editor/ConflictDialog';
import { MarkdownEditor } from '@/components/editor/MarkdownEditor';
import { MarkdownPreview } from '@/components/editor/MarkdownPreview';
import { OutlinePanel } from '@/components/editor/OutlinePanel';
import { RelationsPanel } from '@/components/editor/RelationsPanel';
import { HistoryDialog } from '@/components/editor/HistoryDialog';
import { RenameFileDialog } from '@/components/editor/RenameFileDialog';
import { SaveTemplateDialog } from '@/components/editor/SaveTemplateDialog';
import { ExportNoteDialog } from '@/components/notes/ExportNoteDialog';
import { SourceDialog } from '@/components/editor/SourceDialog';
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
import {
  attachmentAltText,
  buildImageMarkdown,
  mimeFromDataUrl,
  readFileAsDataUrl,
} from '@/lib/attachment';
import { ROUTES } from '@/lib/constants';
import { errorMessage, formatDateTime, formatRelative } from '@/lib/format';
import { freshnessState, reliabilityStars, sourceTypeLabel } from '@/lib/freshness';
import { parseOutline } from '@/lib/markdown';
import { relationHighlights } from '@/lib/relations';
import { cn } from '@/lib/utils';
import { attachmentService } from '@/services/attachmentService';
import { linkService } from '@/services/linkService';
import { relationService } from '@/services/relationService';
import { noteService } from '@/services/noteService';
import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { isTauriRuntime } from '@/services/api';
import { useNotesStore } from '@/stores/notesStore';
import { LAYOUT_DEFAULTS, useUiStore } from '@/stores/uiStore';
import { persistLayoutPreferences, useWorkspaceStore } from '@/stores/workspaceStore';
import type { LinkCounts, RelationView } from '@/types';
import { EditorView } from '@codemirror/view';
import type { ReactCodeMirrorRef } from '@uiw/react-codemirror';

/** 笔记编辑器（设计文档 16.5）。 */
export function NoteEditorPage() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
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
  const [sourceOpen, setSourceOpen] = useState(false);
  const [saveTemplateOpen, setSaveTemplateOpen] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [relations, setRelations] = useState<RelationView | null>(null);
  const [forceOverwrite, setForceOverwrite] = useState(false);
  /**
   * 编辑器实例代号：历史版本还原 / 外部编辑同步后自增，
   * 让 CodeMirror 重新挂载。@uiw/react-codemirror 在「刚输入过」的窗口内
   * 会把外部 value 变更挂起到输入结束后，导致还原后界面仍停在旧内容。
   */
  const [editorEpoch, setEditorEpoch] = useState(0);
  const loadedId = useRef<string | null>(null);
  const baseHash = useRef<string | null>(null);
  const editorWrapper = useRef<HTMLDivElement | null>(null);
  /** 预览区容器：纯预览模式下按标题序号滚动用 */
  const previewPaneRef = useRef<HTMLDivElement | null>(null);
  const [splitWidth, setSplitWidth] = useState(0);
  const splitObserver = useRef<ResizeObserver | null>(null);
  const [linkCounts, setLinkCounts] = useState<LinkCounts | null>(null);
  /** 引用计数对应的笔记 id，避免切换笔记瞬间显示上一篇的计数 */
  const [countsFor, setCountsFor] = useState<string | null>(null);
  /** 保存 / 重命名后递增，驱动反向链接面板重新拉取派生数据 */
  const [linkRevision, setLinkRevision] = useState(0);
  /**
   * 大纲形态（对标语雀）：hidden = 不显示；pinned = 固定在右侧；
   * mini = 贴边收起（鼠标移上去临时悬浮展开，可「钉住大纲」固定）。
   */
  // 大纲默认固定在正文右侧（对标语雀：右栏常驻）；可「收起」为贴边 mini 态
  const [outlineMode, setOutlineMode] = useState<'hidden' | 'pinned' | 'mini'>('pinned');
  /** mini 态下鼠标是否悬停在贴边条上（临时展开） */
  const [outlineHover, setOutlineHover] = useState(false);
  /** 当前光标所在标题的偏移，用于大纲高亮 */
  const [activeHeadingOffset, setActiveHeadingOffset] = useState<number | null>(null);
  const editorViewRef = useRef<ReactCodeMirrorRef | null>(null);

  const signature = `${content}\u0000${title}\u0000${tags.join(',')}`;

  const bumpLinkRevision = useCallback(() => setLinkRevision((value) => value + 1), []);

  const headings = useMemo(() => parseOutline(content), [content]);
  const activeNoteId = detail?.note.id ?? null;

  /** 粘贴 / 拖入图片：落盘到 attachments/ 并把相对路径插进正文（V1.1 M2-1）。 */
  const handleAttachFiles = useCallback(
    async (files: File[]): Promise<string | null> => {
      const snippets: string[] = [];
      for (const file of files) {
        try {
          const dataUrl = await readFileAsDataUrl(file);
          const saved = await attachmentService.save({
            noteId: activeNoteId,
            fileName: file.name || null,
            mimeType: mimeFromDataUrl(dataUrl) ?? file.type ?? null,
            dataBase64: dataUrl,
          });
          snippets.push(buildImageMarkdown(saved.linkPath, attachmentAltText(file.name)));
        } catch (error) {
          toast.error(errorMessage(error));
        }
      }
      if (snippets.length === 0) return null;
      toast.success(
        snippets.length === 1
          ? '图片已保存到 attachments/'
          : `已保存 ${snippets.length} 张图片到 attachments/`,
      );
      return snippets.join('\n');
    },
    [activeNoteId],
  );

  const jumpToHeading = useCallback((offset: number) => {
    const view = editorViewRef.current?.view;
    if (!view) return;
    view.dispatch({
      selection: { anchor: offset },
      effects: EditorView.scrollIntoView(offset, { y: 'start' }),
    });
    view.focus();
    setActiveHeadingOffset(offset);
  }, []);

  /**
   * 大纲点击：编辑器可用时跳编辑器；纯预览模式（编辑器未挂载）按标题**序号**
   * 滚动预览区 —— 预览没有源码偏移，但标题顺序与 `parseOutline` 一致。
   */
  const jumpFromOutline = useCallback(
    (offset: number, index: number) => {
      if (editorViewRef.current?.view) {
        jumpToHeading(offset);
        return;
      }
      const nodeList = previewPaneRef.current?.querySelectorAll('h1, h2, h3, h4, h5, h6');
      const target = nodeList?.[index];
      if (target instanceof HTMLElement) {
        target.scrollIntoView({ block: 'start', behavior: 'smooth' });
        setActiveHeadingOffset(offset);
      }
    },
    [jumpToHeading],
  );

  // ---------------------------------------------------- 双向链接（V1.1 M2-2）
  const [wikilinkTitles, setWikilinkTitles] = useState<string[]>([]);
  const [wikilinkTargets, setWikilinkTargets] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!activeNoteId) return;
    let cancelled = false;
    void Promise.all([
      noteService
        .list({ status: 'active', sort: 'titleAsc', pageSize: 500 })
        .catch(() => null),
      linkService.outgoing(activeNoteId).catch(() => []),
    ]).then(([list, outgoing]) => {
      if (cancelled) return;
      setWikilinkTitles((list?.items ?? []).map((item) => item.title));
      const map: Record<string, string> = {};
      outgoing.forEach((link) => {
        if (link.targetNoteId) map[link.targetTitle.toLowerCase()] = link.targetNoteId;
      });
      setWikilinkTargets(map);
    });
    return () => {
      cancelled = true;
    };
  }, [activeNoteId, linkRevision]);

  const getWikilinkTitles = useCallback(() => wikilinkTitles, [wikilinkTitles]);
  const resolveWikilink = useCallback(
    (target: string) => wikilinkTargets[target.trim().toLowerCase()] ?? null,
    [wikilinkTargets],
  );
  /** 预览里点击 chip：已存在则跳转，未解析则按标题创建后打开 */
  const openWikilink = useCallback(
    async (target: string) => {
      const existing = wikilinkTargets[target.trim().toLowerCase()];
      if (existing) {
        navigate(ROUTES.note(existing));
        return;
      }
      const created = await useNotesStore.getState().create({ title: target });
      if (created) {
        toast.success(`已创建「${created.note.title}」`);
        navigate(ROUTES.note(created.note.id));
      }
    },
    [navigate, wikilinkTargets],
  );

  const { saveNow, resetBaseline } = useAutosave({
    enabled: settings?.autosaveEnabled ?? false,
    delay: settings?.autosaveDelayMs ?? 1000,
    signature: detail ? signature : null,
    onSave: async () => {
      if (!detail) return false;
      // 磁盘内容已被别处改动（历史版本还原 / 外部编辑器）时不要盲目覆盖：
      // 让下面的同步 effect 把界面拉回磁盘内容，否则会把刚还原的版本又写回旧内容
      const latest = useNotesStore.getState().detail;
      if (latest && latest.note.id === detail.note.id) {
        if (baseHash.current !== (latest.note.contentHash ?? null)) return false;
      }
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
        bumpLinkRevision();
      }
      return ok;
    },
  });

  useEffect(() => {
    if (!detail) return;
    const switchingNote = loadedId.current !== detail.note.id;
    // 同一篇笔记内容哈希变了 = 磁盘内容被别处改过（历史版本还原、外部编辑器、
    // 冲突后重新加载）。此时也要同步，否则界面会停在被覆盖掉的旧内容上。
    const changedOnDisk =
      !switchingNote && baseHash.current !== (detail.note.contentHash ?? null);
    const hasLocalEdits = saveStatus === 'dirty' || saveStatus === 'saving';
    // 用户正在输入时不覆盖他的内容（保存时再走冲突流程）
    if (!switchingNote && (!changedOnDisk || hasLocalEdits)) return;

    loadedId.current = detail.note.id;
    setForceOverwrite(false);
    // 外部变更（非切换笔记）时重建编辑器，确保内容一定刷新到界面
    if (changedOnDisk) setEditorEpoch((value) => value + 1);
    setContent(detail.content);
    setTitle(detail.note.title);
    setTags(detail.tags.map((tag) => tag.name));
    if (switchingNote) setActiveHeadingOffset(null);
    baseHash.current = detail.note.contentHash ?? null;
    resetBaseline(
      `${detail.content}\u0000${detail.note.title}\u0000${detail.tags
        .map((tag) => tag.name)
        .join(',')}`,
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [detail?.note.id, detail?.note.contentHash]);

  /**
   * 从笔记列表的「大纲预览」跳转过来（URL `?h=<偏移量>`）：
   * 滚动到对应标题；纯预览模式先切分屏，等编辑器挂载后再跳。
   */
  const appliedHeading = useRef<string | null>(null);
  useEffect(() => {
    const headingParam = searchParams.get('h');
    const noteId = detail?.note.id;
    if (!headingParam || !noteId || !content) return;
    const key = `${noteId}:${headingParam}`;
    if (appliedHeading.current === key) return;
    // 预览模式下编辑器未挂载，先切到分屏；editorMode 变化会重新触发本 effect
    if (editorMode === 'preview') {
      setEditorMode('split');
      return;
    }
    const offset = Number(headingParam);
    appliedHeading.current = key;
    const frame = requestAnimationFrame(() => jumpToHeading(offset));
    // 清掉参数：刷新页面或切换笔记后不再重复跳转
    const next = new URLSearchParams(searchParams);
    next.delete('h');
    setSearchParams(next, { replace: true });
    return () => cancelAnimationFrame(frame);
  }, [
    searchParams,
    setSearchParams,
    detail?.note.id,
    content,
    editorMode,
    setEditorMode,
    jumpToHeading,
  ]);

  // 关系（N2）：随笔记切换与保存刷新，用于元信息栏角标
  useEffect(() => {
    const noteId = detail?.note.id;
    if (!noteId) return;
    let cancelled = false;
    relationService
      .list(noteId)
      .then((next) => {
        if (!cancelled) setRelations(next);
      })
      .catch(() => {
        if (!cancelled) setRelations(null);
      });
    return () => {
      cancelled = true;
    };
  }, [detail?.note.id, linkRevision]);

  // 引用计数（元信息栏）：随笔记切换与保存刷新
  useEffect(() => {
    const noteId = detail?.note.id;
    if (!noteId) return;
    let cancelled = false;
    linkService
      .counts(noteId)
      .then((next) => {
        if (cancelled) return;
        setLinkCounts(next);
        setCountsFor(noteId);
      })
      .catch(() => {
        // 链接是派生数据，读取失败时隐藏计数即可
      });
    return () => {
      cancelled = true;
    };
  }, [detail?.note.id, linkRevision]);

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
          {editorMode !== 'preview' ? (
            <Button
              size="icon"
              variant="ghost"
              title={outlineMode === 'hidden' ? '显示大纲' : '隐藏大纲'}
              aria-pressed={outlineMode !== 'hidden'}
              className={cn(outlineMode !== 'hidden' && 'text-[var(--brand)]')}
              onClick={() => {
                setOutlineHover(false);
                setOutlineMode((value) => (value === 'hidden' ? 'pinned' : 'hidden'));
              }}
            >
              <ListTree className="h-4 w-4" />
            </Button>
          ) : null}
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
              <DropdownMenuItem onSelect={() => setHistoryOpen(true)} data-testid="note-menu-history">
                <History className="h-3.5 w-3.5" /> 历史版本…
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => setSourceOpen(true)}>
                <BookMarked className="h-3.5 w-3.5" /> 来源与时效
              </DropdownMenuItem>
              <DropdownMenuItem onSelect={() => void showPath()}>复制相对路径</DropdownMenuItem>
              <DropdownMenuItem onSelect={() => void reveal()}>在文件夹中显示</DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                onSelect={() => setSaveTemplateOpen(true)}
                disabled={readOnly}
              >
                <FileText className="h-3.5 w-3.5" /> 另存为模板…
              </DropdownMenuItem>
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
          {linkCounts && countsFor === note.id ? (
            <>
              <span className="flex items-center gap-1" title="有多少篇笔记引用了本篇">
                <Link2 className="h-3 w-3" />被 {linkCounts.backlinks} 篇引用
              </span>
              <span className="flex items-center gap-1" title="本篇引用了多少处笔记">
                引用 {linkCounts.outgoing} 篇
              </span>
              {linkCounts.unresolved > 0 ? (
                <span className="flex items-center gap-1 text-amber-600">
                  <Unlink2 className="h-3 w-3" />
                  {linkCounts.unresolved} 处未解析
                </span>
              ) : null}
            </>
          ) : null}
          {note.status === 'deleted' ? (
            <span className="text-red-500">位于回收站（只读）</span>
          ) : null}
          {note.sourceType || note.sourceRef || note.sourceReliability ? (
            <button
              type="button"
              className="flex items-center gap-1 hover:text-[var(--brand)]"
              title="点击编辑来源与时效"
              onClick={() => setSourceOpen(true)}
            >
              <BookMarked className="h-3 w-3" />
              {sourceTypeLabel(note.sourceType) || '来源'}
              {note.sourceReliability ? (
                <span className="text-amber-500">{reliabilityStars(note.sourceReliability)}</span>
              ) : null}
            </button>
          ) : null}
          {relations
            ? (() => {
                const highlights = relationHighlights(
                  relations.outgoing,
                  relations.incoming,
                );
                return (
                  <>
                    {highlights.supersededBy ? (
                      <span className="flex items-center gap-1 text-amber-600">
                        <GitBranch className="h-3 w-3" />
                        已被「
                        {relations.incoming.find((item) => item.id === highlights.supersededBy?.id)
                          ?.sourceTitle ?? '其它笔记'}
                        」取代
                      </span>
                    ) : null}
                    {highlights.supersedes ? (
                      <span className="text-amber-600">
                        已取代「
                        {relations.outgoing.find((item) => item.id === highlights.supersedes?.id)
                          ?.targetTitle ?? '其它笔记'}
                        」
                      </span>
                    ) : null}
                    {highlights.conflictCount > 0 ? (
                      <span className="text-red-500">{highlights.conflictCount} 处冲突</span>
                    ) : null}
                  </>
                );
              })()
            : null}
          {freshnessState(note.validUntil) === 'expired' ? (
            <span className="flex items-center gap-1 text-amber-600" title={`有效期至 ${note.validUntil}`}>
              <AlertTriangle className="h-3 w-3" />
              可能已过期
            </span>
          ) : freshnessState(note.validUntil) === 'expiring' ? (
            <span className="text-amber-600" title={`有效期至 ${note.validUntil}`}>
              即将到期
            </span>
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

      <div className="relative flex min-h-0 flex-1">
        <div ref={attachSplitContainer} className="flex min-h-0 min-w-0 flex-1">
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
              key={editorEpoch}
              viewRef={editorViewRef}
              value={content}
              theme={theme}
              fontSize={fontSize}
              readOnly={readOnly}
              onAttachFiles={readOnly ? undefined : handleAttachFiles}
              getWikilinkTitles={readOnly ? undefined : getWikilinkTitles}
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
          <div
            ref={previewPaneRef}
            data-testid="preview-pane"
            className="min-w-0 flex-1 overflow-auto"
          >
            <MarkdownPreview
              content={content}
              noteId={note.id}
              resolveWikilink={resolveWikilink}
              onOpenWikilink={(target) => void openWikilink(target)}
            />
          </div>
          ) : null}
        </div>
        {outlineMode === 'pinned' ? (
          <OutlinePanel
            headings={headings}
            activeOffset={activeHeadingOffset}
            onJump={(offset) => jumpFromOutline(offset, headings.findIndex((item) => item.offset === offset))}
            onCollapse={() => setOutlineMode('mini')}
          />
        ) : null}
        {outlineMode === 'mini' ? (
          <div
            data-testid="outline-mini"
            title="大纲（鼠标移上去展开）"
            className="flex w-3.5 shrink-0 cursor-pointer items-center justify-center border-l border-[var(--border-subtle)] bg-[var(--surface)]"
            onMouseEnter={() => setOutlineHover(true)}
            onMouseLeave={() => setOutlineHover(false)}
            onClick={() => setOutlineHover(true)}
          >
            <span className="select-none text-[10px] tracking-widest text-[var(--ink-subtle)] [writing-mode:vertical-rl]">
              大纲
            </span>
          </div>
        ) : null}
        {/* mini 态：悬浮展开的大纲（绝对定位，不挤压正文） */}
        {outlineMode === 'mini' && outlineHover ? (
          <div
            data-testid="outline-overlay"
            className="absolute right-0 top-0 z-20 h-full shadow-2xl"
            onMouseEnter={() => setOutlineHover(true)}
            onMouseLeave={() => setOutlineHover(false)}
          >
            <OutlinePanel
              headings={headings}
              activeOffset={activeHeadingOffset}
              onJump={(offset) => jumpFromOutline(offset, headings.findIndex((item) => item.offset === offset))}
              pinned={false}
              onPin={() => {
                setOutlineHover(false);
                setOutlineMode('pinned');
              }}
            />
          </div>
        ) : null}
      </div>

      <RelationsPanel
        noteId={note.id}
        refreshKey={linkRevision}
        onChanged={bumpLinkRevision}
      />

      <BacklinkPanel noteId={note.id} refreshKey={linkRevision} />

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
          onClose={() => {
            setRenameOpen(false);
            bumpLinkRevision();
          }}
        />
      ) : null}
      {sourceOpen ? (
        <SourceDialog
          note={note}
          onClose={() => setSourceOpen(false)}
          onSaved={() => {
            setSourceOpen(false);
            void useNotesStore.getState().openDetail(note.id);
          }}
        />
      ) : null}
      {historyOpen ? (
        <HistoryDialog
          noteId={note.id}
          currentContent={content}
          onClose={() => setHistoryOpen(false)}
          onRestored={() => setHistoryOpen(false)}
        />
      ) : null}
      {saveTemplateOpen ? (
        <SaveTemplateDialog
          defaultName={note.title}
          content={content.replace(/^---[\s\S]*?---\n/, '')}
          onClose={() => setSaveTemplateOpen(false)}
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
