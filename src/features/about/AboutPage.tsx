import {
  BookOpen,
  Database,
  Download,
  FileCode2,
  Loader2,
  RefreshCw,
  Shield,
  Sparkles,
} from 'lucide-react';
import { useEffect, useState } from 'react';

import { Button } from '@/components/ui/button';
import { EVENTS, SHORTCUTS } from '@/lib/constants';
import { errorMessage } from '@/lib/format';
import { appService, pickSavePath } from '@/services/appService';
import { subscribe } from '@/services/api';
import type {
  AppInfo,
  AppUpdateInfo,
  AppUpdateInstallResult,
  AppUpdateProgress,
} from '@/types';

const STACK = [
  { label: '桌面外壳', value: 'Tauri 2 + Rust' },
  { label: '前端', value: 'React 19 + TypeScript + Vite' },
  { label: '界面', value: 'Tailwind CSS 4 + shadcn/ui 风格组件' },
  { label: '编辑器', value: 'CodeMirror 6' },
  { label: '数据', value: 'Markdown 文件 + SQLite（WAL）' },
  { label: '搜索', value: 'SQLite FTS5 + 中文双字短语索引' },
];

/** 关于页面（设计文档 16.2 /about）。 */
export function AboutPage() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [logPath, setLogPath] = useState<string | null>(null);
  /** 读取失败的原因：不能只把版本/平台显示成「—」——用户来这页就是为了看这些 */
  const [infoError, setInfoError] = useState<string | null>(null);
  const [updateInfo, setUpdateInfo] = useState<AppUpdateInfo | null>(null);
  const [updateResult, setUpdateResult] =
    useState<AppUpdateInstallResult | null>(null);
  const [updateProgress, setUpdateProgress] = useState<AppUpdateProgress | null>(null);
  const [updating, setUpdating] = useState<'check' | 'install' | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const [diagnosticMessage, setDiagnosticMessage] = useState<string | null>(null);

  useEffect(() => {
    void appService
      .info()
      .then((value) => {
        setInfo(value);
        setInfoError(null);
      })
      .catch((error: unknown) => {
        setInfo(null);
        setInfoError(errorMessage(error));
      });
    void appService.logPath().then(setLogPath).catch(() => setLogPath(null));
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void subscribe<AppUpdateProgress>(EVENTS.updateProgress, (progress) => {
      if (!cancelled) setUpdateProgress(progress);
    })
      .then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const checkUpdate = async () => {
    setUpdating('check');
    setUpdateError(null);
    setUpdateResult(null);
    try {
      setUpdateInfo(await appService.checkUpdate());
    } catch (caught) {
      setUpdateError(errorMessage(caught));
    } finally {
      setUpdating(null);
    }
  };

  const installUpdate = async () => {
    setUpdating('install');
    setUpdateError(null);
    setUpdateProgress(null);
    try {
      setUpdateResult(await appService.installUpdate());
    } catch (caught) {
      setUpdateError(errorMessage(caught));
    } finally {
      setUpdating(null);
    }
  };

  const exportDiagnostics = async () => {
    setDiagnosticMessage(null);
    try {
      const target = await pickSavePath(
        `pkos-diagnostics-${new Date().toISOString().slice(0, 10)}.txt`,
        ['txt', 'log'],
        '诊断日志',
      );
      if (!target) return;
      const saved = await appService.exportDiagnostics(target);
      setDiagnosticMessage(`已导出诊断日志：${saved}`);
    } catch (caught) {
      setDiagnosticMessage(`导出失败：${errorMessage(caught)}`);
    }
  };

  return (
    <div className="h-full overflow-auto p-6">
      <div className="mx-auto max-w-3xl space-y-6">
        <header className="space-y-2">
          <div className="flex items-center gap-2 text-[var(--brand)]">
            <Sparkles className="h-4 w-4" />
            <span className="text-xs font-semibold uppercase tracking-wide">
              V2.0 · DEVELOPMENT
            </span>
          </div>
          <h1 className="text-lg font-semibold text-[var(--ink)]">Personal Knowledge OS</h1>
          <p className="text-sm leading-relaxed text-[var(--ink-muted)]">
            第一阶段的成功标准不是功能数量，而是「用户愿意每天用它记录知识，
            并且在需要时能快速找回」。
          </p>
        </header>

        <section className="grid gap-3 sm:grid-cols-2">
          {STACK.map((item) => (
            <div
              key={item.label}
              className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-3"
            >
              <p className="text-[11px] uppercase tracking-wide text-[var(--ink-subtle)]">
                {item.label}
              </p>
              <p className="mt-0.5 text-sm text-[var(--ink)]">{item.value}</p>
            </div>
          ))}
        </section>

        <section className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4">
          <div className="flex flex-wrap items-center gap-2">
            <h2 className="flex items-center gap-2 text-sm font-semibold text-[var(--ink)]">
              <Download className="h-4 w-4" />
              应用更新
            </h2>
            <Button
              variant="outline"
              size="sm"
              className="ml-auto"
              disabled={updating !== null}
              onClick={() => void checkUpdate()}
            >
              {updating === 'check' ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <RefreshCw className="h-3.5 w-3.5" />
              )}
              检查更新
            </Button>
          </div>
          <p className="mt-2 text-xs text-[var(--ink-muted)]">
            更新包必须通过内置公钥签名校验；校验失败不会安装，也不会替换当前版本。
          </p>
          {updateInfo ? (
            <div className="mt-3 rounded-lg border border-[var(--border-subtle)] bg-[var(--surface-muted)] p-3">
              <p className="text-sm font-medium text-[var(--ink)]">
                发现新版本 {updateInfo.version}
              </p>
              <p className="mt-1 text-xs text-[var(--ink-subtle)]">
                当前 {updateInfo.currentVersion} · {updateInfo.target}
                {updateInfo.date ? ` · ${updateInfo.date}` : ''}
              </p>
              {updateInfo.body ? (
                <p className="mt-2 whitespace-pre-wrap text-xs text-[var(--ink-muted)]">
                  {updateInfo.body}
                </p>
              ) : null}
              <Button
                className="mt-3"
                size="sm"
                disabled={updating !== null}
                onClick={() => void installUpdate()}
              >
                {updating === 'install' ? (
                  <Loader2 className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Download className="h-3.5 w-3.5" />
                )}
                下载并安装
              </Button>
            </div>
          ) : null}
          {updateProgress ? (
            <p className="mt-2 text-xs text-[var(--ink-muted)]">
              {updateProgress.stage === 'verifying'
                ? '下载完成，正在验证签名…'
                : `正在下载：${Math.round(updateProgress.downloaded / 1024)} KB`}
            </p>
          ) : null}
          {updateResult ? (
            <div className="mt-3 flex flex-wrap items-center gap-2 rounded-md bg-emerald-500/10 p-2 text-xs text-emerald-700 dark:text-emerald-300">
              <span>{updateResult.message}</span>
              <Button
                size="sm"
                onClick={() => {
                  void appService.restart();
                }}
              >
                立即重启
              </Button>
            </div>
          ) : null}
          {updateError ? (
            <p className="mt-2 text-xs text-rose-600 dark:text-rose-300">{updateError}</p>
          ) : null}
        </section>

        <section className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4">
          <h2 className="mb-2 flex items-center gap-2 text-sm font-semibold text-[var(--ink)]">
            <Database className="h-4 w-4" />
            运行信息
          </h2>
          {infoError ? (
            <p data-testid="about-info-error" className="mb-2 text-xs text-amber-600">
              读取运行信息失败：{infoError}
            </p>
          ) : null}
          <dl className="grid gap-1.5 text-xs text-[var(--ink-muted)] sm:grid-cols-2">
            <div className="flex justify-between gap-2">
              <dt>应用版本</dt>
              <dd>{info?.version ?? '—'}</dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt>数据库版本</dt>
              <dd>{info?.schemaVersion ?? '—'}</dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt>SQLite</dt>
              <dd>{info?.sqliteVersion ?? '—'}</dd>
            </div>
            <div className="flex justify-between gap-2">
              <dt>平台</dt>
              <dd>{info?.platform ?? '—'}</dd>
            </div>
            <div className="col-span-full flex justify-between gap-2">
              <dt>数据目录</dt>
              <dd className="truncate">{info?.dataDir ?? '—'}</dd>
            </div>
            <div className="col-span-full flex justify-between gap-2">
              <dt>日志文件</dt>
              <dd className="truncate">{logPath ?? '—'}</dd>
            </div>
          </dl>
          <div className="mt-3 flex flex-wrap items-center gap-2">
            <Button variant="outline" size="sm" onClick={() => void exportDiagnostics()}>
              <Download className="h-3.5 w-3.5" />
              导出诊断日志
            </Button>
            <span className="text-[11px] text-[var(--ink-subtle)]">
              只包含运行元数据与应用日志，不读取笔记正文。
            </span>
          </div>
          {diagnosticMessage ? (
            <p className="mt-2 break-all text-xs text-[var(--ink-muted)]">
              {diagnosticMessage}
            </p>
          ) : null}
        </section>

        <section className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4">
          <h2 className="mb-2 flex items-center gap-2 text-sm font-semibold text-[var(--ink)]">
            <FileCode2 className="h-4 w-4" />
            快捷键
          </h2>
          <ul className="grid gap-1.5 text-xs text-[var(--ink-muted)] sm:grid-cols-2">
            {SHORTCUTS.map((shortcut) => (
              <li key={shortcut.keys} className="flex items-center justify-between gap-2">
                <span>{shortcut.action}</span>
                <kbd className="rounded border border-[var(--border-subtle)] px-1.5 py-0.5 text-[10px]">
                  {shortcut.keys}
                </kbd>
              </li>
            ))}
          </ul>
        </section>

        <section className="grid gap-3 sm:grid-cols-2">
          <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4">
            <h2 className="mb-1.5 flex items-center gap-2 text-sm font-semibold text-[var(--ink)]">
              <Shield className="h-4 w-4" />
              数据安全
            </h2>
            <ul className="space-y-1 text-xs text-[var(--ink-muted)]">
              <li>· 内容默认不上传，仅保存在本地目录</li>
              <li>· 原子写入，保存失败不会覆盖原文件</li>
              <li>· 删除先进回收站，可恢复</li>
              <li>· 备份为可独立验证的 ZIP 包</li>
            </ul>
          </div>
          <div className="rounded-xl border border-[var(--border-subtle)] bg-[var(--surface)] p-4">
            <h2 className="mb-1.5 flex items-center gap-2 text-sm font-semibold text-[var(--ink)]">
              <BookOpen className="h-4 w-4" />
              设计文档
            </h2>
            <ul className="space-y-1 text-xs text-[var(--ink-muted)]">
              <li>· docs/ARCHITECTURE.md</li>
              <li>· docs/DATABASE_SCHEMA.md</li>
              <li>· docs/API_CONTRACT.md</li>
              <li>· docs/DEVELOPMENT_RULES.md</li>
              <li>· Personal-Knowledge-OS-Engineering-Design-V1.0.md</li>
            </ul>
          </div>
        </section>
      </div>
    </div>
  );
}
