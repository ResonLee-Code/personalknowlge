import {
  CheckCircle2,
  FileUp,
  FolderOpen,
  Link2,
  Loader2,
  PencilLine,
  TriangleAlert,
  X,
  XCircle,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input, Label, Textarea } from '@/components/ui/input';
import { reportFailure } from '@/lib/actionFeedback';
import { errorMessage } from '@/lib/format';
import { isTauriRuntime } from '@/services/api';
import { pickDirectory, pickMarkdownFiles } from '@/services/appService';
import { clipService } from '@/services/clipService';
import { noteService } from '@/services/noteService';
import { templateService } from '@/services/templateService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { ClippedPage, ImportReport, TemplateSummary } from '@/types';

type Mode = 'create' | 'import' | 'clip';

/** 新建笔记（支持手动创建与导入外部文件）。 */
export function NewNoteDialog() {
  const open = useUiStore((state) => state.newNoteDialogOpen);
  const setOpen = useUiStore((state) => state.setNewNoteDialogOpen);
  // 表单在打开时挂载，关闭即销毁，因此无需在 effect 中重置状态
  if (!open) return null;
  return <NewNotePanel onClose={() => setOpen(false)} />;
}

function NewNotePanel({ onClose }: { onClose: () => void }) {
  const [mode, setMode] = useState<Mode>('create');
  const [categoryId, setCategoryId] = useState('');
  const categories = useCategoriesStore((state) => state.flat);
  const pathOf = useCategoriesStore((state) => state.pathOf);

  return (
    <Dialog open onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent className="w-[min(620px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle>新建笔记</DialogTitle>
          <DialogDescription>
            可以直接写一篇新笔记，也可以把已有的 Markdown / 文本文件导入知识库。
          </DialogDescription>
        </DialogHeader>

        <div className="mb-4 flex items-center gap-1 rounded-md border border-[var(--border-subtle)] p-0.5">
          {(
            [
              { value: 'create', label: '手动创建', icon: PencilLine },
              { value: 'import', label: '导入外部文件', icon: FileUp },
              { value: 'clip', label: '从网页剪藏', icon: Link2 },
            ] as const
          ).map((option) => (
            <button
              key={option.value}
              type="button"
              onClick={() => setMode(option.value)}
              className={[
                'flex flex-1 items-center justify-center gap-1.5 rounded px-3 py-1.5 text-xs transition-colors',
                mode === option.value
                  ? 'bg-[var(--brand-soft)] text-[var(--brand)]'
                  : 'text-[var(--ink-subtle)] hover:text-[var(--ink)]',
              ].join(' ')}
            >
              <option.icon className="h-3.5 w-3.5" />
              {option.label}
            </button>
          ))}
        </div>

        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label htmlFor="new-note-category">目标分类</Label>
            <select
              id="new-note-category"
              value={categoryId}
              onChange={(event) => setCategoryId(event.target.value)}
              className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)]"
            >
              <option value="">Inbox（未分类）</option>
              {categories.map((category) => (
                <option key={category.id} value={category.id}>
                  {pathOf(category.id)}
                </option>
              ))}
            </select>
          </div>

          {mode === 'create' ? (
            <CreateForm categoryId={categoryId} onClose={onClose} />
          ) : mode === 'import' ? (
            <ImportForm categoryId={categoryId} onClose={onClose} />
          ) : (
            <ClipForm categoryId={categoryId} onClose={onClose} />
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}

function CreateForm({ categoryId, onClose }: { categoryId: string; onClose: () => void }) {
  const create = useNotesStore((state) => state.create);
  const navigate = useNavigate();
  const [title, setTitle] = useState('');
  const [tags, setTags] = useState('');
  const [content, setContent] = useState('');
  const [busy, setBusy] = useState(false);
  const [templates, setTemplates] = useState<TemplateSummary[]>([]);
  const [templateName, setTemplateName] = useState('');
  /** 用户是否手动改过正文：改过之后切换标题不再重新套用模板 */
  const [contentEdited, setContentEdited] = useState(false);

  useEffect(() => {
    let cancelled = false;
    // 模板是可选能力：读取失败（例如浏览器预览模式）时静默忽略，不影响手动创建
    void templateService
      .list()
      .then((list) => {
        if (!cancelled) setTemplates(list);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  const applyTemplate = async (name: string, overrideTitle?: string) => {
    setTemplateName(name);
    if (!name) return;
    try {
      const rendered = await templateService.render({
        name,
        title: (overrideTitle ?? title).trim() || '未命名笔记',
      });
      setContent(rendered.content);
      setContentEdited(false);
    } catch (error) {
      reportFailure('载入模板失败', errorMessage(error));
      setTemplateName('');
    }
  };

  const submit = async () => {
    const trimmed = title.trim();
    if (!trimmed) {
      toast.error('请填写标题');
      return;
    }
    setBusy(true);
    const detail = await create({
      title: trimmed,
      categoryId: categoryId || null,
      initialContent: content,
      tags: tags
        .split(',')
        .map((tag) => tag.trim())
        .filter(Boolean),
    });
    setBusy(false);
    if (detail) {
      onClose();
      toast.success('已创建笔记');
      navigate(`/notes/${detail.note.id}`);
    } else {
      reportFailure('创建笔记失败', useNotesStore.getState().error);
    }
  };

  return (
    <>
      <div className="space-y-1.5">
        <Label htmlFor="new-note-title">标题</Label>
        <Input
          id="new-note-title"
          autoFocus
          value={title}
          placeholder="例如：SQLite 全文搜索学习笔记"
          onChange={(event) => {
            setTitle(event.target.value);
            // 已选模板且正文未被手改时，跟随标题重新渲染（{{title}} 占位符）
            if (templateName && !contentEdited) {
              void applyTemplate(templateName, event.target.value);
            }
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) void submit();
          }}
        />
      </div>
      <div className="space-y-1.5">
        <Label htmlFor="new-note-tags">标签（逗号分隔）</Label>
        <Input
          id="new-note-tags"
          value={tags}
          placeholder="sqlite, search"
          onChange={(event) => setTags(event.target.value)}
        />
      </div>
      <div className="space-y-1.5">
        {templates.length > 0 ? (
          <div className="space-y-1.5">
            <Label htmlFor="new-note-template">模板</Label>
            <select
              id="new-note-template"
              value={templateName}
              onChange={(event) => void applyTemplate(event.target.value)}
              className="h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-sm text-[var(--ink)]"
            >
              <option value="">不使用模板</option>
              {templates.map((template) => (
                <option key={template.name} value={template.name}>
                  {template.title}
                </option>
              ))}
            </select>
            <p className="text-[11px] text-[var(--ink-subtle)]">
              模板来自 .knowledge/templates，支持 {'{{title}}'} {'{{date}}'} {'{{time}}'}{' '}
              {'{{datetime}}'} {'{{weekday}}'} 占位符。
            </p>
          </div>
        ) : null}
        <Label htmlFor="new-note-content">初始内容（可选）</Label>
        <Textarea
          id="new-note-content"
          value={content}
          rows={5}
          placeholder="支持 Markdown 语法"
          onChange={(event) => {
            setContent(event.target.value);
            setContentEdited(true);
          }}
        />
      </div>
      <DialogFooter>
        <Button variant="secondary" onClick={onClose} disabled={busy}>
          取消
        </Button>
        <Button onClick={() => void submit()} disabled={busy}>
          创建笔记
        </Button>
      </DialogFooter>
    </>
  );
}

/**
 * 从网页剪藏（V1.2.1 C1）：
 * 输入地址 → 抓取预览（标题 / 站点 / 字数 / 正文前几行）→ 创建笔记，
 * 并把来源写进 frontmatter（`source_type: web` + `source_ref: 最终地址`）。
 */
function ClipForm({ categoryId, onClose }: { categoryId: string; onClose: () => void }) {
  const navigate = useNavigate();
  const create = useNotesStore((state) => state.create);
  const [url, setUrl] = useState('');
  const [page, setPage] = useState<ClippedPage | null>(null);
  const [title, setTitle] = useState('');
  const [busy, setBusy] = useState(false);
  const [creating, setCreating] = useState(false);

  const fetchPage = async () => {
    const trimmed = url.trim();
    if (!trimmed) {
      toast.error('请填写网页地址');
      return;
    }
    setBusy(true);
    try {
      const clipped = await clipService.clip(trimmed);
      setPage(clipped);
      setTitle(clipped.title);
      toast.success(`已抓取：${clipped.wordCount} 字`);
    } catch (error) {
      setPage(null);
      reportFailure('抓取网页失败', errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const submit = async () => {
    if (!page) {
      toast.error('请先抓取网页');
      return;
    }
    const trimmed = title.trim() || page.title;
    setCreating(true);
    // 来源信息写进正文抬头（frontmatter 的 source_* 由 setSource 落库）
    const header = `> 来源：${page.finalUrl}\n> 抓取于 ${page.fetchedAt}${page.site ? ` · ${page.site}` : ''}\n\n`;
    const detail = await create({
      title: trimmed,
      categoryId: categoryId || null,
      initialContent: `${header}${page.markdown}`,
    });
    if (detail) {
      try {
        await noteService.setSource({
          id: detail.note.id,
          sourceType: 'web',
          sourceRef: page.finalUrl,
        });
      } catch (error) {
        // 来源是附加信息：不阻断剪藏，但要如实告诉用户
        toast.warning(`笔记已创建，但来源信息写入失败：${errorMessage(error)}`);
      }
      // 重新拉一次详情，让编辑器的元信息栏立刻显示「网页剪藏」来源
      await useNotesStore.getState().openDetail(detail.note.id);
      setCreating(false);
      onClose();
      toast.success('已剪藏为新笔记');
      navigate(`/notes/${detail.note.id}`);
      return;
    }
    setCreating(false);
    reportFailure('创建笔记失败', useNotesStore.getState().error);
  };

  const preview = page ? page.markdown.split('\n').slice(0, 12).join('\n') : '';

  return (
    <>
      <div className="space-y-2 rounded-md border border-[var(--border-subtle)] p-3">
        <div className="flex flex-wrap gap-2">
          <Input
            aria-label="要剪藏的网页地址"
            value={url}
            data-testid="clip-url"
            placeholder="https://example.com/article"
            className="h-8 min-w-[220px] flex-1 text-xs"
            onChange={(event) => setUrl(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') void fetchPage();
            }}
          />
          <Button
            size="sm"
            variant="secondary"
            data-testid="clip-fetch"
            disabled={busy}
            onClick={() => void fetchPage()}
          >
            {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Link2 className="h-3.5 w-3.5" />}
            抓取
          </Button>
        </div>
        <p className="text-[11px] text-[var(--ink-subtle)]">
          只抓取**公开网页**（http / https，单页 ≤ 3 MB）：抽取正文转成 Markdown，
          并把来源地址写进笔记元数据；页面需要登录或正文由脚本渲染时可能抽不到内容。
        </p>

        {page ? (
          <div className="space-y-2 border-t border-[var(--border-subtle)] pt-2">
            <div className="space-y-1.5">
              <Label htmlFor="clip-title">标题</Label>
              <Input
                id="clip-title"
                value={title}
                className="h-8 text-xs"
                onChange={(event) => setTitle(event.target.value)}
              />
            </div>
            <p className="text-[11px] text-[var(--ink-subtle)]" data-testid="clip-meta">
              站点：{page.site ?? '未知'} · {page.wordCount} 字 · 抓取于 {page.fetchedAt}
            </p>
            <pre
              data-testid="clip-preview"
              className="max-h-40 overflow-auto whitespace-pre-wrap rounded bg-[var(--surface-muted)] p-2 text-[11px] leading-relaxed text-[var(--ink-muted)]"
            >
              {preview}
            </pre>
          </div>
        ) : null}
      </div>

      <DialogFooter>
        <Button variant="secondary" onClick={onClose} disabled={creating}>
          取消
        </Button>
        <Button
          data-testid="clip-create"
          onClick={() => void submit()}
          disabled={creating || !page}
        >
          {creating ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Link2 className="h-3.5 w-3.5" />}
          剪藏为新笔记
        </Button>
      </DialogFooter>
    </>
  );
}

function ImportForm({ categoryId, onClose }: { categoryId: string; onClose: () => void }) {
  const navigate = useNavigate();
  const [preserveStructure, setPreserveStructure] = useState(false);
  const [importMode, setImportMode] = useState('auto');
  const refreshNotes = useNotesStore((state) => state.refresh);
  const refreshStats = useWorkspaceStore((state) => state.refreshStats);
  const [paths, setPaths] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);

  const desktop = isTauriRuntime();

  const addPaths = (added: string[]) => {
    setPaths((previous) => {
      const next = [...previous];
      added.forEach((path) => {
        if (!next.includes(path)) next.push(path);
      });
      return next;
    });
  };

  const chooseFiles = async () => {
    try {
      addPaths(await pickMarkdownFiles());
    } catch (error) {
      reportFailure('选择 Markdown 文件失败', errorMessage(error));
    }
  };

  const chooseFolder = async () => {
    try {
      const dir = await pickDirectory('选择要导入的文件夹');
      if (dir) addPaths([dir]);
    } catch (error) {
      reportFailure('选择文件夹失败', errorMessage(error));
    }
  };

  const submit = async () => {
    if (paths.length === 0) {
      toast.error('请先选择文件或文件夹');
      return;
    }
    setBusy(true);
    try {
      const result = await noteService.importFiles({
        paths,
        categoryId: categoryId || null,
        preserveStructure,
        mode: importMode,
      });
      await refreshNotes();
      await refreshStats();
      if (result.imported.length > 0) {
        // 编码提醒要在「导入成功」这条路径上也说出来——这条路径会直接跳到第一篇笔记，
        // 结果面板根本不会展示（面板只在「全部失败」时保留）。
        if ((result.warnings?.length ?? 0) > 0) {
          toast.warning(
            `${result.warnings!.length} 个文件不是 UTF-8，已按原编码解码：${result.warnings![0]}`,
          );
        }
        // 导入成功后直接进入第一篇笔记，避免停在结果页
        const first = result.imported[0];
        onClose();
        if (result.failed > 0) {
          toast.warning(
            `已导入 ${result.imported.length} 篇，${result.failed} 个文件被跳过（体积超限或路径不可读）`,
          );
        } else {
          const extra =
            result.attachments > 0
              ? `，附 ${result.attachments} 个附件`
              : '';
          const modeLabel =
            result.mode === 'obsidian' ? '（Obsidian）' : result.mode === 'notion' ? '（Notion）' : '';
          toast.success(`已导入 ${result.imported.length} 篇笔记${extra}${modeLabel}`);
        }
        if (first) navigate(`/notes/${first.noteId}`);
      } else {
        // 全部失败时保留结果面板，列出原因供用户修正
        setReport(result);
        toast.error('没有导入任何文件，请查看下方跳过原因');
      }
    } catch (error) {
      reportFailure('导入失败', errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  if (report) {
    return (
      <>
        <div className="max-h-72 space-y-2 overflow-auto rounded-md border border-[var(--border-subtle)] p-3 text-xs">
          <p className="flex items-center gap-1.5 text-emerald-600">
            <CheckCircle2 className="h-3.5 w-3.5" />
            成功导入 {report.imported.length} 篇（共 {report.total} 个文件）
          </p>
          {report.imported.slice(0, 8).map((item) => (
            <p key={item.noteId} className="truncate text-[var(--ink-muted)]">
              · {item.title}
              <span className="ml-1 text-[var(--ink-subtle)]">{item.filePath}</span>
            </p>
          ))}
          {(report.warnings?.length ?? 0) > 0 ? (
            <>
              <p className="flex items-center gap-1.5 pt-1 text-amber-600">
                <TriangleAlert className="h-3.5 w-3.5" />
                {report.warnings!.length} 条提醒
              </p>
              {report.warnings!.slice(0, 5).map((line) => (
                <p key={line} className="truncate text-[var(--ink-subtle)]">
                  · {line}
                </p>
              ))}
            </>
          ) : null}
          {report.skipped.length > 0 ? (
            <>
              <p className="flex items-center gap-1.5 pt-1 text-red-500">
                <XCircle className="h-3.5 w-3.5" />
                跳过 {report.skipped.length} 个文件
              </p>
              {report.skipped.slice(0, 8).map((item) => (
                <p key={item.sourcePath} className="truncate text-[var(--ink-subtle)]">
                  · {item.sourcePath} — {item.reason}
                </p>
              ))}
            </>
          ) : null}
        </div>
        <DialogFooter>
          <Button variant="secondary" onClick={onClose}>
            关闭
          </Button>
          <Button
            disabled={report.imported.length === 0}
            onClick={() => {
              const first = report.imported[0];
              onClose();
              if (first) navigate(`/notes/${first.noteId}`);
            }}
          >
            打开第一篇
          </Button>
        </DialogFooter>
      </>
    );
  }

  return (
    <>
      <div className="space-y-2 rounded-md border border-[var(--border-subtle)] p-3">
        <div className="flex flex-wrap gap-2">
          <Button
            size="sm"
            variant="secondary"
            onClick={() => void chooseFiles()}
            disabled={!desktop || busy}
          >
            <FileUp className="h-3.5 w-3.5" />
            选择文件
          </Button>
          <Button
            size="sm"
            variant="secondary"
            onClick={() => void chooseFolder()}
            disabled={!desktop || busy}
          >
            <FolderOpen className="h-3.5 w-3.5" />
            选择文件夹
          </Button>
          {paths.length > 0 ? (
            <Button size="sm" variant="ghost" onClick={() => setPaths([])} disabled={busy}>
              清空（{paths.length}）
            </Button>
          ) : null}
        </div>

        {!desktop ? (
          <p className="text-[11px] text-amber-600">
            浏览器预览模式无法读取本地文件，请在桌面应用中使用导入功能。
          </p>
        ) : (
          <p className="text-[11px] text-[var(--ink-subtle)]">
            支持 <code>.md</code> / <code>.markdown</code> / <code>.txt</code> /{' '}
            <code>.docx</code> / <code>.csv</code>，可多选；
            选择文件夹会递归导入其中所有受支持的文本文件（单文件 ≤ 5 MB，单次 ≤ 500 个）。
            源文件不会被修改。
          </p>
        )}

        <div className="grid gap-2 sm:grid-cols-2">
          <label className="space-y-1 text-xs text-[var(--ink-muted)]">
            <span>导入格式</span>
            <select
              value={importMode}
              disabled={!desktop}
              onChange={(event) => setImportMode(event.target.value)}
              className="h-8 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs text-[var(--ink)]"
            >
              <option value="auto">自动识别（推荐）</option>
              <option value="plain">通用 Markdown</option>
              <option value="obsidian">Obsidian 库</option>
              <option value="notion">Notion 导出</option>
            </select>
          </label>
          <label className="mt-4 flex items-center gap-2 text-xs text-[var(--ink-muted)]">
            <input
              type="checkbox"
              checked={preserveStructure}
              disabled={!desktop}
              onChange={(event) => setPreserveStructure(event.target.checked)}
            />
            保留目录结构（重建子目录 + 原文件名）
          </label>
        </div>

        {paths.length > 0 ? (
          <ul className="max-h-40 space-y-1 overflow-auto text-xs text-[var(--ink-muted)]">
            {paths.map((path) => (
              <li key={path} className="flex items-center gap-2">
                <span className="min-w-0 flex-1 truncate" title={path}>
                  {path}
                </span>
                <button
                  type="button"
                  aria-label={`移除 ${path}`}
                  className="rounded p-0.5 hover:bg-[var(--surface-muted)]"
                  onClick={() => setPaths((previous) => previous.filter((item) => item !== path))}
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </li>
            ))}
          </ul>
        ) : null}
      </div>

      <DialogFooter>
        <Button variant="secondary" onClick={onClose} disabled={busy}>
          取消
        </Button>
        <Button onClick={() => void submit()} disabled={busy || paths.length === 0}>
          {busy ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <FileUp className="h-3.5 w-3.5" />
          )}
          导入到知识库
        </Button>
      </DialogFooter>
    </>
  );
}
