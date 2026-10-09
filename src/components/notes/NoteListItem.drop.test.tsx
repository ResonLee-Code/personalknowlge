import { fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { NoteListItem } from '@/components/notes/NoteListItem';
import { TooltipProvider } from '@/components/ui/tooltip';
import { NOTE_DRAG_TYPE } from '@/lib/dragNotes';
import { useNotesStore } from '@/stores/notesStore';
import type { NoteSummary } from '@/types';

const note: NoteSummary = {
  id: 'note_target',
  title: '目标笔记',
  filePath: '00_Inbox/target.md',
  categoryPath: 'Inbox',
  status: 'active',
  favorite: false,
  wordCount: 10,
  createdAt: '2026-10-01T00:00:00+08:00',
  updatedAt: '2026-10-01T00:00:00+08:00',
  tags: [],
};

/** 极简 DataTransfer：jsdom 不实现拖拽的数据通道 */
function dataTransfer(ids: string[]) {
  const store: Record<string, string> = { [NOTE_DRAG_TYPE]: JSON.stringify(ids) };
  return {
    types: Object.keys(store),
    effectAllowed: 'move',
    dropEffect: 'none',
    setData: (type: string, value: string) => {
      store[type] = value;
    },
    getData: (type: string) => store[type] ?? '',
  } as unknown as DataTransfer;
}

/** 让行高度可预测：80px 高，因此 10px 落在「插到前面」、40px 落在「成为子文档」 */
function stubRowGeometry() {
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: 0,
    top: 0,
    left: 0,
    right: 320,
    bottom: 80,
    width: 320,
    height: 80,
    toJSON: () => ({}),
  } as DOMRect);
}

function renderItem(props: Partial<Parameters<typeof NoteListItem>[0]> = {}) {
  return render(
    <TooltipProvider delayDuration={0}>
      <MemoryRouter>
        <ul>
          <NoteListItem
            note={note}
            selected={false}
            basePath="/notes"
            onOpen={() => {}}
            {...props}
          />
        </ul>
      </MemoryRouter>
    </TooltipProvider>,
  );
}

/**
 * jsdom 没有 DragEvent，`fireEvent.drop(el, { clientY })` 也不会带上坐标，
 * 因此这里手工构造带 clientY 与 dataTransfer 的鼠标事件交给 React 的 drop 处理。
 */
function fireDrag(
  element: HTMLElement,
  type: 'dragover' | 'drop',
  clientY: number,
  transfer: DataTransfer,
) {
  const event = new MouseEvent(type, { bubbles: true, cancelable: true, clientY });
  Object.defineProperty(event, 'dataTransfer', { value: transfer });
  fireEvent(element, event);
}

describe('NoteListItem 拖拽落点', () => {
  beforeEach(() => {
    useNotesStore.setState({ selectionMode: false, selection: [] });
    stubRowGeometry();
  });

  it('行中部落点上报「成为子文档」并带上拖拽的笔记 id', () => {
    const onNoteDragOver = vi.fn();
    const onNoteDrop = vi.fn();
    renderItem({ onNoteDragOver, onNoteDrop });
    const row = screen.getByTestId('note-list-item');

    fireDrag(row, 'dragover', 40, dataTransfer(['note_moved']));
    expect(onNoteDragOver).toHaveBeenLastCalledWith('inside', ['note_moved']);

    fireDrag(row, 'drop', 40, dataTransfer(['note_moved']));
    expect(onNoteDrop).toHaveBeenLastCalledWith('inside', ['note_moved']);
  });

  it('行上下边缘分别上报 before / after（同层排序）', () => {
    const onNoteDrop = vi.fn();
    renderItem({ onNoteDrop });
    const row = screen.getByTestId('note-list-item');

    fireDrag(row, 'drop', 6, dataTransfer(['note_moved']));
    expect(onNoteDrop).toHaveBeenLastCalledWith('before', ['note_moved']);

    fireDrag(row, 'drop', 74, dataTransfer(['note_moved']));
    expect(onNoteDrop).toHaveBeenLastCalledWith('after', ['note_moved']);
  });

  it('非笔记拖拽（例如拖文件进来）不拦截事件', () => {
    const onNoteDrop = vi.fn();
    renderItem({ onNoteDrop });
    const row = screen.getByTestId('note-list-item');
    const foreign = {
      types: ['Files'],
      dropEffect: 'none',
      setData: () => {},
      getData: () => '',
    } as unknown as DataTransfer;

    fireDrag(row, 'drop', 40, foreign);
    expect(onNoteDrop).not.toHaveBeenCalled();
  });

  it('按 dropHint 渲染蓝色横线 / 蓝色框（对标语雀）', () => {
    const { rerender } = renderItem({ dropHint: 'before' });
    expect(screen.getByTestId('note-drop-line')).toBeInTheDocument();

    rerender(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter>
          <ul>
            <NoteListItem
              note={note}
              selected={false}
              basePath="/notes"
              onOpen={() => {}}
              dropHint="inside"
            />
          </ul>
        </MemoryRouter>
      </TooltipProvider>,
    );
    expect(screen.queryByTestId('note-drop-line')).not.toBeInTheDocument();
    expect(screen.getByTestId('note-list-item').className).toContain('ring-2');
  });
});
