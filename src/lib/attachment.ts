/** 附件粘贴相关的纯函数（便于单测，不依赖编辑器实例）。 */

/** 由文件名推断 Markdown 图片的 alt 文本（去掉扩展名与时间戳前缀）。 */
export function attachmentAltText(fileName?: string | null): string {
  if (!fileName) return '图片';
  const base = fileName.split(/[\\/]/).pop() ?? fileName;
  const stem = base.replace(/\.[A-Za-z0-9]{1,8}$/, '').replace(/^\d{8}-\d{6}-/, '');
  const cleaned = stem.trim();
  return cleaned.length > 0 ? cleaned : '图片';
}

/**
 * 生成图片 Markdown。
 * 路径包含空格或括号时使用 `<...>` 包裹，避免被 Markdown 解析器截断。
 */
export function buildImageMarkdown(linkPath: string, alt = '图片'): string {
  const normalized = linkPath.trim().replace(/\\/g, '/');
  const target = /[\s()<>]/.test(normalized) ? `<${normalized}>` : normalized;
  const safeAlt = alt.replace(/[[\]]/g, '').trim() || '图片';
  return `![${safeAlt}](${target})`;
}

/** 读取文件为 data URL（供后端拆分 Base64 落盘）。 */
export function readFileAsDataUrl(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result === 'string') resolve(result);
      else reject(new Error('读取文件失败'));
    };
    reader.onerror = () => reject(reader.error ?? new Error('读取文件失败'));
    reader.readAsDataURL(file);
  });
}

/** 从 data URL 中提取 MIME 类型。 */
export function mimeFromDataUrl(dataUrl: string): string | null {
  const match = /^data:([^;,]+)[;,]/.exec(dataUrl);
  return match?.[1] ?? null;
}
