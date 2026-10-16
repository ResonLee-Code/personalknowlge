import { create } from 'zustand';

export type EditorMode = 'edit' | 'split' | 'preview';
export type ThemeName = 'light' | 'dark';
/** 大纲放在哪：`list` = 与笔记列表共用一栏（默认，省一列横向空间）；`side` = 正文右侧独立一列 */
export type OutlinePlacement = 'list' | 'side';
/** 列表栏当前显示什么 */
export type ListPaneMode = 'list' | 'outline';
/** 大纲钉在正文右侧时的形态：隐藏 / 常驻 / 贴边 mini（贴在右边，鼠标移上去展开） */
export type SideOutlineMode = 'hidden' | 'pinned' | 'mini';
/**
 * 笔记级的动作请求（2026-10-17）。
 *
 * 为什么走 store 转一手：这三个入口（收藏 / 移动分类 / 更多操作）按用户要求挪到了**顶栏**，
 * 而它们要打开的那些对话框（移动分类、重命名、历史版本、来源与时效、导出…）都挂在编辑器里
 * （那里才知道 note 与各对话框的状态）。顶栏只负责"请求"，编辑器负责"接住并打开"。
 */
export type NoteActionKind =
  | 'move'
  | 'rename'
  | 'duplicate'
  | 'history'
  | 'source'
  | 'copyPath'
  | 'reveal'
  | 'template'
  | 'export'
  | 'delete'
  /** 回收站里的笔记：这两个替代「移到回收站」 */
  | 'restore'
  | 'purge';

/** 与工作区设置对应的界面偏好（用于从磁盘恢复上次的布局）。 */
export interface UiPreferences {
  sidebarWidth?: number;
  listWidth?: number;
  /** 来自工作区设置（字符串），只接受已知取值 */
  outlinePlacement?: string;
  autoOutlineOnSelect?: boolean;
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
  /** 大纲的位置（工作区级偏好） */
  outlinePlacement: OutlinePlacement;
  /** 列表栏当前是列表还是大纲（**只活在内存里**：这是"临时看一眼"，不是长期偏好） */
  listPaneMode: ListPaneMode;
  /** 选中笔记后是否自动切到大纲（工作区级偏好，默认关） */
  autoOutlineOnSelect: boolean;
  /**
   * 钉在正文右侧时的大纲形态（V1.3.0 第二轮界面调整）。
   *
   * 为什么从 `NoteEditorPage` 的局部 state 提到这里：大纲的开关现在放在**笔记列表栏的头部**
   * （单一入口，见 `OutlineToggleButton`），列表栏也要能开关右侧那一列——两处读同一份状态，
   * 就不能再让编辑器一个人拿着。仍然只活在内存里（与 `listPaneMode` 同口径）。
   */
  outlineMode: SideOutlineMode;
  /** 分屏模式下编辑器占比（0.3 ~ 0.7） */
  editorSplitRatio: number;
  sidebarCollapsed: boolean;
  /** 侧边栏「分类」区块是否折叠（内存态，与 `sidebarCollapsed` 同口径） */
  categoriesCollapsed: boolean;
  /** 侧边栏「标签」区块是否折叠 */
  tagsCollapsed: boolean;
  editorMode: EditorMode;
  theme: ThemeName;
  fontSize: number;
  commandPaletteOpen: boolean;
  newNoteDialogOpen: boolean;
  quickSwitcherOpen: boolean;
  setSidebarWidth: (width: number) => void;
  setListWidth: (width: number) => void;
  setOutlinePlacement: (placement: OutlinePlacement) => void;
  setListPaneMode: (mode: ListPaneMode) => void;
  setAutoOutlineOnSelect: (value: boolean) => void;
  setOutlineMode: (mode: SideOutlineMode) => void;
  /** 顶栏发出的笔记动作请求；编辑器消费后清空（见 NoteActionKind 的说明） */
  noteActionRequest: NoteActionKind | null;
  requestNoteAction: (kind: NoteActionKind) => void;
  clearNoteAction: () => void;
  setEditorSplitRatio: (ratio: number) => void;
  toggleSidebar: () => void;
  toggleCategoriesCollapsed: () => void;
  toggleTagsCollapsed: () => void;
  setEditorMode: (mode: EditorMode) => void;
  setTheme: (theme: ThemeName) => void;
  setFontSize: (size: number) => void;
  setCommandPaletteOpen: (open: boolean) => void;
  setNewNoteDialogOpen: (open: boolean) => void;
  setQuickSwitcherOpen: (open: boolean) => void;
  /**
   * N5：从**搜索结果**跳进编辑器时要高亮的精确区间（码点下标）。
   * 编辑器消费一次后清空；只活在内存里，不落库。
   */
  pendingHighlight: { noteId: string; charStart: number; charEnd: number } | null;
  setPendingHighlight: (
    value: { noteId: string; charStart: number; charEnd: number } | null,
  ) => void;
  applyThemeToDocument: () => void;
  applyPreferences: (preferences: UiPreferences) => void;
}

