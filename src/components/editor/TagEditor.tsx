import { Plus, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import { useTagsStore } from '@/stores/tagsStore';

/** 笔记标签编辑（设计文档 5.4 FR-T03）。 */
export function TagEditor({
  tags,
  onChange,
  placeholder = '输入标签后回车',
  inline = false,
}: {
  tags: string[];
  onChange: (tags: string[]) => void;
  placeholder?: string;
  /**
   * 行内模式（2026-10-19）：把标签并进元信息那一行，未编辑时只显示一个「+ 标签」。
   *
   * 背景：标签输入框原本独占一行，没有标签时也占着 ~40px；用户希望标题区再压一压。
   * 行内模式下，没在编辑时输入框收成一个小按钮；点一下才展开，失焦且没输入时自动收回。
   */
  inline?: boolean;
}) {
  const [input, setInput] = useState('');
  const [editing, setEditing] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const suggestions = useTagsStore((state) => state.tags);

  useEffect(() => {
    if (editing) inputRef.current?.focus();
  }, [editing]);

  const add = (value: string) => {
    const trimmed = value.trim();
    if (!trimmed) return;
    if (tags.some((tag) => tag.toLowerCase() === trimmed.toLowerCase())) {
      setInput('');
      return;
    }
    onChange([...tags, trimmed]);
    setInput('');
  };

  const remove = (value: string) => onChange(tags.filter((tag) => tag !== value));

  const related = suggestions
    .filter(
      (tag) =>
        input.length > 0 &&
        tag.name.toLowerCase().includes(input.toLowerCase()) &&
        !tags.some((existing) => existing.toLowerCase() === tag.name.toLowerCase()),
    )
    .slice(0, 5);

  return (
    <div className={inline ? 'relative flex flex-wrap items-center gap-1' : 'space-y-1'}>
      <div className="flex flex-wrap items-center gap-1">
        {tags.map((tag) => (
          <span
            key={tag}
            className="flex items-center gap-1 rounded-full bg-[var(--brand-soft)] px-2 py-0.5 text-[11px] text-[var(--brand)]"
          >
            {tag}
            <button type="button" onClick={() => remove(tag)} aria-label={`移除标签 ${tag}`}>
              <X className="h-3 w-3" />
            </button>
          </span>
        ))}
        {!inline || editing ? (
          <Input
            ref={inputRef}
            aria-label="添加标签"
            value={input}
            placeholder={tags.length === 0 ? placeholder : '添加标签'}
            className={
              inline
                ? 'h-6 w-28 border-none px-1 text-[11px] focus-visible:ring-0'
                : 'h-7 w-32 border-none px-1 text-xs focus-visible:ring-0'
            }
            onChange={(event) => setInput(event.target.value)}
            onBlur={() => {
              if (inline && !input.trim()) setEditing(false);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault();
                add(input);
                if (inline && !input.trim()) setEditing(false);
              } else if (event.key === 'Escape') {
                setInput('');
                setEditing(false);
              } else if (event.key === 'Backspace' && !input && tags.length > 0) {
                remove(tags[tags.length - 1] as string);
              }
            }}
          />
        ) : (
          <button
            type="button"
            aria-label="添加标签"
            className="flex items-center gap-0.5 rounded-full px-1.5 py-0.5 text-[11px] text-[var(--ink-subtle)] transition-colors hover:bg-[var(--surface-muted)] hover:text-[var(--brand)]"
            onClick={() => setEditing(true)}
          >
            <Plus className="h-3 w-3" />
            标签
          </button>
        )}
      </div>
      {related.length > 0 ? (
        <div
          className={
            inline
              ? 'absolute left-0 top-full z-20 mt-1 flex w-56 flex-wrap gap-1 rounded-md border border-[var(--border-subtle)] bg-[var(--surface-raised)] p-1.5 shadow-lg'
              : 'flex flex-wrap gap-1'
          }
        >
          {related.map((tag) => (
            <button
              key={tag.id}
              type="button"
              className={cn(
                'rounded-full border border-[var(--border-subtle)] px-2 py-0.5 text-[11px] text-[var(--ink-muted)]',
                'hover:border-[var(--brand)] hover:text-[var(--brand)]',
              )}
              onClick={() => add(tag.name)}
            >
              {tag.name}
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}
