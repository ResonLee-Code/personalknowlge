import type {
  AttachmentData,
  ReadAttachmentInput,
  SaveAttachmentInput,
  SavedAttachment,
} from '@/types';

import { call } from './api';

export const attachmentService = {
  /** 把粘贴/拖入的图片写入 `attachments/YYYY-MM/`，返回可写入正文的链接路径 */
  save: (input: SaveAttachmentInput) => call<SavedAttachment>('attachment_save', { input }),

  /** 读取工作区内的相对图片，返回 data URL（供预览内联显示） */
  read: (input: ReadAttachmentInput) => call<AttachmentData>('attachment_read', { input }),
};

export type { AttachmentData, ReadAttachmentInput, SaveAttachmentInput, SavedAttachment };
