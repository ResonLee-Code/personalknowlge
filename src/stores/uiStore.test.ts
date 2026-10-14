import { beforeEach, describe, expect, it } from 'vitest';

import { LAYOUT_DEFAULTS, useUiStore } from '@/stores/uiStore';

describe('uiStore 布局取值', () => {
  beforeEach(() => {
    useUiStore.setState({
      sidebarWidth: LAYOUT_DEFAULTS.sidebarWidth,
      listWidth: LAYOUT_DEFAULTS.listWidth,
      editorSplitRatio: LAYOUT_DEFAULTS.editorSplitRatio,
      editorMode: 'split',
      // 大纲相关：位置与「这一栏显示什么」是两件事，逐条用例都要从默认值开始
      outlinePlacement: 'list',
      listPaneMode: 'list',
      autoOutlineOnSelect: false,
    });
  });

  it('像素宽度取整（Retina 下 clientX 可能是小数，后端按 u32 存储）', () => {
    useUiStore.getState().setSidebarWidth(320.5);
    expect(useUiStore.getState().sidebarWidth).toBe(321);

    useUiStore.getState().setListWidth(400.4);
    expect(useUiStore.getState().listWidth).toBe(400);
  });

  it('宽度会夹取到允许区间', () => {
    useUiStore.getState().setSidebarWidth(50);
    expect(useUiStore.getState().sidebarWidth).toBe(180);

    useUiStore.getState().setListWidth(9999);
    expect(useUiStore.getState().listWidth).toBe(560);
  });

  it('分屏比例保留三位小数并夹取到 0.3 ~ 0.7', () => {
    useUiStore.getState().setEditorSplitRatio(0.51234);
    expect(useUiStore.getState().editorSplitRatio).toBe(0.512);

    useUiStore.getState().setEditorSplitRatio(0.95);
    expect(useUiStore.getState().editorSplitRatio).toBe(0.7);

    useUiStore.getState().setEditorSplitRatio(0.01);
    expect(useUiStore.getState().editorSplitRatio).toBe(0.3);
  });

  it('applyPreferences 只接受已知的视图模式与主题', () => {
    useUiStore.getState().applyPreferences({
      editorMode: '不存在的模式',
      theme: 'rainbow',
      sidebarWidth: 300.7,
      editorSplitRatio: 0.6,
    });
    const state = useUiStore.getState();
    expect(state.editorMode).toBe('split'); // 未识别，保持原值
    expect(state.theme).toBe('light');
    expect(state.sidebarWidth).toBe(301);
    expect(state.editorSplitRatio).toBe(0.6);
  });

  it('大纲默认与笔记列表共用一栏（V1.3.0 布局调整）', () => {
    const state = useUiStore.getState();
    expect(state.outlinePlacement).toBe('list');
    expect(state.listPaneMode).toBe('list');
    expect(state.autoOutlineOnSelect).toBe(false);
  });

  it('挪到正文右侧时列表栏还给列表；挪回列表栏时这一栏直接显示大纲', () => {
    // 挪到右侧：大纲有了新家，列表栏不该再显示大纲（否则用户看不到列表）
    useUiStore.getState().setListPaneMode('outline');
    useUiStore.getState().setOutlinePlacement('side');
    expect(useUiStore.getState().listPaneMode).toBe('list');

    // 挪回列表栏：这一栏就应该立刻显示大纲，而不是让用户觉得大纲凭空消失了
    useUiStore.getState().setOutlinePlacement('list');
    expect(useUiStore.getState().listPaneMode).toBe('outline');
  });

  it('applyPreferences 只接受已知的大纲位置', () => {
    useUiStore.getState().applyPreferences({ outlinePlacement: 'top' });
    expect(useUiStore.getState().outlinePlacement).toBe('list'); // 未识别，保持原值

    useUiStore.getState().applyPreferences({
      outlinePlacement: 'side',
      autoOutlineOnSelect: true,
    });
    expect(useUiStore.getState().outlinePlacement).toBe('side');
    expect(useUiStore.getState().autoOutlineOnSelect).toBe(true);
  });
});
