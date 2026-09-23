import type { LucideIcon } from 'lucide-react';
import * as React from 'react';

import { cn } from '@/lib/utils';

interface EmptyStateProps {
  icon?: LucideIcon;
  title: string;
  description?: string;
  action?: React.ReactNode;
  className?: string;
}

/** 统一空状态（设计文档 17.3）。 */
export function EmptyState({ icon: Icon, title, description, action, className }: EmptyStateProps) {
  return (
    <div
      className={cn(
        'flex h-full flex-col items-center justify-center gap-3 px-6 py-10 text-center',
        className,
      )}
    >
      {Icon ? (
        <span className="flex h-11 w-11 items-center justify-center rounded-full bg-[var(--surface-muted)] text-[var(--ink-subtle)]">
          <Icon className="h-5 w-5" />
        </span>
      ) : null}
      <div className="space-y-1">
        <p className="text-sm font-medium text-[var(--ink)]">{title}</p>
        {description ? (
          <p className="max-w-sm text-xs leading-relaxed text-[var(--ink-subtle)]">{description}</p>
        ) : null}
      </div>
      {action}
    </div>
  );
}
