import {
  Bold,
  Code,
  Heading1,
  Heading2,
  Heading3,
  Italic,
  IndentDecrease,
  IndentIncrease,
  Link2,
  List,
  ListOrdered,
  Minus,
  MoreHorizontal,
  Quote,
  SquareCheckBig,
  Strikethrough,
  Type,
} from 'lucide-react';
import { useEffect, useRef, useState, type ReactNode } from 'react';

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { cn } from '@/lib/utils';

/** 一个按钮占的横向步长：26px 按钮 + 2px 间距（算"放得下几个"用） */
const BUTTON_STEP = 28;

/**
 * 文本编辑工具条（2026-10-16 新增；2026-10-17 按用户要求改成"实测宽度"版）。
 *
 * 布局规则（都来自用户这一轮的说明）：
 * - 它并进编辑器工具栏**同一行**，**始终靠右**；
 * - 按钮**按优先级从左到右依次排列**，能放几个放几个；
 * - **放不下的才进「…」**（全都放得下时就不显示「…」）；
 * - 与左边的「保存」之间由调用方留出**硬间隔**（两倍保存按钮宽），不可逾越。
 *
 * 宽度用 `ResizeObserver` 实测容器宽度，按固定步长算出能放几个——
 * 比"写死哪些常显、哪些进菜单"更贴近"能铺开就铺开"。
 * （不在 effect 里同步 setState：observe 之后观察者自己会回调一次，用它拿初始宽度。）
 *
 * 命令本身在 `lib/markdownCommands.ts`（纯函数、可单测），这里只负责"长什么样、点了叫什么"。
 */
export function MarkdownFormatBar({
  disabled,
  onWrap,
  onLinePrefix,
  onHeading,
  onIndent,
}: {
  disabled?: boolean;
  onWrap: (before: string, after?: string, placeholder?: string) => void;
  onLinePrefix: (prefix: string) => void;
  onHeading: (level: 0 | 1 | 2 | 3) => void;
  onIndent: (delta: 1 | -1) => void;
}) {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const [available, setAvailable] = useState(0);

  useEffect(() => {
    const node = containerRef.current;
    if (!node || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => setAvailable(node.clientWidth));
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  /** 命令表：**顺序就是优先级**（常用的在前，放不下时后面的进「…」） */
  const commands: Array<{ label: string; icon: ReactNode; run: () => void }> = [
    { label: '加粗', icon: <Bold className="h-3.5 w-3.5" />, run: () => onWrap('**', '**', '加粗文字') },
    { label: '斜体', icon: <Italic className="h-3.5 w-3.5" />, run: () => onWrap('*', '*', '斜体文字') },
    {
      label: '删除线',
      icon: <Strikethrough className="h-3.5 w-3.5" />,
      run: () => onWrap('~~', '~~', '删除线'),
    },
    { label: '行内代码', icon: <Code className="h-3.5 w-3.5" />, run: () => onWrap('`', '`', '代码') },
    {
      label: '链接',
      icon: <Link2 className="h-3.5 w-3.5" />,
      run: () => onWrap('[', '](https://)', '链接文字'),
    },
    { label: '无序列表', icon: <List className="h-3.5 w-3.5" />, run: () => onLinePrefix('- ') },
    { label: '有序列表', icon: <ListOrdered className="h-3.5 w-3.5" />, run: () => onLinePrefix('1. ') },
    { label: '待办', icon: <SquareCheckBig className="h-3.5 w-3.5" />, run: () => onLinePrefix('- [ ] ') },
    { label: '引用', icon: <Quote className="h-3.5 w-3.5" />, run: () => onLinePrefix('> ') },
    { label: '标题 1', icon: <Heading1 className="h-3.5 w-3.5" />, run: () => onHeading(1) },
    { label: '标题 2', icon: <Heading2 className="h-3.5 w-3.5" />, run: () => onHeading(2) },
    { label: '标题 3', icon: <Heading3 className="h-3.5 w-3.5" />, run: () => onHeading(3) },
    { label: '正文', icon: <Type className="h-3.5 w-3.5" />, run: () => onHeading(0) },
    { label: '分割线', icon: <Minus className="h-3.5 w-3.5" />, run: () => onLinePrefix('---') },
    { label: '增加缩进', icon: <IndentIncrease className="h-3.5 w-3.5" />, run: () => onIndent(1) },
    { label: '减少缩进', icon: <IndentDecrease className="h-3.5 w-3.5" />, run: () => onIndent(-1) },
  ];

  const fits = Math.max(0, Math.floor(available / BUTTON_STEP));
  // 放不下时给「…」留一个位置
  const visibleCount = fits >= commands.length ? commands.length : Math.max(0, fits - 1);
  const hidden = commands.slice(visibleCount);

  const button = (command: { label: string; icon: ReactNode; run: () => void }) => (
    <button
      key={command.label}
      type="button"
      title={command.label}
      aria-label={command.label}
      disabled={disabled}
      onClick={command.run}
      className={cn(
        'flex h-6.5 w-6.5 shrink-0 items-center justify-center rounded text-[var(--ink-subtle)] transition-colors',
        'hover:bg-[var(--surface-muted)] hover:text-[var(--ink)] disabled:cursor-not-allowed disabled:opacity-40',
      )}
    >
      {command.icon}
    </button>
  );

  return (
    <div
      ref={containerRef}
      data-testid="format-bar"
      className="flex min-w-0 flex-1 items-center justify-end gap-0.5"
    >
      {commands.slice(0, visibleCount).map(button)}
      {hidden.length > 0 ? (
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              data-testid="format-more"
              disabled={disabled}
              title="更多格式"
              aria-label="更多格式"
              className="flex h-6.5 w-6.5 shrink-0 items-center justify-center rounded text-[var(--ink-subtle)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)] disabled:cursor-not-allowed disabled:opacity-40"
            >
              <MoreHorizontal className="h-3.5 w-3.5" />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-56">
            <div className="flex flex-wrap items-center gap-0.5 p-1">
              {hidden.map((command) => (
                <button
                  key={command.label}
                  type="button"
                  title={command.label}
                  aria-label={command.label}
                  disabled={disabled}
                  onClick={command.run}
                  className="flex h-7 w-7 items-center justify-center rounded text-[var(--ink-muted)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)] disabled:opacity-40"
                >
                  {command.icon}
                </button>
              ))}
            </div>
          </DropdownMenuContent>
        </DropdownMenu>
      ) : null}
    </div>
  );
}
