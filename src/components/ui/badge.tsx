import * as React from 'react';

import { cn } from '@/lib/utils';

export function Badge({
  className,
  variant = 'default',
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & { variant?: 'default' | 'outline' | 'brand' }) {
  return (
    <span
      className={cn(
        'inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium',
        variant === 'default' && 'bg-[var(--surface-muted)] text-[var(--ink-muted)]',
        variant === 'outline' && 'border border-[var(--border-subtle)] text-[var(--ink-muted)]',
        variant === 'brand' && 'bg-[var(--brand-soft)] text-[var(--brand)]',
        className,
      )}
      {...props}
    />
  );
}

export function Separator({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('h-px w-full bg-[var(--border-subtle)]', className)} {...props} />;
}

export function Skeleton({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn('animate-pulse rounded-md bg-[var(--surface-muted)]', className)}
      {...props}
    />
  );
}
