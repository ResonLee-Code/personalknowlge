import { describe, expect, it } from 'vitest';

import {
  attachmentAltText,
  buildImageMarkdown,
  mimeFromDataUrl,
  readFileAsDataUrl,
} from '@/lib/attachment';

describe('attachment helpers', () => {
  it('由文件名推断 alt 文本', () => {
    expect(attachmentAltText('截图 2026.png')).toBe('截图 2026');
    expect(attachmentAltText('20261007-101500-设计稿.png')).toBe('设计稿');
    expect(attachmentAltText(undefined)).toBe('图片');
    expect(attachmentAltText('.png')).toBe('图片');
  });

  it('生成图片 Markdown，路径含空格时使用尖括号', () => {
    expect(buildImageMarkdown('../attachments/a.png', '图')).toBe('![图](../attachments/a.png)');
    expect(buildImageMarkdown('../attachments/a b.png', '图')).toBe(
      '![图](<../attachments/a b.png>)',
    );
    // alt 中的方括号会被去掉，避免破坏语法
    expect(buildImageMarkdown('a.png', '图[1]')).toBe('![图1](a.png)');
  });

  it('从 data URL 提取 MIME', () => {
    expect(mimeFromDataUrl('data:image/png;base64,AAAA')).toBe('image/png');
    expect(mimeFromDataUrl('not-a-data-url')).toBeNull();
  });

  it('读取文件为 data URL', async () => {
    const file = new File(['hello'], 'a.txt', { type: 'text/plain' });
    const dataUrl = await readFileAsDataUrl(file);
    expect(dataUrl.startsWith('data:text/plain')).toBe(true);
  });
});
