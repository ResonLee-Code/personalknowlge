import { Copy, TriangleAlert } from 'lucide-react';
import * as React from 'react';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';

/**
 * 失败原因的「可点详情」（Track F2）。
 *
 * 为什么需要它：B3（Track B3）已经让失败原因**可见**（不再只挂在悬停提示里）并写进应用日志，
 * 但两件事仍然挡着用户看清原因：
 * - 头部/按钮旁的空间有限，长原因被**截断**，剩下的部分没处看；
 * - toast 提示**会自己消失**，想回头看时已经没了。
 *
 * 这里把"看得见"升级成"点得开"：一行摘要 + 「详情」按钮，点开是**完整原文**，可复制。
 * 统一成一个组件是刻意的——否则每个失败路径各写一套，又会漂移成"有的能点、有的不能"。
 */
export interface FailureDetailProps {
  /** 一行摘要（例如「保存失败」）；为空时只显示详情入口 */
  summary?: string;
  /** 完整原因（后端 `message`；可能很长，不做截断） */
  detail?: string | null;
  /** 错误码（有就显示；报障时对着日志找） */
  code?: string | null;
  /** 额外证据（文件路径 / 哈希 / 时间…），按行显示 */
  extra?: Array<string | null | undefined> | null;
  /** 摘要是否截断（头部空间小时用） */
  truncate?: boolean;
  className?: string;
  testId?: string;
}

/** 要复制到剪贴板的文本：摘要、全文、错误码与补充信息都在里面，方便直接贴进报障。 */
export function failureDetailText({
  summary,
  detail,
  code,
  extra,
}: Pick<FailureDetailProps, 'summary' | 'detail' | 'code' | 'extra'>): string {
  const lines: string[] = [];
  const head = [summary?.trim(), code?.trim() ? `[${code.trim()}]` : null]
    .filter(Boolean)
    .join(' ');
  if (head) lines.push(head);
  if (detail?.trim()) lines.push(detail.trim());
  (extra ?? []).forEach((item) => {
    if (item?.trim()) lines.push(item.trim());
  });
  return lines.join('\n');
}

export function FailureDetail({
  summary,
  detail,
  code,
  extra,
  truncate = false,
  className,
  testId = 'failure-detail',
}: FailureDetailProps) {
  const [open, setOpen] = React.useState(false);
  const [copied, setCopied] = React.useState(false);
  const full = detail?.trim() ?? '';
  const extras = (extra ?? []).filter((item): item is string => Boolean(item?.trim()));
  const hasMore = Boolean(full) || Boolean(code?.trim()) || extras.length > 0;
  const text = failureDetailText({ summary, detail, code, extra: extras });

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // 剪贴板不可用（无权限 / 非安全上下文）不该变成新的失败源：保持原样，用户还能手动选中
      setCopied(false);
    }
  };

  return (
    <span className={['inline-flex min-w-0 items-center gap-1', className ?? ''].join(' ')}>
      {summary ? (
        <span className={truncate ? 'min-w-0 truncate' : undefined}>{summary}</span>
      ) : null}
      {hasMore ? (
        <>
          <button
            type="button"
            data-testid={`${testId}-open`}
            onClick={() => setOpen(true)}
            className="shrink-0 rounded px-1 text-[11px] font-medium underline decoration-dotted underline-offset-2 hover:text-[var(--ink)]"
          >
            详情
          </button>
          <Dialog open={open} onOpenChange={setOpen}>
            <DialogContent data-testid={`${testId}-dialog`}>
              <DialogHeader>
                <DialogTitle className="flex items-center gap-2 text-sm">
                  <TriangleAlert className="h-4 w-4 text-red-500" />
                  {summary?.trim() || '失败详情'}
                </DialogTitle>
                <DialogDescription className="text-xs">
                  同一条原因也写进了**应用日志**（与后端 `…/logs/app.log` 是同一份），报障时可按时间对上。
                </DialogDescription>
              </DialogHeader>
              {code?.trim() ? (
                <p className="text-xs text-[var(--ink-subtle)]" data-testid={`${testId}-code`}>
                  错误码：<code>{code.trim()}</code>
                </p>
              ) : null}
              {full ? (
                <pre
                  data-testid={`${testId}-full`}
                  className="mt-2 max-h-72 overflow-auto whitespace-pre-wrap break-words rounded-lg border border-[var(--border-subtle)] bg-[var(--surface-muted)] p-3 text-xs"
                >
                  {full}
                </pre>
              ) : null}
              {extras.length > 0 ? (
                <ul
                  data-testid={`${testId}-extra`}
                  className="mt-2 flex flex-col gap-0.5 text-xs text-[var(--ink-subtle)]"
                >
                  {extras.map((item, index) => (
                    <li key={`${index}-${item}`}>{item}</li>
                  ))}
                </ul>
              ) : null}
              <DialogFooter>
                <Button type="button" variant="outline" size="sm" onClick={copy}>
                  <Copy className="mr-1 h-3.5 w-3.5" />
                  {copied ? '已复制' : '复制详情'}
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </>
      ) : null}
    </span>
  );
}
