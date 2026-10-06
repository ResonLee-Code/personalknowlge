import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { RelationsPanel } from '@/components/editor/RelationsPanel';
import { noteService } from '@/services/noteService';
import { relationService } from '@/services/relationService';

vi.mock('@/services/relationService', () => ({
  relationService: { list: vi.fn(), add: vi.fn(), remove: vi.fn(), rebuild: vi.fn() },
}));
vi.mock('@/services/noteService', () => ({
  noteService: { list: vi.fn(), get: vi.fn() },
}));

const list = vi.mocked(relationService.list);
const add = vi.mocked(relationService.add);
const remove = vi.mocked(relationService.remove);

const empty = { outgoing: [], incoming: [] };

function renderPanel() {
  return render(
    <MemoryRouter>
      <RelationsPanel noteId="note_a" />
    </MemoryRouter>,
  );
}

describe('RelationsPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    list.mockResolvedValue(empty);
    vi.mocked(noteService.list).mockResolvedValue({ items: [], total: 0, page: 1, pageSize: 20 });
  });

  it('默认折叠；展开后在没有关系时给出引导文案', async () => {
    renderPanel();
    expect(await screen.findByText('出 0 · 入 0')).toBeInTheDocument();
    // 默认折叠：内容不渲染（避免挤占编辑区高度）
    expect(screen.queryByText(/还没有关系/)).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '关系' }));
    expect(await screen.findByText(/还没有关系/)).toBeInTheDocument();
  });

  it('展示出边与入边，并支持删除', async () => {
    list.mockResolvedValue({
      outgoing: [
        { id: 'r1', relationType: 'SUPERSEDES', targetNoteId: 'note_b', targetTitle: '旧方案', note: '被新版取代' },
      ],
      incoming: [
        { id: 'r2', relationType: 'CONTRADICTS', sourceNoteId: 'note_c', sourceTitle: '争议记录' },
      ],
    });
    remove.mockResolvedValue(empty);

    renderPanel();
    await userEvent.click(await screen.findByRole('button', { name: '关系' }));
    expect(await screen.findByTestId('relation-outgoing')).toHaveTextContent('取代');
    expect(screen.getByTestId('relation-incoming')).toHaveTextContent('冲突');

    await userEvent.click(screen.getByRole('button', { name: '删除关系：旧方案' }));
    await waitFor(() => expect(remove).toHaveBeenCalledWith('r1'));
  });

  it('新增关系时提交来源、目标与类型', async () => {
    vi.mocked(noteService.list).mockResolvedValue({
      items: [
        {
          id: 'note_b',
          title: '索引设计 v1',
          filePath: '00_Inbox/a.md',
          categoryPath: '',
          status: 'active',
          favorite: false,
          wordCount: 10,
          tags: [],
          createdAt: '2026-10-01T00:00:00+08:00',
          updatedAt: '2026-10-01T00:00:00+08:00',
        },
      ],
      total: 1,
      page: 1,
      pageSize: 20,
    });
    add.mockResolvedValue(empty);

    renderPanel();
    await screen.findByTestId('relations-panel');
    await userEvent.click(screen.getByRole('button', { name: /添加关系/ }));

    await screen.findByRole('option', { name: /取代/ });
    await userEvent.selectOptions(screen.getByLabelText('关系类型'), 'SUPERSEDES');
    await userEvent.click(await screen.findByRole('button', { name: '索引设计 v1' }));
    await userEvent.click(screen.getByRole('button', { name: '添加' }));

    await waitFor(() =>
      expect(add).toHaveBeenCalledWith({
        sourceId: 'note_a',
        targetId: 'note_b',
        relationType: 'SUPERSEDES',
        note: null,
      }),
    );
  });
});
