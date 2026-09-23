import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';

import { NoteListItem } from '@/components/notes/NoteListItem';
import type { NoteSummary } from '@/types';

const note: NoteSummary = {
  id: 'note_1',
  title: 'SQLite 全文搜索学习笔记',
  filePath: '01_技术/note.md',
  categoryId: 'cat_1',
  categoryPath: '技术/数据库',
  status: 'active',
  favorite: true,
  wordCount: 120,
  createdAt: '2026-09-23T10:00:00+08:00',
  updatedAt: '2026-09-23T12:00:00+08:00',
  tags: ['sqlite', 'search'],
  snippet: [
    { text: '这里讨论 ', matched: false },
    { text: '全文搜索', matched: true },
    { text: ' 的实现', matched: false },
  ],
};

describe('NoteListItem', () => {
  it('展示标题、分类、标签与摘要高亮', () => {
    render(
      <MemoryRouter>
        <ul>
          <NoteListItem note={note} selected={false} basePath="/notes" onOpen={() => {}} />
        </ul>
      </MemoryRouter>,
    );

    expect(screen.getByText('SQLite 全文搜索学习笔记')).toBeInTheDocument();
    expect(screen.getByText(/技术\/数据库/)).toBeInTheDocument();
    expect(screen.getByText('sqlite')).toBeInTheDocument();
    expect(screen.getByText('全文搜索')).toHaveClass('search-highlight');
  });

  it('点击时触发打开回调', async () => {
    const onOpen = vi.fn();
    render(
      <MemoryRouter>
        <ul>
          <NoteListItem note={note} selected basePath="/notes" onOpen={onOpen} />
        </ul>
      </MemoryRouter>,
    );

    await userEvent.click(screen.getByText('SQLite 全文搜索学习笔记'));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });
});
