import {
  FileText,
  FlaskConical,
  FolderTree,
  HardDrive,
  Inbox,
  LayoutDashboard,
  ListChecks,
  Network,
  Star,
  Tags,
  Trash2,
} from 'lucide-react';

import { ROUTES } from '@/lib/constants';
import type { WorkspaceStats } from '@/types';

export interface NavItem {
  to: string;
  label: string;
  icon: typeof FileText;
  count?: number;
}

/**
 * 侧边栏导航项（V1.3.0 第二轮界面调整）。
 *
 * 展开态（图标 + 文字 + 计数）与收起态（只剩图标）**共用这一份**：
 * 收起态以前只有「新建笔记」，收起来之后就哪儿也去不了；两处各写一份也迟早对不上。
 *
 * 注意「设置」不在里面：右上角那个齿轮是唯一入口（顶级动作不该有两个入口）。
 */
export function buildNavItems(stats: WorkspaceStats): NavItem[] {
  return [
    { to: ROUTES.dashboard, label: '首页', icon: LayoutDashboard },
    { to: ROUTES.notes, label: '全部笔记', icon: FileText, count: stats.noteCount },
    { to: ROUTES.inbox, label: 'Inbox', icon: Inbox, count: stats.inboxCount },
    { to: ROUTES.favorites, label: '收藏', icon: Star },
    { to: ROUTES.graph, label: '知识图谱', icon: Network },
    { to: ROUTES.research, label: '研究助手', icon: FlaskConical },
    { to: ROUTES.attachments, label: '附件', icon: HardDrive },
    { to: ROUTES.manage, label: '目录管理', icon: ListChecks },
    { to: ROUTES.categories, label: '分类', icon: FolderTree, count: stats.categoryCount },
    { to: ROUTES.tags, label: '标签', icon: Tags, count: stats.tagCount },
    { to: ROUTES.trash, label: '回收站', icon: Trash2, count: stats.trashCount },
  ];
}
