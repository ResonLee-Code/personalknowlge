import { autocompletion } from '@codemirror/autocomplete';
import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { EditorView } from '@codemirror/view';
import { oneDark } from '@codemirror/theme-one-dark';
import CodeMirror, { type ReactCodeMirrorRef } from '@uiw/react-codemirror';
import { useCallback, useMemo, type RefObject } from 'react';

import { wikilinkCompletionSource } from './wikilinkCompletion';

interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
  theme: 'light' | 'dark';
  fontSize: number;
  readOnly?: boolean;
  placeholder?: string;
  /**
   * 粘贴 / 拖入图片时的处理函数：返回要插入正文的 Markdown（`null` 表示忽略）。
   * 由调用方负责落盘（桌面端写入 attachments/），编辑器只负责插入。
   */
  onAttachFiles?: (files: File[]) => Promise<string | null>;
  /** 暴露 CodeMirror 实例，供大纲跳转等外部操作使用 */
  viewRef?: RefObject<ReactCodeMirrorRef | null>;
  /** 提供 `[[` 补全的候选标题（不传则不启用自动补全） */
  getWikilinkTitles?: () => readonly string[];
}

/** CodeMirror 6 Markdown 编辑器（设计文档 7.1 / 16.5）。 */
export function MarkdownEditor({
  value,
  onChange,
  theme,
  fontSize,
  readOnly = false,
  placeholder = '开始记录…（支持 Markdown 语法）',
  onAttachFiles,
  viewRef,
  getWikilinkTitles,
}: MarkdownEditorProps) {
  const insertSnippet = useCallback((view: EditorView, snippet: string, at?: number) => {
    const position = at ?? view.state.selection.main.head;
    view.dispatch({
      changes: { from: position, insert: snippet },
      selection: { anchor: position + snippet.length },
      scrollIntoView: true,
    });
    view.focus();
  }, []);

  const handleFiles = useCallback(
    (files: File[], view: EditorView, at?: number): boolean => {
      const images = files.filter((file) => file.type.startsWith('image/'));
      if (!onAttachFiles || images.length === 0) return false;
      void (async () => {
        const snippet = await onAttachFiles(images);
        if (snippet) insertSnippet(view, snippet, at);
      })();
      return true;
    },
    [insertSnippet, onAttachFiles],
  );

  const extensions = useMemo(
    () => [
      markdown({ base: markdownLanguage }),
      EditorView.lineWrapping,
      EditorView.domEventHandlers({
        paste: (event, view) => {
          const files = Array.from(event.clipboardData?.files ?? []);
          return handleFiles(files, view);
        },
        drop: (event, view) => {
          const files = Array.from(event.dataTransfer?.files ?? []);
          const at = view.posAtCoords({ x: event.clientX, y: event.clientY });
          return handleFiles(files, view, at ?? undefined);
        },
      }),
      ...(getWikilinkTitles
        ? [
            autocompletion({
              override: [wikilinkCompletionSource(getWikilinkTitles)],
              closeOnBlur: true,
            }),
          ]
        : []),
    ],
    [handleFiles, getWikilinkTitles],
  );

  return (
    <div className="h-full min-h-0 w-full" style={{ fontSize }}>
      <CodeMirror
        ref={viewRef}
        value={value}
        height="100%"
        className="h-full"
        theme={theme === 'dark' ? oneDark : 'light'}
        extensions={extensions}
        editable={!readOnly}
        placeholder={placeholder}
        basicSetup={{
          lineNumbers: true,
          foldGutter: false,
          highlightActiveLine: true,
          highlightActiveLineGutter: false,
          autocompletion: false,
          searchKeymap: true,
        }}
        onChange={onChange}
      />
    </div>
  );
}
