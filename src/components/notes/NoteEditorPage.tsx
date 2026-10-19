import {
  Check,
  ChevronRight,
  Columns2,
  AlertTriangle,
  BookMarked,
  Eye,
  FileText,
  FolderOpen,
  GitBranch,
  Link2,
  Pencil,
  Save,
  Unlink2,
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
import { reportAction, reportFailure } from '@/lib/actionFeedback';
import { TagEditor } from '@/components/editor/TagEditor';
import { ResizeHandle } from '@/components/layout/ResizeHandle';
import { MoveNoteDialog } from '@/components/notes/MoveNoteDialog';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { Skeleton } from '@/components/ui/badge';
import { useAutosave } from '@/hooks/useAutosave';
import { codepointRangeToUtf16 } from '@/lib/highlightRange';
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
import { editorContentBelongsTo } from '@/components/notes/editorContentGuard';
import { registerEditorFlush } from '@/stores/notesStore';

import { LAYOUT_DEFAULTS, useUiStore } from '@/stores/uiStore';
import { InsertBlockMenu } from '@/components/editor/InsertBlockMenu';
import { MarkdownFormatBar } from '@/components/editor/MarkdownFormatBar';
import {
  changeIndent,
  insertBlock,
  setHeading,
  toggleLinePrefix,
  wrapSelection,
  type EditResult,
  type EditState,
} from '@/lib/markdownCommands';
import { useOutlineStore } from '@/stores/outlineStore';
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
  const save = useNotesStore((state) => state.save);
  const markDirty = useNotesStore((state) => state.markDirty);

  const settings = useWorkspaceStore((state) => state.settings);
  const theme = useUiStore((state) => state.theme);
  const editorMode = useUiStore((state) => state.editorMode);
  const setEditorMode = useUiStore((state) => state.setEditorMode);
  const fontSize = useUiStore((state) => state.fontSize);
  const editorSplitRatio = useUiStore((state) => state.editorSplitRatio);
  const setEditorSplitRatio = useUiStore((state) => state.setEditorSplitRatio);
  const pendingHighlight = useUiStore((state) => state.pendingHighlight);
  const setPendingHighlight = useUiStore((state) => state.setPendingHighlight);

  const [content, setContent] = useState('');
  const [title, setTitle] = useState('');
  const [tags, setTags] = useState<string[]>([]);
  /**
   * 对话框的开关**直接由 store 里的动作请求派生**（2026-10-17）。
   *
   * 为什么不本地 useState + effect 里 setState：React 的 lint 规则不许在 effect 里同步
   * setState（会级联渲染）。请求本身就是"要开哪个对话框"，派生出来最直接，也少一层状态。
   */
  const noteActionRequest = useUiStore((state) => state.noteActionRequest);
  const clearNoteAction = useUiStore((state) => state.clearNoteAction);
  const closeDialog = useCallback(() => clearNoteAction(), [clearNoteAction]);
  const [relations, setRelations] = useState<RelationView | null>(null);
  const [forceOverwrite, setForceOverwrite] = useState(false);
  /**
   * 编辑器实例代号：历史版本还原 / 外部编辑同步后自增，
   * 让 CodeMirror 重新挂载。@uiw/react-codemirror 在「刚输入过」的窗口内
   * 会把外部 value 变更挂起到输入结束后，导致还原后界面仍停在旧内容。
   */
  const [editorEpoch, setEditorEpoch] = useState(0);
  /**
   * 底部「关系 / 反向链接」默认收成**一行摘要**（2026-10-19 第二轮）。
   *
   * 上一轮把整块限高 42% 后，默认仍有 ~167px：关系一行、反链一行、空提示一行、未解析一行。
   * 用户希望继续压缩。现在默认只留 32px 摘要（关系出入度、反链数、未解析数），
   * 需要看细节再点「展开」；展开后仍受 42% 上限约束、内部滚动。
   */
  const [bottomOpen, setBottomOpen] = useState(false);
  const [unresolvedOpen, setUnresolvedOpen] = useState(false);
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
   * 状态放在 uiStore 里：开关现在在**笔记列表栏头部**，那边也要能开关右侧这一列。
   */
  const outlineMode = useUiStore((state) => state.outlineMode);
  const setOutlineMode = useUiStore((state) => state.setOutlineMode);
  /** mini 态下鼠标是否悬停在贴边条上（临时展开） */
  const [outlineHover, setOutlineHover] = useState(false);
  /** 当前光标所在标题的偏移，用于大纲高亮 */
  const [activeHeadingOffset, setActiveHeadingOffset] = useState<number | null>(null);
  const editorViewRef = useRef<ReactCodeMirrorRef | null>(null);

  const signature = `${content}\u0000${title}\u0000${tags.join(',')}`;

  /**
   * 复制相对路径 / 在文件夹中显示（2026-10-17）。
   *
   * 从 store **现取**当前笔记，而不是用组件里的 `note`：这两个函数要放在 early return
   * **之前**才能被下面的 effect 依赖（hooks 不能在条件分支后面调用）。
   */
  const showPath = useCallback(async () => {
    const current = useNotesStore.getState().detail?.note;
    if (!current) return;
    try {
      await navigator.clipboard.writeText(current.filePath);
      toast.success('已复制相对路径');
    } catch {
      toast.error('复制失败');
    }
  }, []);

  const reveal = useCallback(async () => {
    const current = useNotesStore.getState().detail?.note;
    if (!current) return;
    if (!isTauriRuntime()) {
      toast.info('浏览器预览模式不支持打开目录');
      return;
    }
    const root = await useWorkspaceStore.getState().workspace?.rootPath;
    if (!root) return;
    await revealItemInDir(`${root}/${current.filePath}`);
  }, []);

  useEffect(() => {
    // 打开对话框的请求由上面的派生值处理；这里只管**立即执行**的那些动作
    if (
      !noteActionRequest ||
      ['move', 'rename', 'history', 'source', 'template', 'export'].includes(noteActionRequest)
    ) {
      return;
    }
    clearNoteAction();
    const current = useNotesStore.getState().detail?.note;
    if (!current) return;
    const notes = useNotesStore.getState();
    switch (noteActionRequest) {
      case 'copyPath':
        void showPath();
        break;
      case 'reveal':
        void reveal();
        break;
      case 'duplicate':
        void notes.duplicate(current.id).then((ok) =>
          reportAction(ok, '已创建副本', useNotesStore.getState().error, '创建副本失败'),
        );
        break;
      case 'delete':
        void notes.remove(current.id).then((ok) => {
          reportAction(ok, '已移入回收站', useNotesStore.getState().error, '移入回收站失败');
          if (ok) navigate('/notes');
        });
        break;
      case 'restore':
        void notes.restore(current.id).then((ok) => {
          reportAction(ok, '已恢复', useNotesStore.getState().error, '恢复失败');
          if (ok) void useNotesStore.getState().refresh();
        });
        break;
      case 'purge':
        void notes.purge(current.id).then((ok) => {
          reportAction(ok, '已永久删除', useNotesStore.getState().error, '永久删除失败');
          if (ok) navigate('/trash');
        });
        break;
    }
  }, [
    noteActionRequest,
    clearNoteAction,
    navigate,
    showPath,
    reveal,
  ]);

  const bumpLinkRevision = useCallback(() => setLinkRevision((value) => value + 1), []);

  const headings = useMemo(() => parseOutline(content), [content]);
  const activeNoteId = detail?.note.id ?? null;

  /**
   * 大纲放哪（2026-10-14 布局调整）：`list` = 与笔记列表共用一栏（默认，省一列横向空间）；
   * `side` = 正文右侧独立一列（原来的形态，宽屏时更顺手）。
   * 列表栏那一栏由 `NotesSection` 渲染，所以这里只负责**发布**大纲给 `outlineStore`。
   */
  const outlinePlacement = useUiStore((state) => state.outlinePlacement);
  const setOutlinePlacement = useUiStore((state) => state.setOutlinePlacement);
  const setListPaneMode = useUiStore((state) => state.setListPaneMode);
  const autoOutlineOnSelect = useUiStore((state) => state.autoOutlineOnSelect);
  const publishOutline = useOutlineStore((state) => state.publish);
  const clearOutline = useOutlineStore((state) => state.clear);

  /** 稳定的跳转入口：订阅方拿到的是同一个引用，不会因为每次渲染而重渲染 */
  const outlineJumpRef = useRef<(offset: number) => void>(() => {});
  const publishJump = useCallback((offset: number) => outlineJumpRef.current(offset), []);

  /** 粘贴 / 拖入图片：落盘到 attachments/ 并把相对路径插进正文（V1.1 M2-1）。 */
  const handleAttachFiles = useCallback(
    async (files: File[], kind: 'image' | 'file' = 'image'): Promise<string | null> => {
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
          snippets.push(
            kind === 'image'
              ? buildImageMarkdown(saved.linkPath, attachmentAltText(file.name))
              : // 「附件」插的是普通链接（不是内联图）：图片以外的东西内联显示没有意义
                `[${file.name || saved.linkPath}](${saved.linkPath})`,
          );
        } catch (error) {
          reportFailure(kind === 'image' ? '保存图片失败' : '保存附件失败', errorMessage(error));
        }
      }
      if (snippets.length === 0) return null;
      toast.success(
        kind === 'file'
          ? `附件已保存到 attachments/（${snippets.length} 个）`
          : snippets.length === 1
            ? '图片已保存到 attachments/'
            : `已保存 ${snippets.length} 张图片到 attachments/`,
      );
      return snippets.join('\n');
    },
    [activeNoteId],
  );

  /**
   * 编辑命令的统一落点（2026-10-16）：工具栏 / 插入菜单都算好 `changes`，
   * 这里把它们 dispatch 进 CodeMirror。**view 是唯一事实来源**——光标在哪、选了什么都问它。
   */
  const applyEdit = useCallback((compute: (state: EditState) => EditResult) => {
    const view = editorViewRef.current?.view;
    if (!view) return;
    const { from, to } = view.state.selection.main;
    const result = compute({ value: view.state.doc.toString(), from, to });
    view.dispatch({
      changes: result.changes,
      selection: { anchor: result.selectionFrom, head: result.selectionTo },
    });
    view.focus();
  }, []);

  /** 插入菜单里的「图片…」「附件…」：走隐藏的文件选择框 + 应用自己的附件流程 */
  const attachInputRef = useRef<HTMLInputElement | null>(null);
  const attachKindRef = useRef<'image' | 'file'>('image');
  const pickAttachment = useCallback((kind: 'image' | 'file') => {
    attachKindRef.current = kind;
    attachInputRef.current?.click();
  }, []);


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
   * N5：从搜索结果跳进来时，把命中的**精确区间**选中并滚到那里。
   *
   * 后端给的是**码点**下标，CodeMirror 要 **UTF-16** 下标 —— 这里显式换算
   * （含 emoji 时两者会差位，`codepointRangeToUtf16` 有单测钉住）。
   * 消费一次就清空待办，避免用户继续编辑时又被拽回去。
   */
  useEffect(() => {
    if (!pendingHighlight) return;
    if (!detail || detail.note.id !== pendingHighlight.noteId) return;
    const view = editorViewRef.current?.view;
    if (!view) return;
    const { from, to } = codepointRangeToUtf16(
      content,
      pendingHighlight.charStart,
      pendingHighlight.charEnd,
    );
    view.dispatch({
      selection: to > from ? { anchor: from, head: to } : { anchor: from },
      effects: EditorView.scrollIntoView(from, { y: 'start' }),
    });
    view.focus();
    setPendingHighlight(null);
  }, [pendingHighlight, detail, content, setPendingHighlight]);

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

  useEffect(() => {
    if (!activeNoteId) return;
    // 发布大纲给「笔记列表那一栏」（见 stores/outlineStore.ts 的说明）。
    // 跳转函数放进 ref：订阅方拿到的是稳定引用，同时这里（effect 里）更新它也不算"渲染期写 ref"。
    outlineJumpRef.current = (offset) =>
      jumpFromOutline(offset, headings.findIndex((item) => item.offset === offset));
    publishOutline({
      noteId: activeNoteId,
      title,
      headings,
      activeOffset: activeHeadingOffset,
      jump: publishJump,
    });
  }, [
    activeNoteId,
    title,
    headings,
    activeHeadingOffset,
    publishOutline,
    publishJump,
    jumpFromOutline,
  ]);

  useEffect(() => {
    if (!activeNoteId) return;
    return () => clearOutline(activeNoteId);
  }, [activeNoteId, clearOutline]);

  /**
   * 可选：选中笔记后自动把列表栏切到大纲。
   * 默认**关**——浏览列表时连着点几篇，每次都被切走会很烦；想要"选中即看结构"的人可以打开。
   */
  useEffect(() => {
    if (!activeNoteId) return;
    if (outlinePlacement === 'list' && autoOutlineOnSelect) setListPaneMode('outline');
  }, [activeNoteId, outlinePlacement, autoOutlineOnSelect, setListPaneMode]);

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

  const { saveNow, flush, resetBaseline } = useAutosave({
    enabled: settings?.autosaveEnabled ?? false,
    delay: settings?.autosaveDelayMs ?? 1000,
    signature: detail ? signature : null,
    onSave: async () => {
      if (!detail) return false;
      // 切笔记的中间帧里会短暂出现「detail 已经是新笔记、content/title 还停在上一篇」：
      // 此时若落盘，就会把上一篇的内容写进**新笔记**（真机验收里四篇笔记被写空就是这条路径）。
      // 只有编辑器确实装载过这篇的内容（loadedId 与之相等）才允许保存。
      if (!editorContentBelongsTo(loadedId.current, detail.note.id)) return false;
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

  /** 把「立即落盘」注册给 store：切换笔记时它会先调用这里（自动保存有防抖窗口） */
  const flushRef = useRef(flush);
  useEffect(() => {
    // 在 effect 里同步最新实现（不要在渲染期间写 ref）
    flushRef.current = flush;
  }, [flush]);
  useEffect(() => {
    registerEditorFlush(() => flushRef.current());
    return () => {
      registerEditorFlush(null);
      // 离开编辑器前再保险一次：防抖窗口里的改动不能就这么丢
      void flushRef.current();
    };
  }, []);

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
  const visibleCounts = linkCounts && countsFor === note.id ? linkCounts : null;
  const relationOut = relations?.outgoing.length ?? 0;
  const relationIn = relations?.incoming.length ?? 0;


  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-2 border-b border-[var(--border-subtle)] px-3 py-2">
        {/* 插入菜单（2026-10-16）：放在编辑 / 分屏 / 预览**左边**，只在有编辑器的模式下可用 */}
        <InsertBlockMenu
          disabled={readOnly || editorMode === 'preview'}
          onInsert={(block) => applyEdit((state) => insertBlock(state, block))}
          onAttach={pickAttachment}
        />
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
        {/*
          插入菜单里「图片…／附件…」用的隐藏文件选择框（2026-10-18 补回）：
          上一轮重写工具栏那一段时**把它一起删掉了**，于是点「图片…」什么都不发生
          （文件选择框根本不弹）。E2E 补了断言：点菜单里的「图片…」必须真的弹出文件选择框。
        */}
        <input
          ref={attachInputRef}
          type="file"
          multiple
          className="hidden"
          data-testid="insert-file-input"
          onChange={(event) => {
            const files = Array.from(event.target.files ?? []);
            event.target.value = '';
            void handleAttachFiles(files, attachKindRef.current).then((snippet) => {
              if (snippet) applyEdit((state) => insertBlock(state, snippet));
            });
          }}
        />
        {/* 保存状态只留顶栏那一个（2026-10-16 按用户反馈）：
            这里再显示一次「已同步」，同一件事在屏幕上出现两遍，纯属噪音 */}

        {/*
          文本格式条并进**同一行**、靠右侧（2026-10-17 按用户要求）：
          以前它独占第二行、把编辑区往下推；现在它贴在工具栏右端，
          宽度不够时由它自己的「…」菜单兜住（见 MarkdownFormatBar 的 compact 变体）。
          收藏 / 移动分类 / 更多操作 三个按钮已搬到顶栏（NoteActions）。
        */}
        {/* 保存与格式条之间**硬间隔**：留出两个「保存」按钮的宽度，格式条不会越过它 */}
        <div className="min-w-[132px] shrink-0" aria-hidden />
        <div className="flex min-w-0 flex-1 items-center gap-1.5">
          {editorMode !== 'preview' ? (
            <MarkdownFormatBar
              disabled={readOnly}
              onWrap={(before, after, placeholder) =>
                applyEdit((state) => wrapSelection(state, before, after, placeholder))
              }
              onLinePrefix={(prefix) => applyEdit((state) => toggleLinePrefix(state, prefix))}
              onHeading={(level) => applyEdit((state) => setHeading(state, level))}
              onIndent={(delta) => applyEdit((state) => changeIndent(state, delta))}
            />
          ) : null}
        </div>
      </div>

      {/* 标题 + 元信息（2026-10-19）：非全屏时这一块也在抢正文高度，内外边距各收一档 */}
      <div
        data-testid="editor-header"
        className="space-y-1 border-b border-[var(--border-subtle)] px-4 py-1.5"
      >
        <Input
          aria-label="笔记标题"
          value={title}
          readOnly={readOnly}
          placeholder="笔记标题"
          className="h-8 border-none bg-transparent px-0 text-[17px] font-semibold focus-visible:ring-0"
          onChange={(event) => {
            setTitle(event.target.value);
            markDirty();
          }}
        />
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-[var(--ink-subtle)]">
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
              // 无障碍名给个短的：这个按钮的内容是一串元信息（Inbox · 路径 · 更新时间…），
              // 读屏会把整串念出来；顺带也让自动化能按名字点到它（真机验收里要改有效期）
              aria-label="来源与时效"
              // 元信息栏这个入口也走 store 请求（与顶栏「更多操作」同一套开关）
              onClick={() => useUiStore.getState().requestNoteAction('source')}
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
          {/* 标签并进元信息这一行：没在编辑时只有一个「+ 标签」，不再独占一行 */}
          <span className="ml-auto flex items-center">
            <TagEditor
              inline
              tags={tags}
              onChange={(next) => {
                setTags(next);
                markDirty();
              }}
            />
          </span>
        </div>
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
        {/* 右侧独立一列的大纲：只在「钉在右侧」这种位置下渲染（默认位置是与列表共用一栏） */}
        {outlinePlacement === 'side' && outlineMode === 'pinned' ? (
          <OutlinePanel
            key={note.id}
            headings={headings}
            activeOffset={activeHeadingOffset}
            onJump={(offset) => jumpFromOutline(offset, headings.findIndex((item) => item.offset === offset))}
            onCollapse={() => setOutlineMode('mini')}
            onMoveToList={() => {
              setOutlinePlacement('list');
              // 位置是**工作区级偏好**：用户从面板上改的也要落库，否则重启就丢
              persistLayoutPreferences();
            }}
          />
        ) : null}
        {outlinePlacement === 'side' && outlineMode === 'mini' ? (
          /*
           * 贴边条用真正的 <button>（2026-10-15 界面梳理）：它本来就是可点的（点一下展开），
           * 但以前是个 div——键盘用户到不了，无障碍树里也只有里面的文字、没有它自己的框
           * （验收脚本想"把鼠标移到这一条上"时，取到的是文字的中心，正落在窗口边界上）。
           */
          <button
            type="button"
            data-testid="outline-mini"
            aria-label="大纲（鼠标移上去展开）"
            title="大纲（鼠标移上去展开）"
            className="flex w-3.5 shrink-0 cursor-pointer items-center justify-center border-l border-[var(--border-subtle)] bg-[var(--surface)]"
            onMouseEnter={() => setOutlineHover(true)}
            onMouseLeave={() => setOutlineHover(false)}
            onClick={() => setOutlineHover(true)}
          >
            <span className="select-none text-[10px] tracking-widest text-[var(--ink-subtle)] [writing-mode:vertical-rl]">
              大纲
            </span>
          </button>
        ) : null}
        {/* mini 态：悬浮展开的大纲（绝对定位，不挤压正文） */}
        {outlinePlacement === 'side' && outlineMode === 'mini' && outlineHover ? (
          <div
            data-testid="outline-overlay"
            className="absolute right-0 top-0 z-20 h-full shadow-2xl"
            onMouseEnter={() => setOutlineHover(true)}
            onMouseLeave={() => setOutlineHover(false)}
          >
            <OutlinePanel
              key={`mini-${note.id}`}
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

      {/*
        底部信息区（2026-10-19 第二轮）：
        - 默认只留 **一行摘要**（关系出入度 / 反链数 / 未解析数），把纵向空间让给正文；
        - 点「展开」后才渲染关系与反向链接面板，并且整块最多占 42%、内部滚动；
        - 「未解析 N」可以直接展开到未解析列表，不用再点第二次。
      */}
      <div
        data-testid="editor-bottom-panels"
        className="flex max-h-[42%] min-h-0 shrink-0 flex-col"
      >
        <div className="flex h-8 shrink-0 items-center gap-3 border-t border-[var(--border-subtle)] pl-1 pr-3 text-[11px] text-[var(--ink-subtle)]">
          <button
            type="button"
            data-testid="editor-bottom-toggle"
            aria-expanded={bottomOpen}
            className="flex items-center gap-1 rounded py-0.5 pl-0 pr-1 transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]"
            title={bottomOpen ? '收起关系与反向链接' : '展开关系与反向链接'}
            onClick={() => setBottomOpen((value) => !value)}
          >
            <ChevronRight
              className={cn('h-3 w-3 transition-transform', bottomOpen && 'rotate-90')}
            />
            <GitBranch className="h-3 w-3" />
            关系
            <span className="text-[var(--ink-muted)]">
              出 {relationOut} · 入 {relationIn}
            </span>
          </button>
          <span className="flex items-center gap-1">
            <Link2 className="h-3 w-3" />
            反向链接 {visibleCounts?.backlinks ?? 0}
          </span>
          {visibleCounts && visibleCounts.unresolved > 0 ? (
            <button
              type="button"
              data-testid="editor-bottom-unresolved"
              className="flex items-center gap-1 rounded px-1 py-0.5 text-amber-600 transition-colors hover:bg-amber-500/10"
              title="展开并查看本篇未解析链接"
              onClick={() => {
                setBottomOpen(true);
                setUnresolvedOpen(true);
              }}
            >
              <Unlink2 className="h-3 w-3" />
              未解析 {visibleCounts.unresolved}
            </button>
          ) : null}
          <button
            type="button"
            className="ml-auto rounded px-1 py-0.5 transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]"
            onClick={() => setBottomOpen((value) => !value)}
          >
            {bottomOpen ? '收起' : '展开'}
          </button>
        </div>

        {bottomOpen ? (
          <div
            data-testid="editor-bottom-details"
            className="flex min-h-0 flex-1 flex-col overflow-y-auto"
          >
            <RelationsPanel
              noteId={note.id}
              refreshKey={linkRevision}
              onChanged={bumpLinkRevision}
              embedded
            />

            <BacklinkPanel
              noteId={note.id}
              refreshKey={linkRevision}
              unresolvedOpen={unresolvedOpen}
              onUnresolvedOpenChange={setUnresolvedOpen}
              embedded
            />
          </div>
        ) : null}
      </div>

      <ConflictDialog onKeepLocal={() => setForceOverwrite(true)} />
      {noteActionRequest === 'move' ? (
        <MoveNoteDialog
          noteId={note.id}
          currentCategoryId={note.categoryId}
          onClose={closeDialog}
        />
      ) : null}
      {noteActionRequest === 'rename' ? (
        <RenameFileDialog
          noteId={note.id}
          initialTitle={note.title}
          onClose={() => {
            closeDialog();
            bumpLinkRevision();
          }}
        />
      ) : null}
      {noteActionRequest === 'source' ? (
        <SourceDialog
          note={note}
          onClose={closeDialog}
          onSaved={() => {
            closeDialog();
            void useNotesStore.getState().openDetail(note.id);
          }}
        />
      ) : null}
      {noteActionRequest === 'history' ? (
        <HistoryDialog
          noteId={note.id}
          currentContent={content}
          onClose={closeDialog}
          onRestored={closeDialog}
        />
      ) : null}
      {noteActionRequest === 'template' ? (
        <SaveTemplateDialog
          defaultName={note.title}
          content={content.replace(/^---[\s\S]*?---\n/, '')}
          onClose={closeDialog}
        />
      ) : null}
      {noteActionRequest === 'export' ? (
        <ExportNoteDialog noteId={note.id} title={note.title} onClose={closeDialog} />
      ) : null}
    </div>
  );
}
