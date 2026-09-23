import { FolderOpen, Library, Plus, ShieldCheck, Sparkles, Zap } from 'lucide-react';
import { useEffect, useState } from 'react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { Input, Label } from '@/components/ui/input';
import { formatRelative } from '@/lib/format';
import { pickDirectory } from '@/services/appService';
import { isTauriRuntime } from '@/services/api';
import { useWorkspaceStore } from '@/stores/workspaceStore';

/** 未打开工作区时的入口页。 */
export function WelcomePage() {
  const workspaces = useWorkspaceStore((state) => state.workspaces);
  const loading = useWorkspaceStore((state) => state.loading);
  const error = useWorkspaceStore((state) => state.error);
  const createWorkspace = useWorkspaceStore((state) => state.createWorkspace);
  const openWorkspace = useWorkspaceStore((state) => state.openWorkspace);
  const refreshList = useWorkspaceStore((state) => state.refreshList);

  const [name, setName] = useState('我的知识库');
  const [root, setRoot] = useState('');

  useEffect(() => {
    void refreshList();
  }, [refreshList]);

  const chooseDirectory = async (title: string) => {
    if (!isTauriRuntime()) {
      toast.info('浏览器预览模式：请直接输入目录路径');
      return null;
    }
    return pickDirectory(title);
  };

  const handleCreate = async () => {
    let target = root.trim();
    if (!target) {
      const picked = await chooseDirectory('选择知识库保存目录');
      if (!picked) return;
      target = picked;
    }
    const ok = await createWorkspace(name.trim() || '我的知识库', target);
    if (ok) toast.success('知识库已创建');
    else toast.error(useWorkspaceStore.getState().error ?? '创建失败');
  };

  const handleOpen = async () => {
    let target = root.trim();
    if (!target) {
      const picked = await chooseDirectory('选择已有知识库目录');
      if (!picked) return;
      target = picked;
    }
    const ok = await openWorkspace(target);
    if (ok) toast.success('知识库已打开');
    else toast.error(useWorkspaceStore.getState().error ?? '打开失败');
  };

  return (
    <div className="flex h-full items-center justify-center overflow-auto bg-[var(--surface-muted)] p-6">
      <div className="grid w-full max-w-4xl gap-6 lg:grid-cols-[1.1fr_0.9fr]">
        <section className="space-y-5 rounded-2xl border border-[var(--border-subtle)] bg-[var(--surface)] p-7">
          <div className="flex items-center gap-2 text-[var(--brand)]">
            <Library className="h-5 w-5" />
            <span className="text-xs font-semibold uppercase tracking-wide">Personal Knowledge OS</span>
          </div>
          <div className="space-y-2">
            <h1 className="text-2xl font-semibold leading-snug text-[var(--ink)]">
              本地优先的个人知识库
            </h1>
            <p className="text-sm leading-relaxed text-[var(--ink-muted)]">
              笔记以 Markdown 文件保存在你自己的目录中，应用只负责分类、标签、全文检索与备份。
              数据完全自主，随时可以用其他工具打开。
            </p>
          </div>

          <div className="space-y-3 rounded-xl border border-[var(--border-subtle)] bg-[var(--surface-muted)] p-4">
            <div className="space-y-1.5">
              <Label htmlFor="workspace-name">知识库名称</Label>
              <Input
                id="workspace-name"
                value={name}
                onChange={(event) => setName(event.target.value)}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="workspace-root">保存目录（绝对路径）</Label>
              <Input
                id="workspace-root"
                value={root}
                placeholder="例如 /Users/you/Documents/MyKnowledge"
                onChange={(event) => setRoot(event.target.value)}
              />
            </div>
            <div className="flex flex-wrap gap-2">
              <Button onClick={() => void handleCreate()} disabled={loading}>
                <Plus className="h-4 w-4" />
                创建知识库
              </Button>
              <Button variant="secondary" onClick={() => void handleOpen()} disabled={loading}>
                <FolderOpen className="h-4 w-4" />
                打开已有目录
              </Button>
            </div>
            {error ? <p className="text-xs text-red-500">{error}</p> : null}
          </div>

          <ul className="grid gap-2 text-xs text-[var(--ink-muted)] sm:grid-cols-3">
            <li className="flex items-center gap-1.5">
              <ShieldCheck className="h-3.5 w-3.5 text-[var(--brand)]" />
              本地优先，不上传内容
            </li>
            <li className="flex items-center gap-1.5">
              <Zap className="h-3.5 w-3.5 text-[var(--brand)]" />
              SQLite FTS5 全文检索
            </li>
            <li className="flex items-center gap-1.5">
              <Sparkles className="h-3.5 w-3.5 text-[var(--brand)]" />
              Markdown 文件即数据
            </li>
          </ul>
        </section>

        <section className="space-y-3 rounded-2xl border border-[var(--border-subtle)] bg-[var(--surface)] p-7">
          <h2 className="text-sm font-semibold text-[var(--ink)]">最近打开</h2>
          {workspaces.length === 0 ? (
            <p className="text-xs text-[var(--ink-subtle)]">
              还没有记录。创建或打开一个目录后会显示在这里。
            </p>
          ) : (
            <ul className="space-y-2">
              {workspaces.map((item) => (
                <li key={item.workspace.id}>
                  <button
                    type="button"
                    onClick={() => void openWorkspace(item.workspace.rootPath)}
                    className="w-full rounded-lg border border-[var(--border-subtle)] px-3 py-2 text-left transition-colors hover:border-[var(--brand)]"
                  >
                    <span className="flex items-center justify-between gap-2">
                      <span className="truncate text-sm font-medium text-[var(--ink)]">
                        {item.workspace.name}
                      </span>
                      {!item.isAvailable ? (
                        <span className="text-[11px] text-red-500">目录不可用</span>
                      ) : null}
                    </span>
                    <span className="mt-0.5 block truncate text-[11px] text-[var(--ink-subtle)]">
                      {item.workspace.rootPath}
                    </span>
                    <span className="mt-0.5 block text-[11px] text-[var(--ink-subtle)]">
                      最近打开 {formatRelative(item.workspace.updatedAt)}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
