import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';

import { NoteListItem } from '@/components/notes/NoteListItem';
import { useNotesStore } from '@/stores/notesStore';

const note = {
  id: 'note_1',
  title: 'SQLite 全文搜索学习笔记',
  filePath: '00_Inbox/a.md',
  categoryPath: 'Inbox',
  status: 'active' as const,
  favorite: false,
  wordCount: 10,
  tags: ['sqlite'],
  createdAt: '2026-10-01T00:00:00+08:00',
  updatedAt: '2026-10-01T00:00:00+08:00',
};

describe('NoteListItem 点击行为', () => {
  it('普通点击触发 onOpen，不进入多选', async () => {
    useNotesStore.setState({ selectionMode: false, selection: [] });
    const onOpen = vi.fn();
    render(
      <MemoryRouter>
        <NoteListItem note={note} selected={false} basePath="/notes" onOpen={onOpen} />
      </MemoryRouter>,
    );
    await userEvent.click(screen.getByRole('button', { name: /SQLite 全文搜索学习笔记/ }));
    expect(onOpen).toHaveBeenCalled();
    expect(useNotesStore.getState().selectionMode).toBe(false);
  });

  it('多选模式下点击勾选而不是打开', async () => {
    useNotesStore.setState({ selectionMode: true, selection: [] });
    const onOpen = vi.fn();
    render(
      <MemoryRouter>
        <NoteListItem note={note} selected={false} basePath="/notes" onOpen={onOpen} />
      </MemoryRouter>,
    );
    await userEvent.click(screen.getByRole('button', { name: /SQLite 全文搜索学习笔记/ }));
    expect(onOpen).not.toHaveBeenCalled();
    expect(useNotesStore.getState().selection).toEqual(['note_1']);
  });
});
