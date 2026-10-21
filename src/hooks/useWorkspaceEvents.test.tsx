import { renderHook, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { EVENTS } from '@/lib/constants';
import { subscribe } from '@/services/api';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { ScanReport } from '@/types';

import { useWorkspaceEvents } from './useWorkspaceEvents';

vi.mock('@/services/api', () => ({
  subscribe: vi.fn(),
}));

vi.mock('sonner', () => ({
  toast: { info: vi.fn(), warning: vi.fn() },
}));

describe('useWorkspaceEvents', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('refreshes tags and categories when an external file scan discovers new metadata', async () => {
    const handlers = new Map<string, (payload: unknown) => void>();
    vi.mocked(subscribe).mockImplementation(async (event, handler) => {
      handlers.set(event, handler as (payload: unknown) => void);
      return () => undefined;
    });
    useWorkspaceStore.setState({
      workspace: {
        id: 'workspace_1',
        name: 'test',
        rootPath: '/tmp/test',
        createdAt: '',
        updatedAt: '',
      },
    });
    const refreshNotes = vi.spyOn(useNotesStore.getState(), 'refresh').mockResolvedValue();
    const refreshStats = vi.spyOn(useWorkspaceStore.getState(), 'refreshStats').mockResolvedValue();
    const refreshCategories = vi
      .spyOn(useCategoriesStore.getState(), 'refresh')
      .mockResolvedValue();
    const refreshTags = vi.spyOn(useTagsStore.getState(), 'refresh').mockResolvedValue();

    renderHook(() => useWorkspaceEvents());
    await waitFor(() => expect(handlers.has(EVENTS.filesChanged)).toBe(true));
    refreshNotes.mockClear();
    refreshStats.mockClear();
    refreshCategories.mockClear();
    refreshTags.mockClear();

    handlers.get(EVENTS.filesChanged)?.({
      added: 1,
      updated: 0,
      missing: 0,
      unchanged: 0,
      durationMs: 1,
      warnings: [],
    } satisfies ScanReport);

    expect(refreshNotes).toHaveBeenCalledTimes(1);
    expect(refreshStats).toHaveBeenCalledTimes(1);
    expect(refreshCategories).toHaveBeenCalledTimes(1);
    expect(refreshTags).toHaveBeenCalledTimes(1);
  });
});
