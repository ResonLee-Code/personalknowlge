import { create } from 'zustand';

export type EditorMode = 'edit' | 'split' | 'preview';
export type ThemeName = 'light' | 'dark';

/** 与工作区设置对应的界面偏好（用于从磁盘恢复上次的布局）。 */
export interface UiPreferences {
  sidebarWidth?: number;
  listWidth?: number;
  /** 来自工作区设置（字符串），只接受已知取值 */
  editorMode?: string;
  fontSize?: number;
  theme?: string;
  editorSplitRatio?: number;
}

export const LAYOUT_DEFAULTS = {
  sidebarWidth: 248,
  listWidth: 320,
  editorSplitRatio: 0.5,
} as const;

interface UiState {
  sidebarWidth: number;
  listWidth: number;
  /** 分屏模式下编辑器占比（0.3 ~ 0.7） */
  editorSplitRatio: number;
  sidebarCollapsed: boolean;
  editorMode: EditorMode;
  theme: ThemeName;
  fontSize: number;
  commandPaletteOpen: boolean;
  newNoteDialogOpen: boolean;
  setSidebarWidth: (width: number) => void;
  setListWidth: (width: number) => void;
  setEditorSplitRatio: (ratio: number) => void;
  toggleSidebar: () => void;
  setEditorMode: (mode: EditorMode) => void;
  setTheme: (theme: ThemeName) => void;
  setFontSize: (size: number) => void;
  setCommandPaletteOpen: (open: boolean) => void;
  setNewNoteDialogOpen: (open: boolean) => void;
  applyThemeToDocument: () => void;
  applyPreferences: (preferences: UiPreferences) => void;
}

/** 界面状态：只保存纯 UI 状态（设计文档 15.1）。 */
export const useUiStore = create<UiState>((set, get) => ({
  sidebarWidth: LAYOUT_DEFAULTS.sidebarWidth,
  listWidth: LAYOUT_DEFAULTS.listWidth,
  editorSplitRatio: LAYOUT_DEFAULTS.editorSplitRatio,
  sidebarCollapsed: false,
  editorMode: 'split',
  theme: 'light',
  fontSize: 15,
  commandPaletteOpen: false,
  newNoteDialogOpen: false,
  setSidebarWidth: (sidebarWidth) => {
    // 取整：Retina 屏上 clientX 可能是小数，后端按整数像素存储
    const clamped = Math.round(Math.min(420, Math.max(180, sidebarWidth)));
    set({ sidebarWidth: clamped });
  },
  setListWidth: (listWidth) => {
    const clamped = Math.round(Math.min(560, Math.max(240, listWidth)));
    set({ listWidth: clamped });
  },
  setEditorSplitRatio: (editorSplitRatio) => {
    // 保留三位小数，既够精细也避免浮点噪声
    const clamped = Math.min(0.7, Math.max(0.3, editorSplitRatio));
    set({ editorSplitRatio: Math.round(clamped * 1000) / 1000 });
  },
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  setEditorMode: (editorMode) => set({ editorMode }),
  setTheme: (theme) => {
    set({ theme });
    get().applyThemeToDocument();
  },
  setFontSize: (fontSize) => set({ fontSize: Math.min(24, Math.max(12, fontSize)) }),
  setCommandPaletteOpen: (commandPaletteOpen) => set({ commandPaletteOpen }),
  setNewNoteDialogOpen: (newNoteDialogOpen) => set({ newNoteDialogOpen }),
  applyThemeToDocument: () => {
    const { theme } = get();
    if (typeof document === 'undefined') return;
    document.documentElement.classList.toggle('dark', theme === 'dark');
    document.documentElement.dataset.theme = theme;
  },
  applyPreferences: (preferences) => {
    const next: Partial<UiState> = {};
    if (typeof preferences.sidebarWidth === 'number') {
      next.sidebarWidth = Math.round(
        Math.min(420, Math.max(180, preferences.sidebarWidth)),
      );
    }
    if (typeof preferences.listWidth === 'number') {
      next.listWidth = Math.round(Math.min(560, Math.max(240, preferences.listWidth)));
    }
    if (
      preferences.editorMode === 'edit' ||
      preferences.editorMode === 'split' ||
      preferences.editorMode === 'preview'
    ) {
      next.editorMode = preferences.editorMode;
    }
    if (typeof preferences.fontSize === 'number') next.fontSize = preferences.fontSize;
    const themeApplied = preferences.theme === 'light' || preferences.theme === 'dark';
    if (themeApplied) next.theme = preferences.theme as ThemeName;
    if (typeof preferences.editorSplitRatio === 'number') {
      const ratio = Math.min(0.7, Math.max(0.3, preferences.editorSplitRatio));
      next.editorSplitRatio = Math.round(ratio * 1000) / 1000;
    }
    if (Object.keys(next).length === 0) return;
    set(next);
    if (themeApplied) get().applyThemeToDocument();
  },
}));
