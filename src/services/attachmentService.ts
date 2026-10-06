import type {
  AttachmentData,
  AttachmentListReport,
  AttachmentReference,
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

  /** 附件清单（含引用它的笔记 / 总占用） */
  list: () => call<AttachmentListReport>('attachment_list'),

  /** 删除附件（仅限 attachments/ 目录），返回释放的字节数 */
  remove: (relativePath: string) => call<number>('attachment_delete', { relativePath }),

  /** 某个附件被哪些笔记引用 */
  references: (relativePath: string) =>
    call<AttachmentReference[]>('attachment_references', { relativePath }),
};

export type {
  AttachmentData,
  AttachmentListReport,
  AttachmentReference,
  ReadAttachmentInput,
  SaveAttachmentInput,
  SavedAttachment,
};
