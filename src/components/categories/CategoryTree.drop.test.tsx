import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { CategoryTree } from '@/components/categories/CategoryTree';
import { NOTE_DRAG_TYPE } from '@/lib/dragNotes';
import { noteService } from '@/services/noteService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';

vi.mock('@/services/noteService', () => ({
  noteService: { moveMany: vi.fn(), list: vi.fn() },
}));

const moveMany = vi.mocked(noteService.moveMany);

function dragEventWith(ids: string[]) {
  const data = new Map<string, string>([[NOTE_DRAG_TYPE, JSON.stringify(ids)]]);
  return {
    dataTransfer: {
      types: [NOTE_DRAG_TYPE],
      getData: (type: string) => data.get(type) ?? '',
      setData: (type: string, value: string) => data.set(type, value),
      dropEffect: 'none',
    },
  };
}

describe('CategoryTree 拖拽移动', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    moveMany.mockResolvedValue({ succeeded: 2, failures: [] });
    useCategoriesStore.setState({
      tree: [
        {
          category: {
            id: 'cat_tech',
            workspaceId: 'ws',
            parentId: null,
            name: '技术',
            sortOrder: 1,
            dirName: '01_技术',
            createdAt: '2026-10-01T00:00:00+08:00',
            noteCount: 0,
          },
          depth: 0,
          children: [],
        },
      ],
      flat: [],
    });
    useNotesStore.setState({ items: [], total: 0, selection: [], selectionMode: false });
  });

  it('拖拽悬停时高亮，放下后移动整批笔记', async () => {
    render(
      <MemoryRouter>
        <CategoryTree />
      </MemoryRouter>,
    );

    const target = await screen.findByTestId('category-drop-cat_tech');
    const drag = dragEventWith(['note_a', 'note_b']);

    fireEvent.dragOver(target, drag);
    expect(target.className).toContain('ring-2');

    fireEvent.drop(target, drag);
    await waitFor(() => expect(moveMany).toHaveBeenCalledWith(['note_a', 'note_b'], 'cat_tech'));
  });

  it('非笔记拖拽不触发移动', async () => {
    render(
      <MemoryRouter>
        <CategoryTree />
      </MemoryRouter>,
    );
    const target = await screen.findByTestId('category-drop-cat_tech');
    fireEvent.drop(target, { dataTransfer: { types: [], getData: () => '' } });
    await waitFor(() => expect(moveMany).not.toHaveBeenCalled());
  });
});
