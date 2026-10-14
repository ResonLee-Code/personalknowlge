import { create } from 'zustand';

import type { OutlineHeading } from '@/lib/markdown';

/**
 * 大纲的"发布 / 订阅"（2026-10-14 布局调整）。
 *
 * 背景：大纲原来只活在本页编辑器内部（固定在正文右侧一列）。为了把横向空间还给正文，
 * 现在大纲默认显示在**笔记列表那一栏**——那一栏由 `NotesSection` 渲染，拿不到编辑器内部的
 * headings 与跳转函数。与其把整棵组件树重构，不如让编辑器**发布**一次大纲，列表栏**订阅**：
 *
 * - 编辑器持有唯一事实来源（它才知道正文解析出的标题与滚动位置）；
 * - 列表栏只负责显示与转发点击；
 * - 跳转函数用 ref 保持稳定（否则每次渲染都会让订阅方重渲染）。
 *
 * 只活在内存里：这是纯 UI 状态，不落库（切笔记/刷新后由编辑器重新发布）。
 */
interface OutlineState {
  noteId: string | null;
  title: string;
  headings: OutlineHeading[];
  activeOffset: number | null;
  jump: ((offset: number) => void) | null;
  publish: (payload: {
    noteId: string;
    title: string;
    headings: OutlineHeading[];
    activeOffset: number | null;
    jump: (offset: number) => void;
  }) => void;
  /** 编辑器卸载时撤销发布；带上 noteId 是为了不误清另一篇笔记刚发布的内容 */
  clear: (noteId: string) => void;
}

export const useOutlineStore = create<OutlineState>((set, get) => ({
  noteId: null,
  title: '',
  headings: [],
  activeOffset: null,
  jump: null,
  publish: ({ noteId, title, headings, activeOffset, jump }) =>
    set({ noteId, title, headings, activeOffset, jump }),
  clear: (noteId) => {
    if (get().noteId !== noteId) return;
    set({ noteId: null, title: '', headings: [], activeOffset: null, jump: null });
  },
}));
