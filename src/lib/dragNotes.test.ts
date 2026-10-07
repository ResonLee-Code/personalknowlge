import { describe, expect, it } from 'vitest';

import { isNoteDrag, NOTE_DRAG_TYPE, readNoteDragIds, setNoteDragData } from '@/lib/dragNotes';

function fakeTransfer(): DataTransfer {
  const store = new Map<string, string>();
  const transfer: {
    effectAllowed: string;
    types: string[];
    setData: (type: string, value: string) => void;
    getData: (type: string) => string;
  } & Record<string, unknown> = {
    effectAllowed: 'none',
    types: [],
    setData(type: string, value: string) {
      store.set(type, value);
      transfer.types.push(type);
    },
    getData(type: string) {
      return store.get(type) ?? '';
    },
  };
  return transfer as unknown as DataTransfer;
}

describe('dragNotes', () => {
  it('写入后可以读回（多选批量）', () => {
    const transfer = fakeTransfer();
    setNoteDragData(transfer, ['a', 'b'], '两篇笔记');
    expect(transfer.getData('text/plain')).toBe('两篇笔记');
    expect(readNoteDragIds(transfer)).toEqual(['a', 'b']);
    expect(isNoteDrag(transfer)).toBe(true);
  });

  it('空列表不写入', () => {
    const transfer = fakeTransfer();
    setNoteDragData(transfer, [], 'x');
    expect(readNoteDragIds(transfer)).toEqual([]);
  });

  it('非笔记拖拽与损坏数据返回空', () => {
    const transfer = fakeTransfer();
    expect(readNoteDragIds(transfer)).toEqual([]);
    expect(isNoteDrag(transfer)).toBe(false);
    transfer.setData(NOTE_DRAG_TYPE, '不是 JSON');
    expect(readNoteDragIds(transfer)).toEqual([]);
    expect(isNoteDrag(transfer)).toBe(true);
  });
});
