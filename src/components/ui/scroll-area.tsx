import * as ScrollAreaPrimitive from '@radix-ui/react-scroll-area';
import * as React from 'react';

import { cn } from '@/lib/utils';

export const ScrollArea = React.forwardRef<
  React.ComponentRef<typeof ScrollAreaPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof ScrollAreaPrimitive.Root>
>(({ className, children, ...props }, ref) => (
  <ScrollAreaPrimitive.Root ref={ref} className={cn('relative overflow-hidden', className)} {...props}>
    {/*
      `[&>div]:!block` 覆盖的是 Radix 自己塞进 Viewport 的那层内容 div
      （内联样式 `display: table; min-width: 100%`，所以必须用 `!` 才盖得住）。

      为什么必须盖（2026-10-15 实测）：table 盒子会**按内容撑宽**，于是列表行的实际宽度
      可以远超面板（实测面板 320px 时行有 520px）：标题再长也不会截断，行右侧那两个快捷按钮
      直接被面板裁掉——屏幕上表现为"标题被面板边缘切掉，右边什么都没有"。
      这里两处用它（笔记列表、侧边栏）都只做竖向滚动，不需要 table 那套横向撑宽。
    */}
    <ScrollAreaPrimitive.Viewport className="h-full w-full rounded-[inherit] [&>div]:!block">
      {children}
    </ScrollAreaPrimitive.Viewport>
    <ScrollAreaPrimitive.Scrollbar
      orientation="vertical"
      className="flex w-2 touch-none select-none p-[1px]"
    >
      <ScrollAreaPrimitive.Thumb className="relative flex-1 rounded-full bg-[var(--ink-subtle)]/40" />
    </ScrollAreaPrimitive.Scrollbar>
  </ScrollAreaPrimitive.Root>
));
ScrollArea.displayName = 'ScrollArea';
