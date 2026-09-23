import { openUrl } from '@tauri-apps/plugin-opener';
import { ExternalLink } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import remarkGfm from 'remark-gfm';

import { EmptyState } from '@/components/ui/empty-state';
import { isTauriRuntime } from '@/services/api';

/** Markdown 预览：不启用原始 HTML，避免执行笔记中的脚本（设计文档 6.3）。 */
export function MarkdownPreview({ content }: { content: string }) {
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
          a: ({ href, children, ...props }) => (
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
          ),
          table: ({ children, ...props }) => (
            <div className="overflow-x-auto">
              <table {...props}>{children}</table>
            </div>
          ),
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}
