import { markdown, markdownLanguage } from '@codemirror/lang-markdown';
import { EditorView } from '@codemirror/view';
import { oneDark } from '@codemirror/theme-one-dark';
import CodeMirror from '@uiw/react-codemirror';
import { useMemo } from 'react';

interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
  theme: 'light' | 'dark';
  fontSize: number;
  readOnly?: boolean;
  placeholder?: string;
}

/** CodeMirror 6 Markdown 编辑器（设计文档 7.1 / 16.5）。 */
export function MarkdownEditor({
  value,
  onChange,
  theme,
  fontSize,
  readOnly = false,
  placeholder = '开始记录…（支持 Markdown 语法）',
}: MarkdownEditorProps) {
  const extensions = useMemo(
    () => [
      markdown({ base: markdownLanguage }),
      EditorView.lineWrapping,
    ],
    [],
  );

  return (
    <div className="h-full min-h-0 w-full" style={{ fontSize }}>
      <CodeMirror
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
