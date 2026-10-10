import type { ClippedPage } from '@/types';

import { call } from './api';

/** 网页剪藏：抓取公开网页并转成 Markdown（只读，写入走常规新建流程）。 */
export const clipService = {
  clip: (url: string) => call<ClippedPage>('note_clip_url', { url }),
};
