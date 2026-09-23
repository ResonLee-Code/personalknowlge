import { BookOpen, Database, FileCode2, Shield, Sparkles } from 'lucide-react';
import { useEffect, useState } from 'react';

import { SHORTCUTS } from '@/lib/constants';
import { appService } from '@/services/appService';
import type { AppInfo } from '@/types';

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

  useEffect(() => {
    void appService.info().then(setInfo).catch(() => setInfo(null));
    void appService.logPath().then(setLogPath).catch(() => setLogPath(null));
  }, []);

  return (
    <div className="h-full overflow-auto p-6">
      <div className="mx-auto max-w-3xl space-y-6">
        <header className="space-y-2">
          <div className="flex items-center gap-2 text-[var(--brand)]">
            <Sparkles className="h-4 w-4" />
            <span className="text-xs font-semibold uppercase tracking-wide">V1.0</span>
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
          <h2 className="mb-2 flex items-center gap-2 text-sm font-semibold text-[var(--ink)]">
            <Database className="h-4 w-4" />
            运行信息
          </h2>
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
