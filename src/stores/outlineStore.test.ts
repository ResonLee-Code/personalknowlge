import { beforeEach, describe, expect, it } from 'vitest';

import { useOutlineStore } from '@/stores/outlineStore';

const heading = (offset: number, level: 1 | 2, text: string) => ({ offset, level, text });

describe('outlineStore（编辑 → 列表栏的大纲发布）', () => {
  beforeEach(() => {
    useOutlineStore.setState({
      noteId: null,
      title: '',
      headings: [],
      activeOffset: null,
      jump: null,
    });
  });

  it('编辑器发布后，订阅方能拿到标题、当前高亮与跳转函数', () => {
    const jump = (offset: number) => void offset;
    useOutlineStore.getState().publish({
      noteId: 'note_1',
      title: '示例笔记',
      headings: [heading(0, 1, '全文搜索')],
      activeOffset: 0,
      jump,
    });

    const state = useOutlineStore.getState();
    expect(state.noteId).toBe('note_1');
    expect(state.title).toBe('示例笔记');
    expect(state.headings).toHaveLength(1);
    expect(state.activeOffset).toBe(0);
    expect(state.jump).toBe(jump);
  });

  it('只有"还是我"的时候才允许撤销发布（切笔记时不误清新人）', () => {
    const publish = useOutlineStore.getState().publish;
    publish({
      noteId: 'note_old',
      title: '旧笔记',
      headings: [],
      activeOffset: null,
      jump: () => {},
    });
    publish({
      noteId: 'note_new',
      title: '新笔记',
      headings: [heading(0, 1, '标题')],
      activeOffset: null,
      jump: () => {},
    });

    // 旧编辑器卸载时的清理不该把新笔记的大纲清掉
    useOutlineStore.getState().clear('note_old');
    expect(useOutlineStore.getState().noteId).toBe('note_new');
    expect(useOutlineStore.getState().headings).toHaveLength(1);

    useOutlineStore.getState().clear('note_new');
    expect(useOutlineStore.getState().noteId).toBeNull();
    expect(useOutlineStore.getState().headings).toEqual([]);
  });
});
