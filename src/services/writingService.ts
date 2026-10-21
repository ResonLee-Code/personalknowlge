import type {
  WritingApplyInput,
  WritingApplyPreview,
  WritingApplyResult,
  WritingDraft,
  WritingRequest,
} from '@/types';

import { call } from './api';

export const writingService = {
  draft: (input: WritingRequest) => call<WritingDraft>('writing_draft', { input }),
  applyPreview: (input: WritingApplyInput) =>
    call<WritingApplyPreview>('writing_apply_preview', { input }),
  apply: (input: WritingApplyInput) => call<WritingApplyResult>('writing_apply', { input }),
};
