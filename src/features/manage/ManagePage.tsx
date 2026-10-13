import {
  CheckSquare,
  Copy,
  FileText,
  FolderTree,
  Pin,
  RefreshCw,
  Search,
  Tag,
  Trash2,
  TriangleAlert,
  Undo2,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';

import { BatchCopyDialog, BatchMoveDialog, BatchTagDialog } from '@/components/notes/BatchActionDialogs';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Skeleton } from '@/components/ui/badge';
import { ROUTES } from '@/lib/constants';
import { reportFailure } from '@/lib/actionFeedback';
import { errorMessage, formatRelative } from '@/lib/format';
import { cn } from '@/lib/utils';
import { noteService } from '@/services/noteService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useTagsStore } from '@/stores/tagsStore';
import type { NoteStatus, NoteSummary } from '@/types';

/**
 * 目录管理（V1.2.1 P2，对标语雀「目录管理」）：
 * 把全部文档放在一张宽表里，支持搜索、状态筛选（含「不在目录中」）与批量操作。
 *
 * 这里直接调用 `noteService.list`，不复用笔记列表的 scope：
 * 管理视图需要跨状态（活跃 / 回收站 / 缺失）查看，与中栏列表的语义不同。
 */
export function ManagePage() {
  const navigate = useNavigate();
  const pathOf = useCategoriesStore((state) => state.pathOf);
  const refreshStats = useCategoriesStore((state) => state.refresh);
  const refreshTags = useTagsStore((state) => state.refresh);

  const [keyword, setKeyword] = useState('');
  const [status, setStatus] = useState<NoteStatus>('active');
  const [rootOnly, setRootOnly] = useState(false);
  const [notes, setNotes] = useState<NoteSummary[]>([]);
  const [total, setTotal] = useState(0);
  /** 已加载完成的查询键：与当前查询不一致即视为「加载中」（派生状态，避免在 effect 里同步 setState） */
  const [loadedKey, setLoadedKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [dialog, setDialog] = useState<'move' | 'tag' | 'copy' | null>(null);

  const query = useMemo(
    () => ({
      status,
      keyword: keyword.trim() ? keyword.trim() : null,
      rootOnly,
      sort: 'updatedDesc' as const,
      pageSize: 500,
    }),
    [status, keyword, rootOnly],
  );

  const queryKey = useMemo(() => JSON.stringify(query), [query]);
  const loading = loadedKey !== queryKey;

  const reload = useCallback(async () => {
    try {
      const result = await noteService.list(query);
      setNotes(result.items);
      setTotal(result.total);
      setError(null);
    } catch (cause) {
      setError(errorMessage(cause));
      setNotes([]);
      setTotal(0);
    } finally {
      setLoadedKey(queryKey);
    }
  }, [query, queryKey]);

  useEffect(() => {
    let cancelled = false;
    void noteService
      .list(query)
      .then((result) => {
        if (cancelled) return;
        setNotes(result.items);
        setTotal(result.total);
        setError(null);
      })
      .catch((cause: unknown) => {
        if (cancelled) return;
        setError(errorMessage(cause));
        setNotes([]);
        setTotal(0);
      })
      .finally(() => {
        if (!cancelled) setLoadedKey(queryKey);
      });
    return () => {
      cancelled = true;
    };
  }, [query, queryKey]);

  const allSelected = notes.length > 0 && selected.length === notes.length;
  const toggleAll = () =>
    setSelected(allSelected ? [] : notes.map((note) => note.id));
  const toggleOne = (id: string) =>
    setSelected((previous) =>
      previous.includes(id) ? previous.filter((item) => item !== id) : [...previous, id],
    );

  /** 打开批量对话框：把选中项写进 notesStore 的 selection，复用已有对话框 */
  const openDialog = (kind: 'move' | 'tag' | 'copy') => {
    if (selected.length === 0) return;
    useNotesStore.getState().setSelection(selected);
    setDialog(kind);
  };

  const afterDialog = () => {
    setDialog(null);
    setSelected([]);
    useNotesStore.getState().setSelection([]);
    void (async () => {
      await reload();
      await refreshStats();
      await refreshTags();
    })();
  };

  const restoreMany = async () => {
    const ids = [...selected];
    let restored = 0;
    for (const id of ids) {
      try {
        await noteService.restore(id);
        restored += 1;
      } catch (cause) {
        reportFailure('恢复失败', errorMessage(cause));
        break;
      }
    }
    setSelected([]);
    await reload();
    await refreshStats();
    if (restored > 0) toast.success(`已恢复 ${restored} 篇笔记`);
  };

  const statusLabel = (value: NoteStatus) =>
    value === 'active' ? '活跃' : value === 'deleted' ? '回收站' : '文件缺失';

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 p-4">
      <header className="flex flex-wrap items-center gap-3">
        <div>
          <h1 className="text-lg font-semibold text-[var(--ink)]">目录管理</h1>
          <p className="text-xs text-[var(--ink-subtle)]">
            跨状态查看与管理全部文档（对标语雀的「目录管理」）：搜索、筛选、批量移动 / 打标签 / 复制 / 删除。
          </p>
        </div>
        <div className="ml-auto flex items-center gap-2">
          <Button
            size="sm"
            variant="secondary"
            onClick={() =>
              void (async () => {
                await reload();
                toast.success('已刷新');
              })()
            }
          >
            <RefreshCw className="h-3.5 w-3.5" />
            刷新
          </Button>
          <Button size="sm" onClick={() => navigate(ROUTES.notes)}>
            <FileText className="h-3.5 w-3.5" />
            返回笔记
          </Button>
        </div>
      </header>

      <div className="flex flex-wrap items-center gap-2 rounded-lg border border-[var(--border-subtle)] bg-[var(--surface)] p-2.5">
        <div className="relative min-w-[200px] flex-1">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-[var(--ink-subtle)]" />
          <Input
            aria-label="搜索文档标题"
            value={keyword}
            data-testid="manage-search"
            placeholder="按标题筛选"
            className="h-8 pl-8 text-xs"
            onChange={(event) => setKeyword(event.target.value)}
          />
        </div>
        <select
          value={status}
          data-testid="manage-status"
          aria-label="按状态筛选"
          onChange={(event) => setStatus(event.target.value as NoteStatus)}
          className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs text-[var(--ink)]"
        >
          <option value="active">活跃</option>
          <option value="deleted">回收站</option>
          <option value="missing">文件缺失</option>
        </select>
        <label className="flex items-center gap-1.5 text-[11px] text-[var(--ink-muted)]">
          <input
            type="checkbox"
            data-testid="manage-root-only"
            checked={rootOnly}
            onChange={(event) => setRootOnly(event.target.checked)}
          />
          不在目录中（顶层文档）
        </label>
        <span className="text-[11px] text-[var(--ink-subtle)]">
          {statusLabel(status)} · 共 {total} 篇（最多显示 500 篇）
        </span>
      </div>

      {selected.length > 0 ? (
        <div
          data-testid="manage-batch-toolbar"
          className="flex flex-wrap items-center gap-2 rounded-md bg-[var(--brand-soft)] px-2.5 py-1.5 text-[11px] text-[var(--brand)]"
        >
          <CheckSquare className="h-3.5 w-3.5" />
          已选 {selected.length}
          <button type="button" className="underline decoration-dotted" onClick={() => setSelected([])}>
            清除
          </button>
          <span className="mx-0.5 h-3 w-px bg-[var(--brand)]/30" />
          <button
            type="button"
            data-testid="manage-move"
            className="flex items-center gap-0.5"
            onClick={() => openDialog('move')}
          >
            <FolderTree className="h-3 w-3" />
            移动分类
          </button>
          <button
            type="button"
            data-testid="manage-tag"
            className="flex items-center gap-0.5"
            onClick={() => openDialog('tag')}
          >
            <Tag className="h-3 w-3" />
            打标签
          </button>
          <button
            type="button"
            data-testid="manage-copy"
            className="flex items-center gap-0.5"
            onClick={() => openDialog('copy')}
          >
            <Copy className="h-3 w-3" />
            复制到
          </button>
          {status === 'deleted' ? (
            <button
              type="button"
              data-testid="manage-restore"
              className="flex items-center gap-0.5"
              onClick={() => void restoreMany()}
            >
              <Undo2 className="h-3 w-3" />
              恢复
            </button>
          ) : (
            <button
              type="button"
              data-testid="manage-delete"
              className="flex items-center gap-0.5 text-red-600"
              onClick={() => {
                if (
                  !window.confirm(`将选中的 ${selected.length} 篇笔记移入回收站？（可恢复）`)
                ) {
                  return;
                }
                useNotesStore.getState().setSelection(selected);
                void (async () => {
                  await useNotesStore.getState().deleteMany();
                  afterDialog();
                })();
              }}
            >
              <Trash2 className="h-3 w-3" />
              删除
            </button>
          )}
        </div>
      ) : null}

      <div className="min-h-0 flex-1 overflow-auto rounded-lg border border-[var(--border-subtle)]">
        <table className="w-full border-collapse text-xs">
          <thead className="sticky top-0 z-10 bg-[var(--surface-muted)] text-[var(--ink-muted)]">
            <tr>
              <th className="w-10 px-2 py-2 text-left">
                <input
                  type="checkbox"
                  data-testid="manage-select-all"
                  checked={allSelected}
                  onChange={toggleAll}
                  aria-label="全选"
                />
              </th>
              <th className="px-2 py-2 text-left">标题</th>
              <th className="w-40 px-2 py-2 text-left">分类</th>
              <th className="w-40 px-2 py-2 text-left">标签</th>
              <th className="w-32 px-2 py-2 text-left">层级</th>
              <th className="w-28 px-2 py-2 text-left">更新时间</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr>
                <td colSpan={6} className="p-3">
                  <div className="space-y-2">
                    {[0, 1, 2].map((index) => (
                      <Skeleton key={index} className="h-6 w-full" />
                    ))}
                  </div>
                </td>
              </tr>
            ) : error ? (
              <tr>
                <td colSpan={6} className="p-3 text-red-500">
                  {error}
                </td>
              </tr>
            ) : notes.length === 0 ? (
              <tr>
                <td colSpan={6} className="p-6 text-center text-[var(--ink-subtle)]">
                  当前筛选下没有文档
                </td>
              </tr>
            ) : (
              notes.map((note) => {
                const checked = selected.includes(note.id);
                return (
                  <tr
                    key={note.id}
                    data-testid="manage-row"
                    className={cn(
                      'border-t border-[var(--border-subtle)] transition-colors',
                      checked ? 'bg-[var(--brand-soft)]' : 'hover:bg-[var(--surface-muted)]',
                    )}
                  >
                    <td className="px-2 py-2">
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={() => toggleOne(note.id)}
                        aria-label={`选择 ${note.title}`}
                      />
                    </td>
                    <td className="px-2 py-2">
                      <button
                        type="button"
                        className="flex items-center gap-1 text-left text-[var(--ink)] hover:text-[var(--brand)]"
                        onClick={() => navigate(`${ROUTES.notes}/${note.id}`)}
                      >
                        {note.pinned ? <Pin className="h-3 w-3 text-[var(--brand)]" /> : null}
                        <span className="line-clamp-1">{note.title}</span>
                        {note.status === 'missing' ? (
                          <TriangleAlert className="h-3 w-3 text-amber-500" />
                        ) : null}
                      </button>
                      <span className="text-[10px] text-[var(--ink-subtle)]">{note.filePath}</span>
                    </td>
                    <td className="px-2 py-2 text-[var(--ink-muted)]">
                      {note.categoryId ? pathOf(note.categoryId) : 'Inbox'}
                    </td>
                    <td className="px-2 py-2 text-[var(--ink-muted)]">
                      {note.tags.length > 0 ? note.tags.slice(0, 3).join('、') : '—'}
                    </td>
                    <td className="px-2 py-2 text-[var(--ink-muted)]">
                      {note.parentId
                        ? `子文档 · ${notes.find((item) => item.id === note.parentId)?.title ?? '（父文档不在当前筛选）'}`
                        : '顶层'}
                    </td>
                    <td className="px-2 py-2 text-[var(--ink-subtle)]">
                      {formatRelative(note.updatedAt)}
                    </td>
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>

      {dialog === 'move' ? <BatchMoveDialog onClose={afterDialog} /> : null}
      {dialog === 'tag' ? <BatchTagDialog onClose={afterDialog} /> : null}
      {dialog === 'copy' ? <BatchCopyDialog onClose={afterDialog} /> : null}
    </div>
  );
}
