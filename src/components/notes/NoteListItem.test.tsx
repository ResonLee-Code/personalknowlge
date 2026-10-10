import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it, vi } from 'vitest';

import { NoteListItem } from '@/components/notes/NoteListItem';
import { TooltipProvider } from '@/components/ui/tooltip';
import { useNotesStore } from '@/stores/notesStore';
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
  it('语雀式单行条目：只显示标题，分类 / 标签 / 时间不在列表里重复', () => {
    render(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem note={note} selected={false} basePath="/notes" onOpen={() => {}} />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );

    expect(screen.getByTestId('note-list-item-title')).toHaveTextContent('SQLite 全文搜索学习笔记');
    // 元信息移到预览页与「目录管理」页，列表不再展示
    expect(screen.queryByText(/技术\/数据库/)).not.toBeInTheDocument();
    expect(screen.queryByText('sqlite')).not.toBeInTheDocument();
    expect(screen.queryByText(/分钟前|小时前|天前/)).not.toBeInTheDocument();
  });

  it('搜索态才显示命中摘要并高亮', () => {
    useNotesStore.setState({ keyword: '全文搜索' });
    render(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem note={note} selected={false} basePath="/notes" onOpen={() => {}} />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );

    // 摘要里的命中片段（标题里也有同样的词，因此按摘要容器内的元素断言）
    const highlighted = screen
      .getAllByText('全文搜索')
      .filter((element) => element.className.includes('search-highlight'));
    expect(highlighted.length).toBeGreaterThan(0);
    useNotesStore.setState({ keyword: '' });
  });

  it('置顶与收藏用图标表达，并提供 hover 快捷「新建子文档」', async () => {
    render(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem
              note={{ ...note, pinned: true }}
              selected={false}
              basePath="/notes"
              onOpen={() => {}}
            />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );

    expect(screen.getByTestId('note-pinned-marker')).toHaveAttribute('title', '已置顶');
    expect(screen.getByTestId('note-favorite-marker')).toHaveAttribute('title', '已收藏');
    await userEvent.click(screen.getByTestId('note-quick-child'));
    expect(await screen.findByText('新建子文档')).toBeInTheDocument();
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

    await userEvent.click(screen.getByTestId('note-list-item'));
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

  it('右键弹出操作菜单（含创建子文档等入口）', async () => {
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
    expect(screen.getByTestId('note-menu-rename')).toBeInTheDocument();
    expect(screen.getByTestId('note-menu-export')).toBeInTheDocument();
    expect(screen.getByTestId('note-menu-pin')).toHaveTextContent('置顶');
  });

  it('置顶状态反映在图标与菜单文案上', async () => {
    render(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem
              note={{ ...note, pinned: true }}
              selected={false}
              basePath="/notes"
              onOpen={() => {}}
            />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );

    expect(screen.getByTestId('note-pinned-marker')).toHaveAttribute('title', '已置顶');
    await userEvent.pointer({
      target: screen.getByTestId('note-list-item'),
      keys: '[MouseRight]',
    });
    expect(await screen.findByTestId('note-menu-pin')).toHaveTextContent('取消置顶');
  });

  it('菜单可复制相对路径与标题（对标语雀「复制链接」）', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
    });
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
    await userEvent.click(await screen.findByTestId('note-menu-copy-path'));
    expect(writeText).toHaveBeenCalledWith(note.filePath);

    await userEvent.pointer({
      target: screen.getByTestId('note-list-item'),
      keys: '[MouseRight]',
    });
    await userEvent.click(await screen.findByTestId('note-menu-copy-title'));
    expect(writeText).toHaveBeenCalledWith(note.title);
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

describe('条目菜单：复制到分类入口（C4）', () => {
  it('菜单里有「复制到分类…」并打开对话框', async () => {
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
    await userEvent.click(await screen.findByTestId('note-menu-copy-to'));
    expect(await screen.findByText('复制到分类')).toBeInTheDocument();
    expect(screen.getByTestId('copy-note-confirm')).toBeInTheDocument();
  });
});
