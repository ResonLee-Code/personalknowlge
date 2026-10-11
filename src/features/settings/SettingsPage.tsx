import { Database, FolderOpen, HardDrive, RefreshCw, ShieldCheck, Sparkles } from 'lucide-react';
import { useEffect, useState, type ReactNode } from 'react';
import { Link } from 'react-router-dom';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { Input, Label } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { AiSettingsPanel } from '@/features/settings/AiSettingsPanel';
import { reportAction } from '@/lib/actionFeedback';
import { AUTOSAVE_DELAY_OPTIONS, FONT_SIZE_OPTIONS, ROUTES, THEME_OPTIONS } from '@/lib/constants';
import { errorMessage, formatBytes, formatDateTime } from '@/lib/format';
import { backupService } from '@/services/backupService';
import { openPath, pickDirectory } from '@/services/appService';
import { isTauriRuntime } from '@/services/api';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { BackupInfo, BackupValidation } from '@/types';

/** 设置页面（设计文档 16.7）。 */
export function SettingsPage() {
  const settings = useWorkspaceStore((state) => state.settings);
  const workspace = useWorkspaceStore((state) => state.workspace);
  const updateSettings = useWorkspaceStore((state) => state.updateSettings);
  const rebuildIndex = useWorkspaceStore((state) => state.rebuildIndex);
  const scan = useWorkspaceStore((state) => state.scan);
  const indexStatus = useWorkspaceStore((state) => state.indexStatus);
  const theme = useUiStore((state) => state.theme);
  const setTheme = useUiStore((state) => state.setTheme);

  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [validation, setValidation] = useState<Record<string, BackupValidation | null>>({});
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    void backupService.list().then(setBackups).catch(() => setBackups([]));
  }, []);

  if (!settings) {
    return <p className="p-6 text-sm text-[var(--ink-subtle)]">正在加载设置…</p>;
  }

  const createBackup = async () => {
    setBusy(true);
    try {
      const info = await backupService.create();
      setBackups(await backupService.list());
      toast.success(`备份完成：${info.fileName}`);
    } catch (error) {
      toast.error(errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  const validate = async (path: string) => {
    try {
      const result = await backupService.validate(path);
      setValidation((previous) => ({ ...previous, [path]: result }));
      toast[result.valid ? 'success' : 'error'](
        result.valid ? '校验通过' : '校验未通过，请查看详情',
      );
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  const restore = async (path: string) => {
    const target = isTauriRuntime()
      ? await pickDirectory('选择恢复目标目录（必须为空）')
      : window.prompt('输入恢复目标目录（必须为空）');
    if (!target) return;
    try {
      const report = await backupService.restore(path, target);
      toast.success(`已恢复 ${report.extractedFiles} 个文件到 ${report.targetDir}`);
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  const exportMarkdown = async () => {
    const target = isTauriRuntime()
      ? await pickDirectory('选择导出目录（必须为空）')
      : window.prompt('输入导出目录（必须为空）');
    if (!target) return;
    try {
      const result = await backupService.exportMarkdown(target);
      toast.success(`已导出 ${result.noteCount} 篇笔记与 ${result.attachmentCount} 个附件`);
    } catch (error) {
      toast.error(errorMessage(error));
    }
  };

  return (
    <div className="h-full w-full overflow-auto p-6">
      <div className="mx-auto w-full max-w-6xl space-y-5">
        <header>
          <h1 className="text-lg font-semibold text-[var(--ink)]">设置</h1>
          <p className="mt-1 text-xs text-[var(--ink-subtle)]">
            设置保存在工作区数据库的 <code>settings</code> 表中，切换工作区互不影响。
          </p>
        </header>

        <div className="grid items-start gap-5 xl:grid-cols-2">
        <Section title="编辑器" icon={Database}>
          <Row label="自动保存" hint="停止输入后自动写入文件（默认 1000ms）">
            <Switch
              checked={settings.autosaveEnabled}
              onCheckedChange={(checked) => void updateSettings({ autosaveEnabled: checked })}
            />
          </Row>
          <Row label="自动保存延迟" hint="200ms ~ 10000ms">
            <select
              aria-label="自动保存延迟"
              value={settings.autosaveDelayMs}
              onChange={(event) =>
                void updateSettings({ autosaveDelayMs: Number(event.target.value) })
              }
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              {AUTOSAVE_DELAY_OPTIONS.map((option) => (
                <option key={option} value={option}>
                  {option} ms
                </option>
              ))}
            </select>
          </Row>
          <Row
            label="默认视图"
            hint="打开笔记时的默认布局；在编辑器顶部切换后会被记住"
          >
            <select
              aria-label="默认视图"
              value={settings.editorMode}
              onChange={(event) => void updateSettings({ editorMode: event.target.value })}
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              <option value="edit">编辑</option>
              <option value="split">分屏</option>
              <option value="preview">预览</option>
            </select>
          </Row>
          <Row label="字号" hint="编辑器与预览的字号">
            <select
              aria-label="字号"
              value={settings.fontSize}
              onChange={(event) => void updateSettings({ fontSize: Number(event.target.value) })}
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              {FONT_SIZE_OPTIONS.map((option) => (
                <option key={option} value={option}>
                  {option}px
                </option>
              ))}
            </select>
          </Row>
          <Row label="界面主题" hint="浅色 / 深色">
            <select
              aria-label="界面主题"
              value={theme}
              onChange={(event) => {
                const value = event.target.value as 'light' | 'dark';
                setTheme(value);
                void updateSettings({ theme: value });
              }}
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              {THEME_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </Row>
          <Row label="新建笔记位置" hint="Inbox 或最近使用的分类">
            <select
              aria-label="新建笔记位置"
              value={settings.defaultNewLocation}
              onChange={(event) =>
                void updateSettings({ defaultNewLocation: event.target.value })
              }
              className="h-8 rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              <option value="inbox">Inbox</option>
              <option value="lastCategory">上次使用的分类</option>
            </select>
          </Row>
        </Section>

        <Section title="索引与数据" icon={ShieldCheck}>
          <Row label="索引状态" hint={`${indexStatus?.indexedNotes ?? 0} / ${indexStatus?.totalNotes ?? 0} 篇已索引`}>
            <span className="text-xs text-[var(--ink-muted)]">{indexStatus?.state ?? 'ready'}</span>
          </Row>
          <Row label="重建索引" hint="清空并重新扫描全部 Markdown 文件">
            <Button
              size="sm"
              variant="secondary"
              onClick={() =>
                void rebuildIndex().then((ok) =>
                  reportAction(ok, '索引已重建', useWorkspaceStore.getState().error, '重建索引失败'),
                )
              }
            >
              <RefreshCw className="h-3.5 w-3.5" />
              重建
            </Button>
          </Row>
          <Row label="重新扫描工作区" hint="采纳外部新增或修改的文件">
            <Button
              size="sm"
              variant="secondary"
              onClick={() =>
                void scan().then((report) =>
                  report
                    ? toast.success(
                        `新增 ${report.added}，更新 ${report.updated}，缺失 ${report.missing}`,
                      )
                    : toast.error('扫描失败'),
                )
              }
            >
              <RefreshCw className="h-3.5 w-3.5" />
              扫描
            </Button>
          </Row>
          <Row label="工作区目录" hint={workspace?.rootPath ?? '—'}>
            <Button
              size="sm"
              variant="secondary"
              onClick={() =>
                void openPath(workspace?.rootPath ?? '').catch(() =>
                  toast.error('无法打开目录'),
                )
              }
            >
              <FolderOpen className="h-3.5 w-3.5" />
              打开
            </Button>
          </Row>
        </Section>

        <Section title="备份与导出" icon={HardDrive}>
          <Row label="备份目录" hint={settings.backupDir ?? '默认：.knowledge/backups'}>
            <Button
              size="sm"
              variant="secondary"
              onClick={async () => {
                const dir = isTauriRuntime() ? await pickDirectory('选择备份目录') : window.prompt('输入备份目录');
                if (dir) void updateSettings({ backupDir: dir });
              }}
            >
              修改
            </Button>
          </Row>
          <Row label="创建备份" hint="打包 Markdown、附件、数据库与配置">
            <Button size="sm" onClick={() => void createBackup()} disabled={busy}>
              立即备份
            </Button>
          </Row>
          <Row label="导出 Markdown" hint="把全部笔记导出为普通目录">
            <Button size="sm" variant="secondary" onClick={() => void exportMarkdown()}>
              导出
            </Button>
          </Row>
          <div className="space-y-2 px-3 py-2">
            {backups.length === 0 ? (
              <p className="text-xs text-[var(--ink-subtle)]">暂无备份文件。</p>
            ) : (
              backups.map((backup) => (
                <div
                  key={backup.path}
                  className="rounded-lg border border-[var(--border-subtle)] p-2.5"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="min-w-0 flex-1 truncate text-xs font-medium text-[var(--ink)]">
                      {backup.fileName}
                    </span>
                    <span className="text-[11px] text-[var(--ink-subtle)]">
                      {formatBytes(backup.sizeBytes)}
                    </span>
                  </div>
                  <div className="mt-0.5 text-[11px] text-[var(--ink-subtle)]">
                    {formatDateTime(backup.createdAt)} · {backup.noteCount} 篇笔记 ·{' '}
                    {backup.includesDatabase ? '含数据库' : '不含数据库'}
                  </div>
                  <div className="mt-1.5 flex gap-2">
                    <Button size="sm" variant="ghost" onClick={() => void validate(backup.path)}>
                      校验
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => void restore(backup.path)}>
                      恢复
                    </Button>
                  </div>
                  {validation[backup.path] ? (
                    <ul className="mt-1.5 space-y-0.5 text-[11px] text-[var(--ink-subtle)]">
                      {validation[backup.path]?.checks.map((check) => (
                        <li key={check.name} className={check.passed ? '' : 'text-red-500'}>
                          {check.passed ? '✓' : '✗'} {check.name}：{check.detail}
                        </li>
                      ))}
                    </ul>
                  ) : null}
                </div>
              ))
            )}
          </div>
        </Section>

        <Section title="AI（第二代预留）" icon={Sparkles} testId="settings-ai-section">
          <AiSettingsPanel />
        </Section>

        <Section title="关于" icon={ShieldCheck}>
          <Row label="版本与运行信息" hint="包含应用版本、数据库版本与日志位置">
            <Button asChild size="sm" variant="secondary">
              <Link to={ROUTES.about}>查看</Link>
            </Button>
          </Row>
          <div className="px-3 py-2">
            <Label className="text-[11px] text-[var(--ink-subtle)]">
              工作区 ID：{workspace?.id ?? '—'}
            </Label>
            <Input
              aria-label="当前知识库目录" readOnly value={workspace?.rootPath ?? ''} className="mt-1 h-8 text-xs" />
          </div>
        </Section>
        </div>
      </div>
    </div>
  );
}

function Section({
  title,
  icon: Icon,
  children,
  testId,
}: {
  title: string;
  icon: typeof Database;
  children: ReactNode;
  /** 供组件测试 / E2E 定位该区块 */
  testId?: string;
}) {
  return (
    <section
      data-testid={testId}
      className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)]"
    >
      <header className="flex items-center gap-2 border-b border-[var(--border-subtle)] px-3 py-2">
        <Icon className="h-4 w-4 text-[var(--brand)]" />
        <h2 className="text-sm font-semibold text-[var(--ink)]">{title}</h2>
      </header>
      <div className="divide-y divide-[var(--border-subtle)]">{children}</div>
    </section>
  );
}

function Row({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-4 px-3 py-2.5">
      <div className="min-w-0">
        <p className="text-sm text-[var(--ink)]">{label}</p>
        {hint ? (
          <p className="mt-0.5 truncate text-[11px] text-[var(--ink-subtle)]">{hint}</p>
        ) : null}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}
