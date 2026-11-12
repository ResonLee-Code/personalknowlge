import type {
  ContentCapture,
  ContentCaptureInput,
  ContentCaptureSaveInput,
  ContentCaptureSaveResult,
  ContentDiscoveryRunInput,
} from '@/types';

import { call } from './api';

export const contentDiscoveryService = {
  create: (input: ContentCaptureInput) => call<ContentCapture>('content_capture_create', { input }),
  list: (status?: string | null, limit = 50) =>
    call<ContentCapture[]>('content_capture_list', { status: status ?? null, limit }),
  get: (captureId: string) => call<ContentCapture>('content_capture_get', { captureId }),
  crawlSelected: (input: ContentDiscoveryRunInput) =>
    call<ContentCapture>('content_discovery_run', { input }),
  refresh: (captureId: string, confirmed: boolean) =>
    call<ContentCapture>('content_capture_refresh', { captureId, confirmed }),
  save: (input: ContentCaptureSaveInput) =>
    call<ContentCaptureSaveResult>('content_capture_save', { input }),
  ignore: (captureId: string) => call<ContentCapture>('content_capture_ignore', { captureId }),
};
