import type {
  OrganizeJob,
  OrganizeBatchReport,
  OrganizeSuggestion,
  OrganizeSuggestionKind,
  OrganizeSuggestionStatus,
} from '@/types';

import { call } from './api';

export const organizeService = {
  generate: (noteId: string, kindValue: OrganizeSuggestionKind, confirmOutbound = false) =>
    call<OrganizeSuggestion>('organize_generate', {
      noteId,
      kindValue,
      confirmOutbound,
    }),
  list: (
    statusValue: OrganizeSuggestionStatus | null = null,
    kindValue: OrganizeSuggestionKind | null = null,
    limit = 200,
  ) =>
    call<OrganizeSuggestion[]>('organize_suggestions', {
      statusValue,
      kindValue,
      limit,
    }),
  accept: (id: string) => call<OrganizeSuggestion>('organize_accept', { id }),
  acceptMany: (ids: string[]) => call<OrganizeBatchReport>('organize_accept_many', { ids }),
  reject: (id: string) => call<OrganizeSuggestion>('organize_reject', { id }),
  undo: (id: string) => call<OrganizeSuggestion>('organize_undo', { id }),

  createJob: (kindValue: OrganizeSuggestionKind, noteIds: string[], confirmOutbound = false) =>
    call<OrganizeJob>('organize_job_create', {
      kindValue,
      noteIds: [...noteIds],
      confirmOutbound,
    }),
  job: (id: string) => call<OrganizeJob>('organize_job_get', { id }),
  jobs: (limit = 20) => call<OrganizeJob[]>('organize_job_list', { limit }),
  runJob: (id: string) => call<OrganizeJob>('organize_job_run', { id }),
  pauseJob: (id: string) => call<OrganizeJob>('organize_job_pause', { id }),
  resumeJob: (id: string) => call<OrganizeJob>('organize_job_resume', { id }),
  cancelJob: (id: string) => call<OrganizeJob>('organize_job_cancel', { id }),
  retryJob: (id: string) => call<OrganizeJob>('organize_job_retry', { id }),
  recover: () => call<number>('organize_recover'),
};
