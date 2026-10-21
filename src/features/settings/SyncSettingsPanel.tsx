import {
  AlertTriangle,
  Cloud,
  Download,
  History,
  RefreshCw,
  ShieldCheck,
  Upload,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input, Label } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import { reportAction, reportFailure } from '@/lib/actionFeedback';
import { errorMessage, formatBytes, formatDateTime } from '@/lib/format';
import { syncService } from '@/services/syncService';
import type {
  ResolveSyncConflictInput,
  SyncConflictDetail,
  SyncConflictSummary,
  SyncHistoryEntry,
  SyncPreview,
  SyncSettings,
  UpdateSyncSettingsInput,
} from '@/types';

interface SecretState {
  username: string;
  password: string;
  accessKey: string;
  secretKey: string;
}

const EMPTY_SECRETS: SecretState = {
  username: '',
  password: '',
  accessKey: '',
  secretKey: '',
};

function errorText(cause: unknown): string {
  return errorMessage(cause);
}

function PlanGroup({
  title,
  items,
  icon: Icon,
  tone,
}: {
  title: string;
  items: SyncPreview['uploads'];
  icon: typeof Upload;
  tone?: string;
}) {
  if (items.length === 0) return null;
  return (
    <section className="space-y-1">
      <h4
        className={`flex items-center gap-1.5 text-xs font-medium ${tone ?? 'text-[var(--ink)]'}`}
      >
        <Icon className="h-3.5 w-3.5" />
        {title}（{items.length}）
      </h4>
      <ul className="max-h-32 space-y-0.5 overflow-y-auto">
        {items.slice(0, 50).map((item) => (
          <li
            key={`${item.kind}-${item.path}`}
            className="flex items-center gap-2 rounded bg-[var(--surface-muted)] px-2 py-1 text-[10px]"
          >
            <span className="min-w-0 flex-1 truncate text-[var(--ink)]" title={item.path}>
              {item.path}
            </span>
            {item.sizeBytes > 0 ? (
              <span className="shrink-0 text-[var(--ink-subtle)]">
                {formatBytes(item.sizeBytes)}
              </span>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  );
}

/** Sync 2.0 配置、dry-run、执行与冲突解决。 */
export function SyncSettingsPanel() {
  const [settings, setSettings] = useState<SyncSettings | null>(null);
  const [secrets, setSecrets] = useState<SecretState>(EMPTY_SECRETS);
  const [passphrase, setPassphrase] = useState('');
  const [preview, setPreview] = useState<SyncPreview | null>(null);
  const [conflicts, setConflicts] = useState<SyncConflictSummary[]>([]);
  const [history, setHistory] = useState<SyncHistoryEntry[]>([]);
  const [conflict, setConflict] = useState<SyncConflictDetail | null>(null);
  const [mergedContent, setMergedContent] = useState('');
  const [mergeNote, setMergeNote] = useState<string | null>(null);
  const [busy, setBusy] = useState<'save' | 'preview' | 'run' | 'resolve' | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [next, nextConflicts, nextHistory] = await Promise.all([
        syncService.settings(),
        syncService.conflicts(),
        syncService.history(),
      ]);
      setSettings(next);
      setConflicts(nextConflicts);
      setHistory(nextHistory);
      setLoadError(null);
    } catch (cause) {
      setLoadError(errorText(cause));
    }
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => void load(), 0);
    return () => window.clearTimeout(timer);
  }, [load]);

  const dirty = useMemo(
    () =>
      Boolean(
        preview || passphrase || Object.values(secrets).some((value) => value.trim().length > 0),
      ),
    [passphrase, preview, secrets],
  );

  const patch = (value: Partial<SyncSettings>) => {
    setSettings((current) => (current ? { ...current, ...value } : current));
    setPreview(null);
  };

  const saveSettings = async () => {
    if (!settings) return;
    const input: UpdateSyncSettingsInput = {
      enabled: settings.enabled,
      provider: settings.provider,
      endpoint: settings.endpoint ?? '',
      bucket: settings.bucket ?? '',
      region: settings.region,
      syncNotes: settings.syncNotes,
      syncAttachments: settings.syncAttachments,
      syncIndex: settings.syncIndex,
      ...(secrets.username.trim() ? { username: secrets.username } : {}),
      ...(secrets.password ? { password: secrets.password } : {}),
      ...(secrets.accessKey.trim() ? { accessKey: secrets.accessKey } : {}),
      ...(secrets.secretKey ? { secretKey: secrets.secretKey } : {}),
    };
    setBusy('save');
    try {
      const saved = await syncService.updateSettings(input);
      setSettings(saved);
      setSecrets(EMPTY_SECRETS);
      setPreview(null);
      toast.success('同步设置已保存在应用数据目录');
    } catch (cause) {
      reportFailure('保存同步设置失败', errorText(cause));
    } finally {
      setBusy(null);
    }
  };

  const runPreview = async () => {
    setBusy('preview');
    try {
      const next = await syncService.preview(passphrase);
      setPreview(next);
      toast.success('dry-run 完成，尚未修改任何文件');
    } catch (cause) {
      setPreview(null);
      reportFailure('同步预检失败', errorText(cause));
    } finally {
      setBusy(null);
    }
  };

  const runSync = async () => {
    setBusy('run');
    try {
      const result = await syncService.run({ passphrase, confirm: true });
      setPreview(result.preview);
      setConflicts(await syncService.conflicts());
      toast.success(
        `同步完成：上传 ${result.uploaded}、下载 ${result.downloaded}、冲突 ${result.conflictsCreated}`,
      );
      await load();
    } catch (cause) {
      reportFailure('同步失败，本地文件未被静默覆盖', errorText(cause));
    } finally {
      setBusy(null);
    }
  };

  const openConflict = async (summary: SyncConflictSummary) => {
    try {
      const detail = await syncService.conflictDetail(summary.id);
      setConflict(detail);
      setMergedContent(detail.localContent ?? '');
      setMergeNote(null);
    } catch (cause) {
      reportFailure('读取冲突版本失败', errorText(cause));
    }
  };

  const mergeConflict = async () => {
    if (!conflict) return;
    try {
      const merged = await syncService.conflictMergePreview(conflict.summary.id);
      setMergeNote(merged.note);
      if (merged.mergedContent !== null && merged.mergedContent !== undefined) {
        setMergedContent(merged.mergedContent);
      }
    } catch (cause) {
      reportFailure('三方合并预览失败', errorText(cause));
    }
  };

  const resolveConflict = async (resolution: ResolveSyncConflictInput['resolution']) => {
    if (!conflict) return;
    const input: ResolveSyncConflictInput = {
      conflictId: conflict.summary.id,
      resolution,
      ...(resolution === 'merged' ? { mergedContent } : {}),
    };
    setBusy('resolve');
    try {
      await syncService.resolveConflict(input);
      setConflict(null);
      const ok = true;
      reportAction(ok, '冲突已解决；下一次同步会传播所选版本', null, '冲突处理失败');
      await load();
    } catch (cause) {
      reportFailure('冲突处理失败', errorText(cause));
    } finally {
      setBusy(null);
    }
  };

  const restoreHistory = async (entry: SyncHistoryEntry) => {
    if (
      !window.confirm(
        `把 ${entry.originalPath} 还原为 ${formatDateTime(entry.savedAt)} 的版本？\n当前文件会先留档。`,
      )
    ) {
      return;
    }
    try {
      const path = await syncService.restoreHistory({
        relativePath: entry.relativePath,
        confirm: true,
      });
      toast.success(`已还原 ${path}；当前版本已留档`);
      await load();
    } catch (cause) {
      reportFailure('还原同步历史失败', errorText(cause));
    }
  };

  if (loadError) {
    return (
      <div className="space-y-2 rounded-lg border border-red-500/30 p-4">
        <p className="text-sm text-[var(--ink)]">同步状态读取失败</p>
        <p className="text-xs text-[var(--ink-subtle)]">{loadError}</p>
        <Button size="sm" variant="secondary" onClick={() => void load()}>
          重试
        </Button>
      </div>
    );
  }
  if (!settings) {
    return <p className="text-xs text-[var(--ink-subtle)]">正在读取同步设置…</p>;
  }

  const isS3 = settings.provider === 's3';
  const canPreview =
    settings.enabled &&
    Boolean(settings.endpoint?.trim()) &&
    passphrase.trim().length >= 12 &&
    !busy;

  return (
    <div className="space-y-5" data-testid="sync-settings">
      <section className="rounded-xl border border-[var(--border-subtle)] p-4">
        <div className="mb-3 flex items-start justify-between gap-4">
          <div>
            <h3 className="flex items-center gap-2 text-sm font-semibold text-[var(--ink)]">
              <Cloud className="h-4 w-4" /> 可选加密同步
            </h3>
            <p className="mt-1 text-[11px] leading-relaxed text-[var(--ink-subtle)]">
              只连接你自己的 WebDAV / S3
              兼容存储。正文与附件先在本机加密；服务端只收到密文与随机对象名。
            </p>
          </div>
          <label className="flex items-center gap-2 text-xs text-[var(--ink-muted)]">
            {settings.pendingQueue > 0 ? `待重试 ${settings.pendingQueue} 项` : '开启同步'}
            <Switch
              aria-label="开启同步"
              checked={settings.enabled}
              onCheckedChange={(enabled) => patch({ enabled })}
            />
          </label>
        </div>
        <div className="grid gap-3 md:grid-cols-2">
          <div className="space-y-1">
            <Label htmlFor="sync-provider">存储类型</Label>
            <select
              id="sync-provider"
              value={settings.provider}
              onChange={(event) => patch({ provider: event.target.value as 'webdav' | 's3' })}
              className="h-8 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] px-2 text-xs"
            >
              <option value="webdav">WebDAV</option>
              <option value="s3">S3 兼容</option>
            </select>
          </div>
          <div className="space-y-1">
            <Label htmlFor="sync-endpoint">Endpoint</Label>
            <Input
              id="sync-endpoint"
              value={settings.endpoint ?? ''}
              onChange={(event) => patch({ endpoint: event.target.value })}
              placeholder={isS3 ? 'https://s3.example.com' : 'https://dav.example.com/pkos'}
              className="h-8 text-xs"
            />
          </div>
          {isS3 ? (
            <>
              <div className="space-y-1">
                <Label htmlFor="sync-bucket">Bucket</Label>
                <Input
                  id="sync-bucket"
                  value={settings.bucket ?? ''}
                  onChange={(event) => patch({ bucket: event.target.value })}
                  className="h-8 text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="sync-region">Region</Label>
                <Input
                  id="sync-region"
                  value={settings.region}
                  onChange={(event) => patch({ region: event.target.value })}
                  className="h-8 text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="sync-access-key">
                  Access Key{settings.accessKeySet ? '（已配置）' : ''}
                </Label>
                <Input
                  id="sync-access-key"
                  value={secrets.accessKey}
                  onChange={(event) =>
                    setSecrets((current) => ({ ...current, accessKey: event.target.value }))
                  }
                  autoComplete="off"
                  className="h-8 text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="sync-secret-key">Secret Key（只写不读）</Label>
                <Input
                  id="sync-secret-key"
                  type="password"
                  value={secrets.secretKey}
                  onChange={(event) =>
                    setSecrets((current) => ({ ...current, secretKey: event.target.value }))
                  }
                  autoComplete="new-password"
                  className="h-8 text-xs"
                />
              </div>
            </>
          ) : (
            <>
              <div className="space-y-1">
                <Label htmlFor="sync-username">
                  用户名{settings.usernameSet ? '（已配置）' : ''}
                </Label>
                <Input
                  id="sync-username"
                  value={secrets.username}
                  onChange={(event) =>
                    setSecrets((current) => ({ ...current, username: event.target.value }))
                  }
                  autoComplete="username"
                  className="h-8 text-xs"
                />
              </div>
              <div className="space-y-1">
                <Label htmlFor="sync-password">密码（只写不读）</Label>
                <Input
                  id="sync-password"
                  type="password"
                  value={secrets.password}
                  onChange={(event) =>
                    setSecrets((current) => ({ ...current, password: event.target.value }))
                  }
                  autoComplete="new-password"
                  className="h-8 text-xs"
                />
              </div>
            </>
          )}
        </div>
        <div className="mt-3 flex flex-wrap items-center gap-4 text-xs text-[var(--ink-muted)]">
          <label className="flex items-center gap-2">
            <Switch
              aria-label="同步笔记正文"
              checked={settings.syncNotes}
              onCheckedChange={(syncNotes) => patch({ syncNotes })}
            />
            笔记
          </label>
          <label className="flex items-center gap-2">
            <Switch
              aria-label="同步附件"
              checked={settings.syncAttachments}
              onCheckedChange={(syncAttachments) => patch({ syncAttachments })}
            />
            附件
          </label>
          <label className="flex items-center gap-2">
            <Switch
              aria-label="同步索引缓存"
              checked={settings.syncIndex}
              onCheckedChange={(syncIndex) => patch({ syncIndex })}
            />
            索引缓存
          </label>
        </div>
        <div className="mt-3 flex justify-end">
          <Button size="sm" onClick={() => void saveSettings()} disabled={busy !== null}>
            {busy === 'save' ? '保存中…' : '保存同步设置'}
          </Button>
        </div>
      </section>

      <section className="rounded-xl border border-[var(--border-subtle)] p-4">
        <h3 className="flex items-center gap-2 text-sm font-semibold text-[var(--ink)]">
          <ShieldCheck className="h-4 w-4" /> 加密预检
        </h3>
        <p className="mt-1 text-[11px] leading-relaxed text-[var(--ink-subtle)]">
          口令至少 12 个字符，只用于本轮派生密钥，不写入磁盘。两台设备必须使用同一口令。
        </p>
        <div className="mt-3 flex flex-wrap items-end gap-2">
          <div className="min-w-64 flex-1 space-y-1">
            <Label htmlFor="sync-passphrase">同步口令</Label>
            <Input
              id="sync-passphrase"
              type="password"
              value={passphrase}
              onChange={(event) => {
                setPassphrase(event.target.value);
                setPreview(null);
              }}
              autoComplete="new-password"
              placeholder="至少 12 个字符"
              className="h-8 text-xs"
            />
          </div>
          <Button
            size="sm"
            variant="secondary"
            disabled={!canPreview}
            onClick={() => void runPreview()}
          >
            <RefreshCw
              className={busy === 'preview' ? 'h-3.5 w-3.5 animate-spin' : 'h-3.5 w-3.5'}
            />
            dry-run
          </Button>
          <Button size="sm" disabled={!canPreview || !preview} onClick={() => void runSync()}>
            {busy === 'run' ? '同步中…' : '确认并同步'}
          </Button>
        </div>
        {dirty && !preview ? (
          <p className="mt-2 text-[10px] text-amber-600">设置或口令已修改，请重新 dry-run。</p>
        ) : null}
        {preview ? (
          <div className="mt-4 space-y-3" data-testid="sync-preview">
            <div className="flex flex-wrap gap-3 text-[11px] text-[var(--ink-muted)]">
              <span>上传 {preview.uploads.length}</span>
              <span>下载 {preview.downloads.length}</span>
              <span>远端删除 {preview.remoteDeletes.length}</span>
              <span>本地删除 {preview.localDeletes.length}</span>
              <span>冲突 {preview.conflicts.length}</span>
              <span>{formatBytes(preview.totalBytes)}</span>
            </div>
            {preview.warnings.map((warning) => (
              <p
                key={warning}
                className="rounded border border-amber-500/30 bg-amber-500/10 px-2 py-1.5 text-[10px] text-amber-700 dark:text-amber-400"
              >
                {warning}
              </p>
            ))}
            <PlanGroup title="将上传" items={preview.uploads} icon={Upload} />
            <PlanGroup title="将下载" items={preview.downloads} icon={Download} />
            <PlanGroup
              title="冲突（不会覆盖）"
              items={preview.conflicts}
              icon={AlertTriangle}
              tone="text-amber-600"
            />
            <PlanGroup
              title="远端墓碑（本地删除）"
              items={preview.localDeletes}
              icon={History}
              tone="text-red-600"
            />
          </div>
        ) : null}
      </section>

      <section className="rounded-xl border border-[var(--border-subtle)] p-4">
        <div className="flex items-center justify-between gap-3">
          <h3 className="flex items-center gap-2 text-sm font-semibold text-[var(--ink)]">
            <AlertTriangle className="h-4 w-4" /> 冲突与历史
          </h3>
          <Button size="sm" variant="ghost" onClick={() => void load()}>
            刷新
          </Button>
        </div>
        {conflicts.length === 0 ? (
          <p className="mt-2 text-[11px] text-[var(--ink-subtle)]">当前没有待处理的同步冲突。</p>
        ) : (
          <ul className="mt-2 space-y-1">
            {conflicts.map((item) => (
              <li key={item.id}>
                <button
                  type="button"
                  className="flex w-full items-center gap-2 rounded-md border border-amber-500/30 bg-amber-500/5 px-2 py-1.5 text-left"
                  onClick={() => void openConflict(item)}
                >
                  <span className="min-w-0 flex-1 truncate text-xs text-[var(--ink)]">
                    {item.path}
                  </span>
                  <span className="text-[10px] text-[var(--ink-subtle)]">
                    {formatDateTime(item.detectedAt)}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="mt-4 space-y-1 border-t border-[var(--border-subtle)] pt-3">
          <h4 className="flex items-center gap-1.5 text-xs font-medium text-[var(--ink)]">
            <History className="h-3.5 w-3.5" /> 最近留档
          </h4>
          {history.length === 0 ? (
            <p className="text-[11px] text-[var(--ink-subtle)]">
              尚未产生同步历史；覆盖或删除前会自动留档。
            </p>
          ) : (
            <ul className="max-h-48 space-y-1 overflow-y-auto">
              {history.slice(0, 30).map((entry) => (
                <li
                  key={entry.relativePath}
                  className="flex items-center gap-2 rounded bg-[var(--surface-muted)] px-2 py-1"
                >
                  <span
                    className="min-w-0 flex-1 truncate text-[10px] text-[var(--ink)]"
                    title={entry.originalPath}
                  >
                    {entry.originalPath}
                  </span>
                  <span className="shrink-0 text-[9px] text-[var(--ink-subtle)]">
                    {entry.category} · {formatDateTime(entry.savedAt)}
                  </span>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="h-6 px-1.5 text-[9px]"
                    onClick={() => void restoreHistory(entry)}
                  >
                    还原
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </section>

      <Dialog
        open={conflict !== null}
        onOpenChange={(open) => (!open ? setConflict(null) : undefined)}
      >
        <DialogContent
          className="w-[min(980px,calc(100vw-2rem))]"
          data-testid="sync-conflict-dialog"
        >
          <DialogHeader>
            <DialogTitle>同步冲突：{conflict?.summary.path}</DialogTitle>
            <DialogDescription>
              本地、远端与共同祖先均已留档。冲突不会自动覆盖；选择后仍需下一次同步传播。
            </DialogDescription>
          </DialogHeader>
          {conflict ? (
            <div className="space-y-3">
              <div className="grid max-h-[42vh] gap-2 overflow-y-auto md:grid-cols-3">
                {[
                  ['本地版本', conflict.localContent],
                  ['共同祖先', conflict.baseContent],
                  ['远端版本', conflict.remoteContent],
                ].map(([label, content]) => (
                  <label key={String(label)} className="space-y-1">
                    <span className="text-xs font-medium text-[var(--ink)]">{label}</span>
                    <textarea
                      aria-label={String(label)}
                      readOnly
                      value={
                        typeof content === 'string'
                          ? content
                          : content === null
                            ? '（该版本不存在 / 已删除）'
                            : '（二进制附件，无文本预览）'
                      }
                      className="h-56 w-full resize-none rounded-md border border-[var(--border-subtle)] bg-[var(--surface-muted)] p-2 font-mono text-[10px] text-[var(--ink-muted)]"
                    />
                  </label>
                ))}
              </div>
              {conflict.summary.canMergeText ? (
                <div className="space-y-1">
                  <div className="flex items-center justify-between">
                    <Label htmlFor="sync-merged-content">合并结果</Label>
                    <Button size="sm" variant="secondary" onClick={() => void mergeConflict()}>
                      自动三方合并
                    </Button>
                  </div>
                  <textarea
                    id="sync-merged-content"
                    value={mergedContent}
                    onChange={(event) => setMergedContent(event.target.value)}
                    className="h-40 w-full resize-y rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] p-2 font-mono text-[11px]"
                  />
                  {mergeNote ? (
                    <p className="text-[10px] text-[var(--ink-subtle)]">{mergeNote}</p>
                  ) : null}
                </div>
              ) : null}
            </div>
          ) : null}
          <DialogFooter>
            <Button variant="ghost" onClick={() => setConflict(null)}>
              稍后处理
            </Button>
            <Button
              variant="secondary"
              disabled={busy !== null}
              onClick={() => void resolveConflict('remote')}
            >
              采用远端
            </Button>
            {conflict?.summary.canMergeText ? (
              <Button disabled={busy !== null} onClick={() => void resolveConflict('merged')}>
                使用合并结果
              </Button>
            ) : null}
            <Button disabled={busy !== null} onClick={() => void resolveConflict('local')}>
              保留本地
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
