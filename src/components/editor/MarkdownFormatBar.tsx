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
  Quote,
  SquareCheckBig,
  Strikethrough,
  Type,
} from 'lucide-react';

import { cn } from '@/lib/utils';

/**
 * 文本编辑工具条（2026-10-16 新增，对标语雀编辑器的格式栏）。
 *
 * 这里只负责"长什么样、点了叫什么命令"——**命令本身在 `lib/markdownCommands.ts`**
 * （纯函数、可单测），落进编辑器由 `NoteEditorPage` 通过 CodeMirror 的 view 完成。
 *
 * 全是标准 Markdown，不引入私有格式：选中文字点一下即加/去标记；没选中时插入一对标记，
 * 光标停在中间可以直接打字（再点一次取消）。
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
  const button = (
    label: string,
    icon: React.ReactNode,
    onClick: () => void,
    extra?: string,
  ) => (
    <button
      key={label}
      type="button"
      title={label}
      aria-label={label}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        'flex h-6.5 w-6.5 items-center justify-center rounded text-[var(--ink-subtle)] transition-colors',
        'hover:bg-[var(--surface-muted)] hover:text-[var(--ink)] disabled:cursor-not-allowed disabled:opacity-40',
        extra,
      )}
    >
      {icon}
    </button>
  );

  return (
    <div
      data-testid="format-bar"
      className="flex flex-wrap items-center gap-0.5 border-b border-[var(--border-subtle)] px-3 py-1"
    >
      {/* 段落级：标题 / 列表 / 引用（整行生效） */}
      {button('正文', <Type className="h-3.5 w-3.5" />, () => onHeading(0))}
      {button('标题 1', <Heading1 className="h-3.5 w-3.5" />, () => onHeading(1))}
      {button('标题 2', <Heading2 className="h-3.5 w-3.5" />, () => onHeading(2))}
      {button('标题 3', <Heading3 className="h-3.5 w-3.5" />, () => onHeading(3))}
      <span className="mx-1 h-4 w-px bg-[var(--border-subtle)]" />
      {button('无序列表', <List className="h-3.5 w-3.5" />, () => onLinePrefix('- '))}
      {button('有序列表', <ListOrdered className="h-3.5 w-3.5" />, () => onLinePrefix('1. '))}
      {button('待办', <SquareCheckBig className="h-3.5 w-3.5" />, () => onLinePrefix('- [ ] '))}
      {button('引用', <Quote className="h-3.5 w-3.5" />, () => onLinePrefix('> '))}
      <span className="mx-1 h-4 w-px bg-[var(--border-subtle)]" />
      {/* 行内：加粗 / 斜体 / 删除线 / 行内代码 / 链接 */}
      {button('加粗', <Bold className="h-3.5 w-3.5" />, () => onWrap('**', '**', '加粗文字'))}
      {button('斜体', <Italic className="h-3.5 w-3.5" />, () => onWrap('*', '*', '斜体文字'))}
      {button('删除线', <Strikethrough className="h-3.5 w-3.5" />, () =>
        onWrap('~~', '~~', '删除线'),
      )}
      {button('行内代码', <Code className="h-3.5 w-3.5" />, () => onWrap('`', '`', '代码'))}
      {button('链接', <Link2 className="h-3.5 w-3.5" />, () => onWrap('[', '](https://)', '链接文字'))}
      <span className="mx-1 h-4 w-px bg-[var(--border-subtle)]" />
      {/* 块级：分割线 / 缩进 */}
      {button('分割线', <Minus className="h-3.5 w-3.5" />, () => onLinePrefix('---'))}
      {button('减少缩进', <IndentDecrease className="h-3.5 w-3.5" />, () => onIndent(-1))}
      {button('增加缩进', <IndentIncrease className="h-3.5 w-3.5" />, () => onIndent(1))}
    </div>
  );
}
