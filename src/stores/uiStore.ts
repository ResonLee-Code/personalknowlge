import { create } from 'zustand';

export type EditorMode = 'edit' | 'split' | 'preview';
export type ThemeName = 'light' | 'dark';

interface UiState {
  sidebarWidth: number;
  listWidth: number;
  sidebarCollapsed: boolean;
  editorMode: EditorMode;
  theme: ThemeName;
  fontSize: number;
  commandPaletteOpen: boolean;
  newNoteDialogOpen: boolean;
  setSidebarWidth: (width: number) => void;
  setListWidth: (width: number) => void;
  toggleSidebar: () => void;
  setEditorMode: (mode: EditorMode) => void;
  setTheme: (theme: ThemeName) => void;
  setFontSize: (size: number) => void;
  setCommandPaletteOpen: (open: boolean) => void;
  setNewNoteDialogOpen: (open: boolean) => void;
  applyThemeToDocument: () => void;
}

/** 界面状态：只保存纯 UI 状态（设计文档 15.1）。 */
export const useUiStore = create<UiState>((set, get) => ({
  sidebarWidth: 248,
  listWidth: 320,
  sidebarCollapsed: false,
  editorMode: 'split',
  theme: 'light',
  fontSize: 15,
  commandPaletteOpen: false,
  newNoteDialogOpen: false,
  setSidebarWidth: (sidebarWidth) => {
    const clamped = Math.min(420, Math.max(180, sidebarWidth));
    set({ sidebarWidth: clamped });
  },
  setListWidth: (listWidth) => {
    const clamped = Math.min(560, Math.max(240, listWidth));
    set({ listWidth: clamped });
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
}));
