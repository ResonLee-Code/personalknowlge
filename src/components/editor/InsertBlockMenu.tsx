import {
  CalendarDays,
  Code2,
  GitBranch,
  Image as ImageIcon,
  ListTodo,
  Minus,
  Paperclip,
  Quote,
  Sigma,
  Table2,
  Workflow,
} from 'lucide-react';

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { INSERT_BLOCKS } from '@/lib/markdownCommands';

/**
 * 「插入」菜单（2026-10-16 新增，对标语雀工具栏最左侧那个绿色 + ）。
 *
 * 放在编辑器工具栏**最左边**（编辑 / 分屏 / 预览 之前），只在编辑 / 分屏模式下可用
 * ——纯预览时没有光标可插。
 *
 * 里面每一项插的都是**普通 Markdown**（没有私有格式）：表格、代码块、公式、
 * Mermaid 图形（流程图 / 时序图 / 类图 / 思维导图）、待办、引用、分割线、日期。
 * 图片与附件走应用自己的附件流程（落 `attachments/`，正文插相对路径）。
 */
export function InsertBlockMenu({
  disabled,
  onInsert,
  onAttach,
}: {
  disabled?: boolean;
  /** 在光标处插入一段块级 Markdown */
  onInsert: (block: string) => void;
  /** 选文件后由上层落盘并插入链接（kind 决定是图片还是普通附件） */
  onAttach: (kind: 'image' | 'file') => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          data-testid="insert-menu"
          disabled={disabled}
          title={disabled ? '插入（切到编辑或分屏模式可用）' : '插入内容'}
          aria-label="插入内容"
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-[var(--brand-soft)] text-[var(--brand)] transition-colors hover:bg-[var(--brand)] hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
        >
          <span className="text-base leading-none">＋</span>
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-64">
        <DropdownMenuLabel>基础</DropdownMenuLabel>
        <DropdownMenuItem onSelect={() => onAttach('image')}>
          <ImageIcon className="h-3.5 w-3.5" /> 图片…
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onInsert(INSERT_BLOCKS.table)}>
          <Table2 className="h-3.5 w-3.5" /> 表格
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onAttach('file')}>
          <Paperclip className="h-3.5 w-3.5" /> 附件…
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onInsert(INSERT_BLOCKS.todo)}>
          <ListTodo className="h-3.5 w-3.5" /> 待办清单
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuLabel>程序员专区</DropdownMenuLabel>
        <DropdownMenuItem onSelect={() => onInsert(INSERT_BLOCKS.code)}>
          <Code2 className="h-3.5 w-3.5" /> 代码块
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onInsert(INSERT_BLOCKS.math)}>
          <Sigma className="h-3.5 w-3.5" /> 公式（LaTeX）
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onInsert(INSERT_BLOCKS.flowchart)}>
          <Workflow className="h-3.5 w-3.5" /> 流程图
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onInsert(INSERT_BLOCKS.sequence)}>
          <GitBranch className="h-3.5 w-3.5" /> 时序图
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onInsert(INSERT_BLOCKS.classDiagram)}>
          <GitBranch className="h-3.5 w-3.5" /> 类图 / UML
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onInsert(INSERT_BLOCKS.mindmap)}>
          <GitBranch className="h-3.5 w-3.5" /> 思维导图
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuLabel>其它</DropdownMenuLabel>
        <DropdownMenuItem onSelect={() => onInsert(INSERT_BLOCKS.quote)}>
          <Quote className="h-3.5 w-3.5" /> 引用块
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onInsert(INSERT_BLOCKS.divider)}>
          <Minus className="h-3.5 w-3.5" /> 分割线
        </DropdownMenuItem>
        <DropdownMenuItem
          onSelect={() => {
            const now = new Date();
            const pad = (value: number) => String(value).padStart(2, '0');
            onInsert(`${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`);
          }}
        >
          <CalendarDays className="h-3.5 w-3.5" /> 今天的日期
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
