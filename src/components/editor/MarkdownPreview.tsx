import { openUrl } from '@tauri-apps/plugin-opener';
import { ExternalLink, Link2, Plus } from 'lucide-react';
import { useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import remarkGfm from 'remark-gfm';

import { EmptyState } from '@/components/ui/empty-state';
import { cn } from '@/lib/utils';
import { parseWikilinkHref, preprocessWikilinks } from '@/lib/wikilink';
import { isTauriRuntime } from '@/services/api';
import { attachmentService } from '@/services/attachmentService';

/** 会话内缓存已解析的图片 data URL（key = 笔记 id + 原始 src）。 */
const imageCache = new Map<string, string>();

/** 只把工作区内的相对路径交给后端读取；外链、data URL、绝对路径原样使用。 */
function isWorkspaceRelative(source: string): boolean {
  const value = source.trim();
  if (!value) return false;
  if (value.startsWith('#') || value.startsWith('/')) return false;
  return !/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(value);
}

/**
 * 预览中的图片：相对路径 → 通过后端读成 data URL。
 *
 * 直接用 `<img src="../attachments/…">` 在桌面外壳里无法访问工作区文件，
 * 因此这里按需读取并内联；读取失败时保留原始 `src`（显示为破图，但正文不受影响）。
 */
function AttachmentImage({
  src,
  alt,
  noteId,
  ...props
}: React.ImgHTMLAttributes<HTMLImageElement> & { noteId?: string | null }) {
  const cacheKey = src ? `${noteId ?? ''}\u0000${src}` : '';
  const [resolved, setResolved] = useState<string | undefined>(() =>
    cacheKey ? imageCache.get(cacheKey) : undefined,
  );

  useEffect(() => {
    if (!src || !cacheKey || !isWorkspaceRelative(src) || imageCache.has(cacheKey)) return;
    let cancelled = false;
    void attachmentService
      .read({ noteId: noteId ?? null, source: src })
      .then((data) => {
        imageCache.set(cacheKey, data.dataUrl);
        if (!cancelled) setResolved(data.dataUrl);
      })
      .catch(() => {
        // 读取失败：静默保留原始路径，控制台不刷错误（图片只是展示层）
      });
    return () => {
      cancelled = true;
    };
  }, [src, cacheKey, noteId]);

  return (
    <img
      {...props}
      src={resolved ?? src}
      alt={alt ?? ''}
      loading="lazy"
      className="max-h-[420px] rounded-md border border-[var(--border-subtle)]"
    />
  );
}

/** Markdown 预览：不启用原始 HTML，避免执行笔记中的脚本（设计文档 6.3）。 */
export function MarkdownPreview({
  content,
  noteId,
  resolveWikilink,
  onOpenWikilink,
}: {
  content: string;
  /** 当前笔记 id；用于把相对图片路径解析到工作区 */
  noteId?: string | null;
  /** 判断双向链接目标是否已存在（返回笔记 id 表示已解析） */
  resolveWikilink?: (target: string) => string | null;
  /** 点击 chip：已解析则跳转，未解析则创建 */
  onOpenWikilink?: (target: string) => void;
}) {
  if (!content.trim()) {
    return (
      <EmptyState
        title="这篇笔记还没有正文"
        description="切换到编辑模式开始记录内容。"
        className="h-full"
      />
    );
  }

  const handleLink = (href?: string) => {
    if (!href) return;
    if (!isTauriRuntime()) {
      window.open(href, '_blank', 'noreferrer');
      return;
    }
    void openUrl(href);
  };

  return (
    <div className="markdown-body px-6 py-5">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[rehypeHighlight]}
        components={{
          a: ({ href, children, ...props }) => {
            const wikilink = parseWikilinkHref(href);
            if (wikilink) {
              const resolved = resolveWikilink?.(wikilink.target);
              return (
                <button
                  type="button"
                  data-testid="wikilink-chip"
                  title={
                    resolved
                      ? `打开「${wikilink.target}」`
                      : `「${wikilink.target}」还没有对应笔记，点击创建`
                  }
                  onClick={() => onOpenWikilink?.(wikilink.target)}
                  className={cn(
                    'mx-0.5 inline-flex items-center gap-0.5 rounded px-1 py-0.5 align-baseline text-[0.95em] transition-colors',
                    resolved
                      ? 'bg-[var(--brand-soft)] text-[var(--brand)] hover:brightness-95'
                      : 'border border-dashed border-amber-500/60 text-amber-600 hover:bg-amber-500/10',
                  )}
                >
                  {resolved ? <Link2 className="h-3 w-3" /> : <Plus className="h-3 w-3" />}
                  {children}
                </button>
              );
            }
            return (
              <a
                {...props}
                href={href}
                onClick={(event) => {
                  event.preventDefault();
                  handleLink(href);
                }}
              >
                {children}
                <ExternalLink className="ml-0.5 inline h-3 w-3 opacity-60" />
              </a>
            );
          },
          img: ({ src, alt, ...props }) => (
            <AttachmentImage key={src ?? ''} src={src} alt={alt} noteId={noteId} {...props} />
          ),
          table: ({ children, ...props }) => (
            <div className="overflow-x-auto">
              <table {...props}>{children}</table>
            </div>
          ),
        }}
      >
        {preprocessWikilinks(content)}
      </ReactMarkdown>
    </div>
  );
}