/** 界面状态：只保存纯 UI 状态（设计文档 15.1）。 */
export const useUiStore = create<UiState>((set, get) => ({
  sidebarWidth: LAYOUT_DEFAULTS.sidebarWidth,
  listWidth: LAYOUT_DEFAULTS.listWidth,
  outlinePlacement: 'list',
  listPaneMode: 'list',
  autoOutlineOnSelect: false,
  outlineMode: 'pinned',
  editorSplitRatio: LAYOUT_DEFAULTS.editorSplitRatio,
  sidebarCollapsed: false,
  categoriesCollapsed: false,
  tagsCollapsed: false,
  editorMode: 'split',
  theme: 'light',
  fontSize: 15,
  commandPaletteOpen: false,
  newNoteDialogOpen: false,
  quickSwitcherOpen: false,
  setSidebarWidth: (sidebarWidth) => {
    // 取整：Retina 屏上 clientX 可能是小数，后端按整数像素存储
    const clamped = Math.round(Math.min(420, Math.max(180, sidebarWidth)));
    set({ sidebarWidth: clamped });
  },
  setListWidth: (listWidth) => {
    const clamped = Math.round(Math.min(560, Math.max(240, listWidth)));
    set({ listWidth: clamped });
  },
  setOutlinePlacement: (outlinePlacement) =>
    // 换位置时同步这一栏显示什么：
    // - 挪到「正文右侧」：列表栏还给列表（那边才是大纲的新家）；
    // - 挪回「列表栏」：直接把这一栏切到大纲，否则用户点完按钮会觉得大纲凭空消失了，
    //   还得再按一次工具栏开关才找得回来。
    set({ outlinePlacement, listPaneMode: outlinePlacement === 'side' ? 'list' : 'outline' }),
  setListPaneMode: (listPaneMode) => set({ listPaneMode }),
  setAutoOutlineOnSelect: (autoOutlineOnSelect) => set({ autoOutlineOnSelect }),
  setOutlineMode: (outlineMode) => set({ outlineMode }),
  noteActionRequest: null,
  requestNoteAction: (noteActionRequest) => set({ noteActionRequest }),
  clearNoteAction: () => set({ noteActionRequest: null }),
  setEditorSplitRatio: (editorSplitRatio) => {
    // 保留三位小数，既够精细也避免浮点噪声
    const clamped = Math.min(0.7, Math.max(0.3, editorSplitRatio));
    set({ editorSplitRatio: Math.round(clamped * 1000) / 1000 });
  },
  toggleSidebar: () => set((state) => ({ sidebarCollapsed: !state.sidebarCollapsed })),
  toggleCategoriesCollapsed: () =>
    set((state) => ({ categoriesCollapsed: !state.categoriesCollapsed })),
  toggleTagsCollapsed: () => set((state) => ({ tagsCollapsed: !state.tagsCollapsed })),
  setEditorMode: (editorMode) => set({ editorMode }),
  setTheme: (theme) => {
    set({ theme });
    get().applyThemeToDocument();
  },
  setFontSize: (fontSize) => set({ fontSize: Math.min(24, Math.max(12, fontSize)) }),
  setCommandPaletteOpen: (commandPaletteOpen) => set({ commandPaletteOpen }),
  setNewNoteDialogOpen: (newNoteDialogOpen) => set({ newNoteDialogOpen }),
  setQuickSwitcherOpen: (quickSwitcherOpen) => set({ quickSwitcherOpen }),
  pendingHighlight: null,
  setPendingHighlight: (pendingHighlight) => set({ pendingHighlight }),
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
      preferences.outlinePlacement === 'list' ||
      preferences.outlinePlacement === 'side'
    ) {
      next.outlinePlacement = preferences.outlinePlacement;
    }
    if (typeof preferences.autoOutlineOnSelect === 'boolean') {
      next.autoOutlineOnSelect = preferences.autoOutlineOnSelect;
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
