import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { CheckCircle2, Download, FileCode2, FileText, FileType2, Loader2, Printer } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { errorMessage } from '@/lib/format';
import { cn } from '@/lib/utils';
import { isTauriRuntime } from '@/services/api';
import { openPath, pickSavePath } from '@/services/appService';
import { noteService } from '@/services/noteService';
import type { ExportFormat, ExportNoteResult } from '@/types';

interface FormatOption {
  value: ExportFormat;
  label: string;
  hint: string;
  icon: typeof FileText;
  extension: string | null;
}

const FORMATS: FormatOption[] = [
  {
    value: 'markdown',
    label: 'Markdown (.md)',
    hint: '原样导出，含 frontmatter，数据无损，适合备份与再次导入',
    icon: FileCode2,
    extension: 'md',
  },
  {
    value: 'html',
    label: '网页 (.html)',
    hint: '自包含单文件：内联样式 + 工作区内的图片内联为 data URL，可直接在浏览器中打印为 PDF',
    icon: FileText,
    extension: 'html',
  },
  {
    value: 'docx',
    label: 'Word (.docx)',
    hint: 'Word / WPS 可直接打开编辑，也可在 Word 中另存为 PDF',
    icon: FileType2,
    extension: 'docx',
  },
  {
    value: 'pdf',
    label: 'PDF（打印视图）',
    hint: '打开打印视图（图片同样内联），在系统打印面板中选择「存储为 PDF」',
    icon: Printer,
    extension: null,
  },
];

/** 导出单篇笔记（设计文档：数据自主，随时可导出离开）。 */
export function ExportNoteDialog({
  noteId,
  title,
  onClose,
}: {
  noteId: string;
  title: string;
  onClose: () => void;
}) {
  const [format, setFormat] = useState<ExportFormat>('markdown');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ExportNoteResult | null>(null);
  const desktop = isTauriRuntime();
  const selected = FORMATS.find((item) => item.value === format) as FormatOption;

  const run = async () => {
    if (!desktop) {
      toast.error('浏览器预览模式不支持导出到本地文件，请在桌面应用中使用。');
      return;
    }
    setBusy(true);
    try {
      let targetPath: string | null = null;
      if (selected.extension) {
        targetPath = await pickSavePath(
          `${title}.${selected.extension}`,
          [selected.extension],
          selected.label,
        );
        if (!targetPath) {
          setBusy(false);
          return; // 用户取消
        }
      }

      const exported = await noteService.exportNote({
        id: noteId,
        format,
        targetPath,
      });
      setResult(exported);

      if (exported.printReady) {
        await openPath(exported.path);
        toast.success('已打开打印视图：在打印面板中选择「存储为 PDF」即可保存', {
          action: {
            label: '在文件夹中显示',
            onClick: () => void revealItemInDir(exported.path),
          },
        });
      } else {
        toast.success(`已导出 ${exported.fileName}`, {
          action: {
            label: '在文件夹中显示',
            onClick: () => void revealItemInDir(exported.path),
          },
        });
      }
    } catch (error) {
      toast.error(errorMessage(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open onOpenChange={(next) => (!next ? onClose() : undefined)}>
      <DialogContent className="w-[min(560px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle>导出笔记</DialogTitle>
          <DialogDescription>
            选择导出格式。Markdown 与 HTML 为纯文本，Word 可直接编辑，PDF 通过打印视图另存。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-2">
          {FORMATS.map((option) => (
            <button
              key={option.value}
              type="button"
              onClick={() => setFormat(option.value)}
              className={cn(
                'flex w-full items-start gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors',
                format === option.value
                  ? 'border-[var(--brand)] bg-[var(--brand-soft)]'
                  : 'border-[var(--border-subtle)] hover:bg-[var(--surface-muted)]',
              )}
            >
              <option.icon
                className={cn(
                  'mt-0.5 h-4 w-4 shrink-0',
                  format === option.value ? 'text-[var(--brand)]' : 'text-[var(--ink-subtle)]',
                )}
              />
              <span className="min-w-0">
                <span className="block text-sm text-[var(--ink)]">{option.label}</span>
                <span className="mt-0.5 block text-[11px] leading-relaxed text-[var(--ink-subtle)]">
                  {option.hint}
                </span>
              </span>
            </button>
          ))}
        </div>

        {result ? (
          <p className="flex items-center gap-1.5 rounded-md bg-[var(--surface-muted)] px-3 py-2 text-[11px] text-[var(--ink-muted)]">
            <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600" />
            <span className="min-w-0 truncate" title={result.path}>
              {result.fileName} · {(result.bytes / 1024).toFixed(1)} KB · {result.path}
            </span>
          </p>
        ) : null}

        {!desktop ? (
          <p className="text-[11px] text-amber-600">
            浏览器预览模式无法写入本地文件，请在桌面应用中使用导出功能。
          </p>
        ) : null}

        <DialogFooter>
          <Button variant="secondary" onClick={onClose} disabled={busy}>
            完成
          </Button>
          <Button onClick={() => void run()} disabled={busy}>
            {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Download className="h-3.5 w-3.5" />}
            {selected.extension ? '导出' : '打开打印视图'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
