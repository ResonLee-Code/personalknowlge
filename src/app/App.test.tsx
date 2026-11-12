import { render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { App } from '@/app/App';
import * as api from '@/services/api';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { Workspace } from '@/types';

const deepLink = vi.hoisted(() => ({
  getCurrent: vi.fn(),
  onOpenUrl: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-deep-link', () => deepLink);
vi.mock('@/hooks/useWindowBounds', () => ({ useWindowBounds: () => undefined }));
vi.mock('@/hooks/useWorkspaceEvents', () => ({ useWorkspaceEvents: () => undefined }));
vi.mock('@/hooks/useKeyboardShortcuts', () => ({ useKeyboardShortcuts: () => undefined }));
vi.mock('@/components/layout/Sidebar', () => ({ Sidebar: () => <aside>侧边栏</aside> }));
vi.mock('@/components/layout/TopBar', () => ({ TopBar: () => <header>顶栏</header> }));
vi.mock('@/components/layout/StatusBar', () => ({ StatusBar: () => <footer>状态栏</footer> }));
vi.mock('@/components/ask/AskKnowledgePanel', () => ({ AskKnowledgePanel: () => null }));
vi.mock('@/components/search/GlobalSearchDialog', () => ({ GlobalSearchDialog: () => null }));
vi.mock('@/components/search/QuickSwitcherDialog', () => ({ QuickSwitcherDialog: () => null }));
vi.mock('@/components/notes/NewNoteDialog', () => ({ NewNoteDialog: () => null }));
vi.mock('@/components/notes/NotesSection', () => ({ NotesSection: () => <div>笔记</div> }));
vi.mock('@/features/dashboard/DashboardPage', () => ({ DashboardPage: () => <div>首页</div> }));
vi.mock('@/features/discover/ContentDiscoveryPage', () => ({
  ContentDiscoveryPage: () => <div data-testid="discover-page">内容发现</div>,
}));
vi.mock('@/features/about/AboutPage', () => ({ AboutPage: () => <div>关于</div> }));
vi.mock('@/features/attachments/AttachmentsPage', () => ({
  AttachmentsPage: () => <div>附件</div>,
}));
vi.mock('@/features/categories/CategoriesPage', () => ({ CategoriesPage: () => <div>分类</div> }));
vi.mock('@/features/graph/GraphPage', () => ({ GraphPage: () => <div>图谱</div> }));
vi.mock('@/features/manage/ManagePage', () => ({ ManagePage: () => <div>管理</div> }));
vi.mock('@/features/organize/OrganizePage', () => ({ OrganizePage: () => <div>整理</div> }));
vi.mock('@/features/research/ResearchPage', () => ({ ResearchPage: () => <div>研究</div> }));
vi.mock('@/features/search/SearchPage', () => ({ SearchPage: () => <div>搜索</div> }));
vi.mock('@/features/settings/SettingsPage', () => ({ SettingsPage: () => <div>设置</div> }));
vi.mock('@/features/tags/TagsPage', () => ({ TagsPage: () => <div>标签</div> }));
vi.mock('@/features/workspace/WelcomePage', () => ({ WelcomePage: () => <div>欢迎</div> }));

const workspace: Workspace = {
  id: 'workspace_1',
  name: '测试知识库',
  rootPath: '/tmp/pks',
  createdAt: '2026-10-25T01:00:00+08:00',
  updatedAt: '2026-10-25T01:00:00+08:00',
};

function setWorkspace() {
  useWorkspaceStore.setState({
    workspace,
    loading: false,
    settings: null,
    indexStatus: null,
    bootstrap: vi.fn().mockResolvedValue(undefined),
  });
}

describe('App', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.location.hash = '#/notes';
    setWorkspace();
    useUiStore.setState({
      askPanelOpen: false,
      noteActionRequest: null,
      commandPaletteOpen: false,
      newNoteDialogOpen: false,
      quickSwitcherOpen: false,
    });
  });

  it('打开工作区后渲染内容发现路由和工作台外壳', async () => {
    vi.spyOn(api, 'isTauriRuntime').mockReturnValue(false);
    window.location.hash = '#/discover';

    render(<App />);

    expect(await screen.findByTestId('discover-page')).toBeInTheDocument();
    expect(screen.getByTestId('app-main-surface')).toHaveClass(
      'min-[1440px]:rounded-none',
      'rounded-tl-lg',
    );
    expect(screen.getByText('侧边栏')).toBeInTheDocument();
  });

  it('启动时消费 pkos capture 深链并导航到内容发现', async () => {
    vi.spyOn(api, 'isTauriRuntime').mockReturnValue(true);
    deepLink.getCurrent.mockResolvedValue([
      'pkos://capture?url=https%3A%2F%2Fexample.com%2Farticle&note=%E7%A8%8D%E5%90%8E%E9%98%85%E8%AF%BB',
    ]);
    const unlisten = vi.fn();
    deepLink.onOpenUrl.mockResolvedValue(unlisten);

    const view = render(<App />);

    await waitFor(() =>
      expect(window.location.hash).toContain(
        '/discover?url=https%3A%2F%2Fexample.com%2Farticle&note=%E7%A8%8D%E5%90%8E%E9%98%85%E8%AF%BB',
      ),
    );
    expect(await screen.findByTestId('discover-page')).toBeInTheDocument();
    view.unmount();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('深链 API 不可用时不影响主界面启动', async () => {
    vi.spyOn(api, 'isTauriRuntime').mockReturnValue(true);
    deepLink.getCurrent.mockRejectedValue(new Error('deep link unavailable'));
    deepLink.onOpenUrl.mockRejectedValue(new Error('listener unavailable'));

    render(<App />);

    expect(await screen.findByText('笔记')).toBeInTheDocument();
    await waitFor(() => expect(deepLink.onOpenUrl).toHaveBeenCalledTimes(1));
  });
});
