import type { ErrorInfo, ReactNode } from 'react';
import { Component } from 'react';

import { logAppFailure } from '@/lib/appLog';
import { errorMessage } from '@/lib/format';

interface AppErrorBoundaryProps {
  children: ReactNode;
}

interface AppErrorBoundaryState {
  message: string | null;
}

/**
 * React 的未捕获渲染错误会让生产包只剩空白窗口。
 * 这里保留可读原因和重载入口，并把同一条原因写入应用日志。
 */
export class AppErrorBoundary extends Component<AppErrorBoundaryProps, AppErrorBoundaryState> {
  state: AppErrorBoundaryState = { message: null };

  static getDerivedStateFromError(error: unknown): AppErrorBoundaryState {
    return { message: errorMessage(error) };
  }

  componentDidCatch(error: unknown, _info: ErrorInfo): void {
    void logAppFailure('render', error);
  }

  render() {
    if (this.state.message) {
      return (
        <main
          role="alert"
          className="flex h-full items-center justify-center bg-[var(--surface-muted)] p-8 text-[var(--ink)]"
        >
          <div className="w-full max-w-xl rounded-panel border border-[var(--border-subtle)] bg-[var(--surface-raised)] p-6 shadow-sm">
            <h1 className="text-lg font-semibold">界面加载失败</h1>
            <p className="mt-2 text-sm text-[var(--ink-muted)]">
              应用数据没有受到影响。请重新加载界面；如果问题持续出现，可导出诊断日志。
            </p>
            <pre className="mt-4 whitespace-pre-wrap break-words rounded-md bg-[var(--surface-muted)] p-3 text-xs text-[var(--ink-muted)]">
              {this.state.message}
            </pre>
            <button
              type="button"
              className="mt-5 rounded-md bg-[var(--brand)] px-4 py-2 text-sm font-medium text-white"
              onClick={() => window.location.reload()}
            >
              重新加载界面
            </button>
          </div>
        </main>
      );
    }

    return this.props.children;
  }
}
