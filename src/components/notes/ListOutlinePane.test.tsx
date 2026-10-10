import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ListOutlinePane } from '@/components/notes/ListOutlinePane';
import { noteService } from '@/services/noteService';
import { useNotesStore } from '@/stores/notesStore';

function LocationProbe() {
  const location = useLocation();
  return <p data-testid="location">{`${location.pathname}${location.search}`}</p>;
}

function renderPane() {
  return render(
    <MemoryRouter initialEntries={['/notes']}>
      <Routes>
        <Route
          path="*"
          element={
            <>
              <ListOutlinePane />
              <LocationProbe />
            </>
          }
        />
      </Routes>
    </MemoryRouter>,
  );
}

describe('ListOutlinePane（列表侧常驻大纲）', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useNotesStore.setState({ selectedId: null });
  });

  it('未选中笔记时给出引导文案', () => {
    renderPane();
    expect(screen.getByText(/选中列表里的笔记/)).toBeInTheDocument();
  });

  it('选中笔记后展示标题层级，点击条目跳转到对应标题', async () => {
    vi.spyOn(noteService, 'get').mockResolvedValue({
      note: { id: 'note_1' },
      content: '# 文档概述\n\n正文\n\n## 目的\n\n### 术语\n',
      categoryPath: '',
      tags: [],
    } as never);

    useNotesStore.setState({ selectedId: 'note_1' });
    renderPane();

    const items = await screen.findAllByTestId('list-outline-item');
    expect(items.map((item) => item.textContent)).toEqual(['文档概述', '目的', '术语']);

    await userEvent.click(items[1]!);
    // 第二行标题的偏移量（"## 目的" 前有 12 个字符）
    expect(screen.getByTestId('location')).toHaveTextContent('/notes/note_1?h=12');
  });

  it('正文没有标题时给出书写引导；读取失败时显示错误', async () => {
    vi.spyOn(noteService, 'get').mockResolvedValueOnce({
      note: { id: 'note_1' },
      content: '只有正文。',
      categoryPath: '',
      tags: [],
    } as never);
    useNotesStore.setState({ selectedId: 'note_1' });
    const { unmount } = renderPane();
    expect(await screen.findByText(/还没有标题/)).toBeInTheDocument();
    unmount();

    vi.spyOn(noteService, 'get').mockRejectedValueOnce({ code: 'NOTE_NOT_FOUND', message: '笔记不存在' });
    renderPane();
    expect(await screen.findByText('笔记不存在')).toBeInTheDocument();
  });

  it('面板可折叠（收起后不再渲染条目，但仍记住内容）', async () => {
    vi.spyOn(noteService, 'get').mockResolvedValue({
      note: { id: 'note_1' },
      content: '# 甲\n## 乙\n',
      categoryPath: '',
      tags: [],
    } as never);
    useNotesStore.setState({ selectedId: 'note_1' });
    renderPane();
    await screen.findAllByTestId('list-outline-item');

    await userEvent.click(screen.getByTestId('list-outline-toggle'));
    await waitFor(() => expect(screen.queryAllByTestId('list-outline-item')).toHaveLength(0));
    expect(screen.getByTestId('list-outline-toggle')).toHaveAttribute('aria-expanded', 'false');
  });
});
