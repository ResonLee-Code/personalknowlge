import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { CreateChildNoteDialog } from '@/components/notes/CreateChildNoteDialog';
import { useNotesStore } from '@/stores/notesStore';
import type { NoteSummary } from '@/types';

const parent: NoteSummary = {
  id: 'note_parent',
  title: '广告行业裁切机技术流程方案说明',
  filePath: '01_技术/parent.md',
  categoryId: 'cat_1',
  categoryPath: '技术',
  status: 'active',
  favorite: false,
  wordCount: 100,
  createdAt: '2026-10-01T00:00:00+08:00',
  updatedAt: '2026-10-01T00:00:00+08:00',
  tags: [],
};

describe('CreateChildNoteDialog', () => {
  beforeEach(() => {
    useNotesStore.setState({ error: null });
  });

  it('以父文档 id 创建子文档并关闭弹窗', async () => {
    const create = vi.fn().mockResolvedValue({
      note: { ...parent, id: 'note_child', title: '1.1 目的', parentId: parent.id },
      content: '',
      categoryPath: '技术',
      tags: [],
    });
    useNotesStore.setState({ create });
    const onClose = vi.fn();

    render(
      <MemoryRouter>
        <CreateChildNoteDialog parent={parent} onClose={onClose} />
      </MemoryRouter>,
    );

    expect(screen.getByText(/父文档/)).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText('子文档标题'), '1.1 目的');
    await userEvent.click(screen.getByRole('button', { name: '创建子文档' }));

    await waitFor(() => {
      expect(create).toHaveBeenCalledWith({ title: '1.1 目的', parentId: 'note_parent' });
    });
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it('标题为空时提示且不创建', async () => {
    const create = vi.fn();
    useNotesStore.setState({ create });
    render(
      <MemoryRouter>
        <CreateChildNoteDialog parent={parent} onClose={() => {}} />
      </MemoryRouter>,
    );

    await userEvent.click(screen.getByRole('button', { name: '创建子文档' }));
    expect(create).not.toHaveBeenCalled();
  });
});

describe('CreateChildNoteDialog 同层重名提示', () => {
  it('同一父文档下已有同名子文档时给出提醒（不阻断创建）', async () => {
    useNotesStore.setState({
      items: [
        {
          id: 'note_sibling',
          title: '1.1 目的',
          parentId: 'note_parent',
          filePath: 'a.md',
          categoryPath: '技术',
          status: 'active',
          favorite: false,
          wordCount: 1,
          createdAt: '2026-10-01T00:00:00+08:00',
          updatedAt: '2026-10-01T00:00:00+08:00',
          tags: [],
        },
      ],
    });

    render(
      <MemoryRouter>
        <CreateChildNoteDialog parent={parent} onClose={() => {}} />
      </MemoryRouter>,
    );

    await userEvent.type(screen.getByLabelText('子文档标题'), '1.1 目的');
    expect(await screen.findByTestId('child-title-duplicate')).toBeInTheDocument();
  });
});
