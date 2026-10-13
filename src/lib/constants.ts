/** 路由常量（设计文档 16.2）。 */
export const ROUTES = {
  dashboard: '/',
  notes: '/notes',
  note: (id: string) => `/notes/${id}`,
  search: '/search',
  inbox: '/inbox',
  categories: '/categories',
  tags: '/tags',
  favorites: '/favorites',
  graph: '/graph',
  research: '/research',
  attachments: '/attachments',
  manage: '/manage',
  trash: '/trash',
  settings: '/settings',
  about: '/about',
} as const;

/** 前端需要监听的后端事件名。 */
export const EVENTS = {
  filesChanged: 'workspace://files-changed',
  notesChanged: 'notes://changed',
  indexStatus: 'search://index-status',
  /** 研究助手进度（只含元数据：步骤 / 已抓页数 / 来源数） */
  researchProgress: 'ai://research-progress',
  /** 报告「推测段」的生成增量（Track C1 流式；只是预览，最终以报告为准） */
  researchComposeDelta: 'ai://research-compose-delta',
  /** 向量索引构建进度（D3）：`{ done, total, skipped }`，只含计数、不含正文 */
  vectorIndexProgress: 'ai://vector-index-progress',
} as const;

/** 默认工作区目录约定（与 Rust 端保持一致）。 */
export const LAYOUT = {
  knowledge: '.knowledge',
  trash: '.trash',
  attachments: 'attachments',
  inbox: '00_Inbox',
} as const;

export const AUTOSAVE_DELAY_OPTIONS = [500, 800, 1000, 1500, 2000, 3000];
export const FONT_SIZE_OPTIONS = [13, 14, 15, 16, 17, 18, 20];
export const EDITOR_MODES = [
  { value: 'edit', label: '编辑' },
  { value: 'split', label: '分屏' },
  { value: 'preview', label: '预览' },
] as const;
export const THEME_OPTIONS = [
  { value: 'light', label: '浅色' },
  { value: 'dark', label: '深色' },
] as const;

/** 常用快捷键提示（设计文档 17.1）。 */
export const SHORTCUTS = [
  { keys: '⌘/Ctrl + N', action: '新建笔记' },
  { keys: '⌘/Ctrl + K', action: '全局搜索' },
  { keys: '⌘/Ctrl + P', action: '快速切换笔记（按标题模糊匹配）' },
  { keys: '⌘/Ctrl + S', action: '立即保存' },
  { keys: '⌘/Ctrl + E', action: '聚焦编辑器' },
  { keys: '⌘/Ctrl + W', action: '关闭当前笔记' },
  { keys: 'Delete', action: '删除选中笔记' },
] as const;
