import { ChevronsLeft, ChevronsRight, ListTree, PanelRight, Pin } from 'lucide-react';

import { cn } from '@/lib/utils';
import type { OutlineHeading } from '@/lib/markdown';

interface OutlinePanelProps {
  headings: OutlineHeading[];
  onJump: (offset: number) => void;
  /** 当前光标所在标题的偏移（用于高亮） */
  activeOffset?: number | null;
  /** 是否已钉住（固定在右侧）；false 表示悬浮预览，提供「钉住大纲」按钮 */
  pinned?: boolean;
  /** 收起为贴边 mini 态（对标语雀的「隐藏大纲」） */
  onCollapse?: () => void;
  /** 钉住（对标语雀的「钉住大纲」） */
  onPin?: () => void;
  /** 笔记标题：在列表栏里显示大纲时，让用户知道"这是哪一篇的大纲" */
  title?: string;
  /** 返回笔记列表（大纲与列表共用一栏时的「〈〈」） */
  onBack?: () => void;
  /** 把大纲挪回「笔记列表那一栏」（右侧列里显示时提供） */
  onMoveToList?: () => void;
  /** 把大纲钉到正文右侧（在列表栏里显示时提供） */
  onMoveToSide?: () => void;
  className?: string;
}

/** 编辑器大纲（V1.1 M2-1）：解析 `#` 层级，点击跳转到对应位置。 */
export function OutlinePanel({
  headings,
  onJump,
  activeOffset,
  pinned = true,
  onCollapse,
  onPin,
  title,
  onBack,
  onMoveToList,
  onMoveToSide,
  className,
}: OutlinePanelProps) {
  return (
    <aside
      data-testid="outline-panel"
      className={cn(
        'flex w-[190px] shrink-0 flex-col border-l border-[var(--border-subtle)] bg-[var(--surface)]',
        className,
      )}
    >
      <header className="flex items-center gap-1.5 px-3 py-2 text-xs font-medium text-[var(--ink)]">
        {/* 大纲与列表共用一栏时：一个明确的「〈〈 返回列表」（点工具栏按钮也能回来） */}
        {onBack ? (
          <button
            type="button"
            data-testid="outline-back"
            title="返回笔记列表"
            aria-label="返回笔记列表"
            onClick={onBack}
            className="flex shrink-0 items-center gap-0.5 rounded px-0.5 py-0.5 text-[var(--ink-subtle)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]"
          >
            <ChevronsLeft className="h-3.5 w-3.5" />
          </button>
        ) : null}
        {/*
          栏头**永远是一行**（2026-10-16 修正）：标题长的时候以前会把「大纲」挤成竖排两行
          （「大 / 纲」）、图标也被压扁——因为这一行里除了标题，其它都没有"不许收缩"的约束。
          现在：图标与文字 `shrink-0`（谁说也不能缩）、标题吃剩余宽度并截断、
          右侧计数与按钮 `shrink-0`。宽度不够时**只截标题**，不改栏头的样子。
        */}
        <ListTree className="h-3.5 w-3.5 shrink-0 text-[var(--brand)]" />
        <span data-testid="outline-label" className="shrink-0 whitespace-nowrap">
          大纲
        </span>
        {title ? (
          <span
            data-testid="outline-note-title"
            className="min-w-0 flex-1 truncate font-normal text-[var(--ink-subtle)]"
            title={title}
          >
            · {title}
          </span>
        ) : null}
        <span className="ml-auto shrink-0 text-[11px] text-[var(--ink-subtle)]">
          {headings.length}
        </span>
        {/* 对标语雀：隐藏大纲 → 贴边 mini 态；mini 态可「钉住大纲」固定 */}
        {/* 换位置：列表栏 ⇄ 正文右侧（两个方向各有一个入口，避免"挪了就找不回来"） */}
        {onMoveToSide ? (
          <button
            type="button"
            data-testid="outline-move-to-side"
            title="钉在正文右侧"
            aria-label="钉在正文右侧"
            onClick={onMoveToSide}
            className="shrink-0 rounded p-0.5 text-[var(--ink-subtle)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]"
          >
            <PanelRight className="h-3.5 w-3.5" />
          </button>
        ) : null}
        {onMoveToList ? (
          <button
            type="button"
            data-testid="outline-move-to-list"
            title="改到笔记列表那一栏（省横向空间）"
            aria-label="改到笔记列表那一栏"
            onClick={onMoveToList}
            className="shrink-0 rounded p-0.5 text-[var(--ink-subtle)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]"
          >
            <ChevronsLeft className="h-3.5 w-3.5" />
          </button>
        ) : null}
        {pinned
          ? onCollapse && (
              <button
                type="button"
                data-testid="outline-collapse"
                title="收起大纲（贴边，鼠标移上去可临时展开）"
                aria-label="收起大纲"
                onClick={onCollapse}
                className="shrink-0 rounded p-0.5 text-[var(--ink-subtle)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]"
              >
                <ChevronsRight className="h-3.5 w-3.5" />
              </button>
            )
          : onPin && (
              <button
                type="button"
                data-testid="outline-pin"
                title="钉住大纲（固定在右侧）"
                aria-label="钉住大纲"
                onClick={onPin}
                className="shrink-0 rounded p-0.5 text-[var(--ink-subtle)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]"
              >
                <Pin className="h-3.5 w-3.5" />
              </button>
            )}
      </header>
      <div className="min-h-0 flex-1 overflow-auto px-1.5 pb-3">
        {headings.length === 0 ? (
          <p className="px-1.5 text-[11px] leading-relaxed text-[var(--ink-subtle)]">
            正文还没有标题。用 <code># 标题</code> 开始分层，这里会自动生成大纲。
          </p>
        ) : (
          <ul className="space-y-0.5">
            {headings.map((heading, index) => {
              const active = activeOffset === heading.offset;
              return (
                <li key={`${heading.offset}-${index}`}>
                  <button
                    type="button"
                    data-testid="outline-item"
                    title={heading.text}
                    onClick={() => onJump(heading.offset)}
                    className={cn(
                      'block w-full truncate rounded px-1.5 py-1 text-left text-xs transition-colors',
                      active
                        ? 'bg-[var(--brand-soft)] text-[var(--brand)]'
                        : 'text-[var(--ink-muted)] hover:bg-[var(--surface-muted)] hover:text-[var(--ink)]',
                    )}
                    // 按层级缩进：H1 无缩进，H6 缩进 25px
                    style={{ paddingLeft: `${(heading.level - 1) * 10 + 6}px` }}
                  >
                    {heading.text}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </aside>
  );
}
