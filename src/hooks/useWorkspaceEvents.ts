import { useEffect } from 'react';
import { toast } from 'sonner';

import { EVENTS } from '@/lib/constants';
import { formatWarningsToast } from '@/lib/warnings';
import { subscribe } from '@/services/api';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useSearchStore } from '@/stores/searchStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { IndexStatus, ScanReport } from '@/types';

// 订阅后端事件：外部文件变化、索引状态（设计文档 19.3）。
export function useWorkspaceEvents() {
  const workspaceId = useWorkspaceStore((state) => state.workspace?.id ?? null);

  useEffect(() => {
    if (!workspaceId) return;
    let disposed = false;
    const unlisteners: Array<() => void> = [];

    const register = async () => {
      const filesChanged = await subscribe<ScanReport>(EVENTS.filesChanged, (payload) => {
        const parts: string[] = [];
        if (payload.added) parts.push(`新增 ${payload.added}`);
        if (payload.updated) parts.push(`更新 ${payload.updated}`);
        if (payload.missing) parts.push(`缺失 ${payload.missing}`);
        if (parts.length > 0) {
          toast.info(`检测到外部修改：${parts.join('，')}`);
        }
        // 扫描里的提醒（跳过无法读取的文件 / 重建链接失败…）以前被直接丢掉
        const notice = formatWarningsToast(payload.warnings);
        if (notice) toast.warning(notice);
        void useNotesStore.getState().refresh();
        void useWorkspaceStore.getState().refreshStats();
        const detail = useNotesStore.getState().detail;
        if (detail) void useNotesStore.getState().openDetail(detail.note.id);
      });
      if (disposed) {
        filesChanged();
        return;
      }
      unlisteners.push(filesChanged);

      const notesChanged = await subscribe<{ kind?: string }>(EVENTS.notesChanged, () => {
        void useNotesStore.getState().refresh();
        void useWorkspaceStore.getState().refreshStats();
        void useCategoriesStore.getState().refresh();
        void useTagsStore.getState().refresh();
      });
      if (disposed) {
        notesChanged();
        return;
      }
      unlisteners.push(notesChanged);

      const indexStatus = await subscribe<IndexStatus & { warning?: string }>(
        EVENTS.indexStatus,
        (payload) => {
          if (payload.warning) {
            toast.warning(payload.warning);
            return;
          }
          useWorkspaceStore.setState({ indexStatus: payload });
          useSearchStore.setState({ status: payload });
        },
      );
      if (disposed) {
        indexStatus();
        return;
      }
      unlisteners.push(indexStatus);
    };

    void register();
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [workspaceId]);
}
