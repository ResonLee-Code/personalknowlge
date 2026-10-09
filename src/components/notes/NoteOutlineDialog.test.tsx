import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';

import { NoteOutlineDialog } from '@/components/notes/NoteOutlineDialog';
import { noteService } from '@/services/noteService';
import type { NoteSummary } from '@/types';

const note: NoteSummary = {
  id: 'note_1',
  title: '技术流程方案说明',
  filePath: '01_技术/note.md',
  categoryPath: '技术',
  status: 'active',
  favorite: false,
  wordCount: 100,
  createdAt: '2026-10-01T00:00:00+08:00',
  updatedAt: '2026-10-01T00:00:00+08:00',
  tags: [],
};

function LocationProbe() {
  const location = useLocation();
  return <p data-testid="location">{`${location.pathname}${location.search}`}</p>;
}

describe('NoteOutlineDialog', () => {
  it('解析正文标题层级，点击条目跳转到编辑器对应位置', async () => {
    vi.spyOn(noteService, 'get').mockResolvedValue({
      note: { ...note, workspaceId: 'ws', status: 'active' },
      content: '# 文档概述\n\n正文\n\n## 目的\n\n### 术语\n',
      categoryPath: '技术',
      tags: [],
    });
    const onClose = vi.fn();

    render(
      <MemoryRouter initialEntries={['/notes']}>
        <Routes>
          <Route
            path="*"
            element={
              <>
                <NoteOutlineDialog note={note} onClose={onClose} />
                <LocationProbe />
              </>
            }
          />
        </Routes>
      </MemoryRouter>,
    );

    const items = await screen.findAllByTestId('outline-preview-item');
    expect(items.map((item) => item.textContent)).toEqual(['文档概述', '目的', '术语']);

    await userEvent.click(items[1]!);
    // 第二行标题的偏移量（"## 目的" 前有 12 个字符：'# 文档概述\n\n正文\n\n'）
    expect(screen.getByTestId('location')).toHaveTextContent('/notes/note_1?h=12');
    expect(onClose).toHaveBeenCalled();
    vi.restoreAllMocks();
  });

  it('正文没有标题时给出引导文案', async () => {
    vi.spyOn(noteService, 'get').mockResolvedValue({
      note: { ...note, workspaceId: 'ws', status: 'active' },
      content: '只有正文，没有标题。',
      categoryPath: '技术',
      tags: [],
    });

    render(
      <MemoryRouter>
        <NoteOutlineDialog note={note} onClose={() => {}} />
      </MemoryRouter>,
    );

    expect(await screen.findByText(/还没有标题/)).toBeInTheDocument();
    vi.restoreAllMocks();
  });
});
