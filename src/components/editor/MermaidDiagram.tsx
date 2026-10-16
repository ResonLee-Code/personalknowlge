import { useEffect, useState } from 'react';

import { useUiStore } from '@/stores/uiStore';

/**
 * 预览里的 Mermaid 图（2026-10-16 新增）。
 *
 * 解决什么：正文里写 ```mermaid（流程图 / 时序图 / 类图 / 状态图 / 甘特图 / 思维导图…）时，
 * 以前只当普通代码块显示——用户写了图，看到的却是一坨文本。
 *
 * 几个刻意的选择：
 * - **按需加载**：mermaid 本身是个大依赖，走 `import()`，不拖慢首屏；
 * - **`securityLevel: 'strict'`**：图形里的链接 / HTML 一律不执行（正文是本地内容，但仍不放松）；
 * - **主题跟随应用**：深色主题下用 dark 主题，切换主题会重新渲染；
 * - **失败说清楚**：语法写错时显示原因 + 原始代码，而不是留一块空白（本项目的失败口径）。
 */
export function MermaidDiagram({ source }: { source: string }) {
  const theme = useUiStore((state) => state.theme);
  /**
   * 用 `key` 让"源码变了 / 主题切了"直接**重挂载**内层组件。
   * 不在 effect 里同步 setState 重置（那会触发级联渲染，项目 lint 专门拦这条）。
   */
  return <DiagramRender key={`${theme}\u0000${source}`} source={source} dark={theme === 'dark'} />;
}

function DiagramRender({ source, dark }: { source: string; dark: boolean }) {
  const [svg, setSvg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const mermaid = (await import('mermaid')).default;
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: 'strict',
          theme: dark ? 'dark' : 'default',
        });
        const id = `mermaid-${Math.random().toString(36).slice(2, 10)}`;
        const rendered = await mermaid.render(id, source);
        if (!cancelled) setSvg(rendered.svg);
      } catch (cause) {
        if (!cancelled) setError(cause instanceof Error ? cause.message : String(cause));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [source, dark]);

  if (error) {
    return (
      <div
        data-testid="mermaid-error"
        className="my-2 rounded-md border border-amber-500/40 bg-amber-500/5 p-2 text-xs"
      >
        <p className="text-amber-600">图形没渲染出来（语法可能有问题）：{error}</p>
        <pre className="mt-1 overflow-x-auto rounded bg-[var(--surface-muted)] p-2">
          <code>{source}</code>
        </pre>
      </div>
    );
  }

  if (!svg) {
    return (
      <div data-testid="mermaid-loading" className="my-2 text-xs text-[var(--ink-subtle)]">
        正在渲染图形…
      </div>
    );
  }

  return (
    <div
      data-testid="mermaid-diagram"
      className="my-2 flex justify-center overflow-x-auto rounded-md border border-[var(--border-subtle)] bg-[var(--surface)] p-2"
      // mermaid 自己产出 SVG 字符串；strict 模式下不含脚本
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
}
