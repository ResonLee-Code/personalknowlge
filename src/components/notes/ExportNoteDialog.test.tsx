import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { ExportNoteDialog } from '@/components/notes/ExportNoteDialog';

describe('ExportNoteDialog', () => {
  it('提供四种导出格式并说明各自用途', () => {
    render(<ExportNoteDialog noteId="note_1" title="测试笔记" onClose={() => {}} />);

    expect(screen.getByText('Markdown (.md)')).toBeInTheDocument();
    expect(screen.getByText('网页 (.html)')).toBeInTheDocument();
    expect(screen.getByText('Word (.docx)')).toBeInTheDocument();
    expect(screen.getByText('PDF（打印视图）')).toBeInTheDocument();

    // 默认选中 Markdown，主按钮为「导出」（testing-library 的字符串匹配即精确匹配）
    expect(screen.getByRole('button', { name: '导出' })).toBeInTheDocument();
  });

  it('选择 PDF 后按钮变为「打开打印视图」', async () => {
    render(<ExportNoteDialog noteId="note_1" title="测试笔记" onClose={() => {}} />);

    await userEvent.click(screen.getByText('PDF（打印视图）'));
    expect(screen.getByRole('button', { name: '打开打印视图' })).toBeInTheDocument();
  });

  it('浏览器预览模式提示需在桌面应用中使用导出', () => {
    render(<ExportNoteDialog noteId="note_1" title="测试笔记" onClose={() => {}} />);
    expect(screen.getByText(/浏览器预览模式无法写入本地文件/)).toBeInTheDocument();
  });

  it('关闭按钮回调生效', async () => {
    const onClose = vi.fn();
    render(<ExportNoteDialog noteId="note_1" title="测试笔记" onClose={onClose} />);
    await userEvent.click(screen.getByRole('button', { name: '完成' }));
    expect(onClose).toHaveBeenCalled();
  });
});
