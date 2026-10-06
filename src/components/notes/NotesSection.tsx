import { FileQuestion } from 'lucide-react';
import { useEffect, useMemo } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';

import { EmptyState } from '@/components/ui/empty-state';
import { useNotesStore, type NoteScope } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';

import { ResizeHandle } from '@/components/layout/ResizeHandle';
import { NoteEditorPage } from './NoteEditorPage';
import { NoteListPanel } from './NoteListPanel';

/**
 * 笔记工作区：中栏列表 + 右栏编辑器（设计文档 16.1）。
 * 分类与标签通过查询参数筛选，其余通过路由区分。
 */
export function NotesSection({ baseScope }: { baseScope: NoteScope }) {
  const [params] = useSearchParams();
  const { id } = useParams();
  const setScope = useNotesStore((state) => state.setScope);
  const openDetail = useNotesStore((state) => state.openDetail);
  const detail = useNotesStore((state) => state.detail);
  const setListWidth = useUiStore((state) => state.setListWidth);

  const categoryId = params.get('categoryId');
  const tagId = params.get('tagId');
  const scope: NoteScope = useMemo(() => {
    if (categoryId) return { kind: 'category', id: categoryId };
    if (tagId) return { kind: 'tag', id: tagId };
    return baseScope;
    // baseScope 是字面量对象，用 kind 作为稳定依赖
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [categoryId, tagId, baseScope.kind]);

  useEffect(() => {
    void setScope(scope);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scope.kind, categoryId, tagId, setScope]);

  useEffect(() => {
    if (id && detail?.note.id !== id) {
      void openDetail(id);
    }
  }, [id, detail?.note.id, openDetail]);

  return (
    <div className="flex min-h-0 flex-1">
      <NoteListPanel />
      <ResizeHandle side="left" label="调整列表宽度" onResize={setListWidth} />
      <div className="flex min-w-0 flex-1 flex-col">
        {id ? (
          <NoteEditorPage />
        ) : (
          <EmptyState
            icon={FileQuestion}
            title="选择一篇笔记开始阅读"
            description="从左侧列表中选择笔记，或使用 ⌘K 搜索，⌘N 新建。笔记正文是本地 Markdown 文件，可随时用其他编辑器打开。"
            className="flex-1"
          />
        )}
      </div>
    </div>
  );
}
