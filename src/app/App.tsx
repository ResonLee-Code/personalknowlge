import { Loader2 } from 'lucide-react';
import { useEffect } from 'react';
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom';
import { Toaster } from 'sonner';

import { AppShell } from '@/components/layout/AppShell';
import { NotesSection } from '@/components/notes/NotesSection';
import { TooltipProvider } from '@/components/ui/tooltip';
import { AboutPage } from '@/features/about/AboutPage';
import { CategoriesPage } from '@/features/categories/CategoriesPage';
import { DashboardPage } from '@/features/dashboard/DashboardPage';
import { AttachmentsPage } from '@/features/attachments/AttachmentsPage';
import { GraphPage } from '@/features/graph/GraphPage';
import { SearchPage } from '@/features/search/SearchPage';
import { SettingsPage } from '@/features/settings/SettingsPage';
import { TagsPage } from '@/features/tags/TagsPage';
import { WelcomePage } from '@/features/workspace/WelcomePage';
import { useWorkspaceEvents } from '@/hooks/useWorkspaceEvents';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';

function BootstrappedApp() {
  const workspace = useWorkspaceStore((state) => state.workspace);
  const loading = useWorkspaceStore((state) => state.loading);
  const bootstrap = useWorkspaceStore((state) => state.bootstrap);
  const applyTheme = useUiStore((state) => state.applyThemeToDocument);

  useWorkspaceEvents();

  useEffect(() => {
    applyTheme();
    void bootstrap();
  }, [applyTheme, bootstrap]);

  if (loading && !workspace) {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-sm text-[var(--ink-muted)]">
        <Loader2 className="h-4 w-4 animate-spin" />
        正在准备工作区…
      </div>
    );
  }

  if (!workspace) {
    return <WelcomePage />;
  }

  return (
    <Routes>
      <Route element={<AppShell />}>
        <Route index element={<DashboardPage />} />
        <Route path="notes" element={<NotesSection baseScope={{ kind: 'all' }} />} />
        <Route path="notes/:id" element={<NotesSection baseScope={{ kind: 'all' }} />} />
        <Route path="inbox" element={<NotesSection baseScope={{ kind: 'inbox' }} />} />
        <Route path="inbox/:id" element={<NotesSection baseScope={{ kind: 'inbox' }} />} />
        <Route path="favorites" element={<NotesSection baseScope={{ kind: 'favorites' }} />} />
        <Route path="favorites/:id" element={<NotesSection baseScope={{ kind: 'favorites' }} />} />
        <Route path="graph" element={<GraphPage />} />
        <Route path="attachments" element={<AttachmentsPage />} />
        <Route path="trash" element={<NotesSection baseScope={{ kind: 'trash' }} />} />
        <Route path="trash/:id" element={<NotesSection baseScope={{ kind: 'trash' }} />} />
        <Route path="missing" element={<NotesSection baseScope={{ kind: 'missing' }} />} />
        <Route path="missing/:id" element={<NotesSection baseScope={{ kind: 'missing' }} />} />
        <Route path="search" element={<SearchPage />} />
        <Route path="categories" element={<CategoriesPage />} />
        <Route path="tags" element={<TagsPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="about" element={<AboutPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}

export function App() {
  return (
    <TooltipProvider delayDuration={200}>
      <HashRouter>
        <BootstrappedApp />
      </HashRouter>
      <Toaster
        position="bottom-right"
        toastOptions={{
          style: {
            background: 'var(--surface-raised)',
            color: 'var(--ink)',
            border: '1px solid var(--border-subtle)',
          },
        }}
      />
    </TooltipProvider>
  );
}
