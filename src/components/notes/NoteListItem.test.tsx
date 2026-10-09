import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';

import { NoteListItem } from '@/components/notes/NoteListItem';
import { TooltipProvider } from '@/components/ui/tooltip';
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
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem note={note} selected={false} basePath="/notes" onOpen={() => {}} />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );

    expect(screen.getByText('SQLite 全文搜索学习笔记')).toBeInTheDocument();
    expect(screen.getByText(/技术\/数据库/)).toBeInTheDocument();
    expect(screen.getByText('sqlite')).toBeInTheDocument();
    expect(screen.getByText('全文搜索')).toHaveClass('search-highlight');
  });

  it('点击时触发打开回调', async () => {
    const onOpen = vi.fn();
    render(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem note={note} selected basePath="/notes" onOpen={onOpen} />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );

    await userEvent.click(screen.getByText('SQLite 全文搜索学习笔记'));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it('标题被截断时，悬停显示完整标题', async () => {
    const longTitle = '广告行业裁切机自动上下料背扫定位裁切系统技术流程方案说明（超长标题）';
    render(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem
              note={{ ...note, title: longTitle }}
              selected={false}
              basePath="/notes"
              onOpen={() => {}}
            />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );

    await userEvent.hover(screen.getByTestId('note-list-item-title'));
    await waitFor(() => {
      // 标题本体 + tooltip 内容
      expect(screen.getAllByText(longTitle).length).toBeGreaterThan(1);
    });
  });

  it('右键弹出操作菜单（含创建子文档与大纲预览）', async () => {
    render(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem note={note} selected={false} basePath="/notes" onOpen={() => {}} />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );

    await userEvent.pointer({
      target: screen.getByTestId('note-list-item'),
      keys: '[MouseRight]',
    });

    expect(await screen.findByTestId('note-menu-create-child')).toBeInTheDocument();
    expect(screen.getByTestId('note-menu-outline')).toBeInTheDocument();
    expect(screen.getByTestId('note-menu-rename')).toBeInTheDocument();
    expect(screen.getByTestId('note-menu-export')).toBeInTheDocument();
  });

  it('子文档条目提供「升级为顶层文档」，父文档提供折叠箭头', async () => {
    const onToggleCollapse = vi.fn();
    const { rerender } = render(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem
              note={note}
              selected={false}
              basePath="/notes"
              onOpen={() => {}}
              childCount={2}
              onToggleCollapse={onToggleCollapse}
            />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );

    expect(screen.getByTestId('note-tree-toggle')).toHaveAttribute('aria-expanded', 'true');
    await userEvent.click(screen.getByTestId('note-tree-toggle'));
    expect(onToggleCollapse).toHaveBeenCalledTimes(1);

    rerender(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem
              note={{ ...note, parentId: 'note_parent' }}
              selected={false}
              basePath="/notes"
              onOpen={() => {}}
            />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );

    await userEvent.pointer({
      target: screen.getByTestId('note-list-item'),
      keys: '[MouseRight]',
    });
    expect(await screen.findByTestId('note-menu-promote')).toBeInTheDocument();
  });
});
