import { create } from 'zustand';

import { errorMessage } from '@/lib/format';
import { workspaceService } from '@/services/workspaceService';
import { searchService } from '@/services/searchService';
import { useUiStore } from '@/stores/uiStore';
import type {
  AppSettings,
  IndexStatus,
  ScanReport,
  UpdateSettingsInput,
  Workspace,
  WorkspaceStats,
  WorkspaceSummary,
} from '@/types';

interface WorkspaceState {
  workspace: Workspace | null;
  stats: WorkspaceStats;
  settings: AppSettings | null;
  indexStatus: IndexStatus | null;
  workspaces: WorkspaceSummary[];
  loading: boolean;
  error: string | null;
  initializedWarnings: string[];
  bootstrap: () => Promise<void>;
  refreshList: () => Promise<void>;
  createWorkspace: (name: string, root: string) => Promise<boolean>;
  openWorkspace: (root: string) => Promise<boolean>;
  switchWorkspace: (id: string) => Promise<boolean>;
  closeWorkspace: () => Promise<void>;
  forgetWorkspace: (id: string) => Promise<void>;
  refreshStats: () => Promise<void>;
  scan: () => Promise<ScanReport | null>;
  rebuildIndex: () => Promise<void>;
  updateSettings: (input: UpdateSettingsInput) => Promise<void>;
  refreshIndexStatus: () => Promise<IndexStatus | null>;
}

const emptyStats: WorkspaceStats = {
  noteCount: 0,
  inboxCount: 0,
  trashCount: 0,
  categoryCount: 0,
  tagCount: 0,
  attachmentCount: 0,
  totalWords: 0,
  updatedLast7d: 0,
  updatedLast30d: 0,
  pinnedCount: 0,
};

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  workspace: null,
  stats: emptyStats,
  settings: null,
  indexStatus: null,
  workspaces: [],
  loading: false,
  error: null,
  initializedWarnings: [],

  bootstrap: async () => {
    set({ loading: true, error: null });
    try {
      const current = await workspaceService.current();
      if (current) {
        set({
          workspace: current.workspace,
          stats: current.stats,
          settings: current.settings,
          indexStatus: current.indexStatus,
        });
        useUiStore.getState().applyPreferences(current.settings);
      }
      await get().refreshList();
    } catch (error) {
      set({ error: errorMessage(error) });
    } finally {
      set({ loading: false });
    }
  },

  refreshList: async () => {
    try {
      const workspaces = await workspaceService.list();
      set({ workspaces });
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  createWorkspace: async (name, root) => {
    set({ loading: true, error: null });
    try {
      const result = await workspaceService.create(name, root);
      set({
        workspace: result.workspace,
        stats: result.stats,
        settings: result.settings,
        indexStatus: result.indexStatus,
        initializedWarnings: result.warnings,
      });
      useUiStore.getState().applyPreferences(result.settings);
      await get().refreshList();
      return true;
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    } finally {
      set({ loading: false });
    }
  },

  openWorkspace: async (root) => {
    set({ loading: true, error: null });
    try {
      const result = await workspaceService.open(root);
      set({
        workspace: result.workspace,
        stats: result.stats,
        settings: result.settings,
        indexStatus: result.indexStatus,
        initializedWarnings: result.warnings,
      });
      useUiStore.getState().applyPreferences(result.settings);
      await get().refreshList();
      return true;
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    } finally {
      set({ loading: false });
    }
  },

  switchWorkspace: async (id) => {
    set({ loading: true, error: null });
    try {
      const result = await workspaceService.switch(id);
      set({
        workspace: result.workspace,
        stats: result.stats,
        settings: result.settings,
        indexStatus: result.indexStatus,
        initializedWarnings: result.warnings,
      });
      useUiStore.getState().applyPreferences(result.settings);
      await get().refreshList();
      return true;
    } catch (error) {
      set({ error: errorMessage(error) });
      return false;
    } finally {
      set({ loading: false });
    }
  },

  closeWorkspace: async () => {
    try {
      await workspaceService.close();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
    set({ workspace: null, stats: emptyStats, settings: null, indexStatus: null });
  },

  forgetWorkspace: async (id) => {
    try {
      await workspaceService.forget(id);
      await get().refreshList();
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  refreshStats: async () => {
    if (!get().workspace) return;
    try {
      const stats = await workspaceService.stats();
      set({ stats });
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  scan: async () => {
    try {
      return await workspaceService.scan();
    } catch (error) {
      set({ error: errorMessage(error) });
      return null;
    }
  },

  rebuildIndex: async () => {
    try {
      const status = await workspaceService.rebuildIndex();
      set({ indexStatus: status });
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  updateSettings: async (input) => {
    try {
      const settings = await workspaceService.updateSettings(input);
      // 成功即清除上一次的错误提示（状态栏不再残留历史警告）
      set({ settings, error: null });
    } catch (error) {
      set({ error: errorMessage(error) });
    }
  },

  refreshIndexStatus: async () => {
    if (!get().workspace) return null;
    try {
      const status = await searchService.status();
      set({ indexStatus: status });
      return status;
    } catch (error) {
      set({ error: errorMessage(error) });
      return null;
    }
  },
}));

/**
 * 把界面布局偏好（分栏宽度、分屏比例、主题、字号）写回工作区设置，
 * 下次打开同一个知识库时恢复。拖动过程中不调用，只在拖动结束/键盘调整后调用。
 */
export function persistLayoutPreferences(): void {
  if (!useWorkspaceStore.getState().workspace) return;
  const ui = useUiStore.getState();
  void useWorkspaceStore.getState().updateSettings({
    sidebarWidth: Math.round(ui.sidebarWidth),
    listWidth: Math.round(ui.listWidth),
    editorSplitRatio: Math.round(ui.editorSplitRatio * 1000) / 1000,
    editorMode: ui.editorMode,
    fontSize: ui.fontSize,
    theme: ui.theme,
  });
}
