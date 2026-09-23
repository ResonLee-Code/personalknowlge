import { X } from 'lucide-react';
import { useState } from 'react';

import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';
import { useTagsStore } from '@/stores/tagsStore';

/** 笔记标签编辑（设计文档 5.4 FR-T03）。 */
export function TagEditor({
  tags,
  onChange,
  placeholder = '输入标签后回车',
}: {
  tags: string[];
  onChange: (tags: string[]) => void;
  placeholder?: string;
}) {
  const [input, setInput] = useState('');
  const suggestions = useTagsStore((state) => state.tags);

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
    <div className="space-y-1">
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
        <Input
          value={input}
          placeholder={tags.length === 0 ? placeholder : '添加标签'}
          className="h-7 w-32 border-none px-1 text-xs focus-visible:ring-0"
          onChange={(event) => setInput(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              event.preventDefault();
              add(input);
            } else if (event.key === 'Backspace' && !input && tags.length > 0) {
              remove(tags[tags.length - 1] as string);
            }
          }}
        />
      </div>
      {related.length > 0 ? (
        <div className="flex flex-wrap gap-1">
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
