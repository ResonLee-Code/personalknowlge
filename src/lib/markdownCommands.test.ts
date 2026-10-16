import { describe, expect, it } from 'vitest';

import {
  applyChanges,
  changeIndent,
  insertBlock,
  setHeading,
  toggleLinePrefix,
  wrapSelection,
} from '@/lib/markdownCommands';

/** 跑一个命令并返回「新正文 + 选区」，断言里读起来更像用户看到的结果。 */
function run(state: { value: string; from: number; to: number }, command: (s: typeof state) => ReturnType<typeof wrapSelection>) {
  const result = command(state);
  return { value: applyChanges(state.value, result), from: result.selectionFrom, to: result.selectionTo };
}

describe('markdownCommands（工具栏的编辑命令）', () => {
  it('选中文字加粗；再套一层也不丢内容', () => {
    const state = { value: '这句话要加粗', from: 3, to: 5 };
    expect(run(state, (s) => wrapSelection(s, '**')).value).toBe('这句话**要加**粗');
  });

  it('没选中时插入一对标记，光标停在中间（可以直接打字）', () => {
    const state = { value: '', from: 0, to: 0 };
    const result = run(state, (s) => wrapSelection(s, '**', '**', '加粗文字'));
    expect(result.value).toBe('**加粗文字**');
    expect(result.value.slice(result.from, result.to)).toBe('加粗文字');
  });

  it('列表 / 引用 / 待办按行加前缀；再点一次取消', () => {
    const state = { value: '第一行\n第二行', from: 0, to: 7 };
    const added = run(state, (s) => toggleLinePrefix(s, '- '));
    expect(added.value).toBe('- 第一行\n- 第二行');
    const removed = run({ value: added.value, from: added.from, to: added.to }, (s) =>
      toggleLinePrefix(s, '- '),
    );
    expect(removed.value).toBe('第一行\n第二行');
  });

  it('标题支持从正文一路切到 H3，并会顶掉原来的 #', () => {
    const state = { value: '# 旧标题', from: 2, to: 2 };
    expect(run(state, (s) => setHeading(s, 3)).value).toBe('### 旧标题');
    expect(run({ value: '### 旧标题', from: 2, to: 2 }, (s) => setHeading(s, 0)).value).toBe('旧标题');
  });

  it('缩进 / 反缩进按两个空格，且不会把内容顶出左边界', () => {
    const state = { value: '内容', from: 0, to: 2 };
    const indented = run(state, (s) => changeIndent(s, 1));
    expect(indented.value).toBe('  内容');
    const outdented = run({ value: ' 内容', from: 1, to: 1 }, (s) => changeIndent(s, -1));
    expect(outdented.value).toBe('内容');
  });

  it('插入块级内容：光标在文字中间时**另起一块**，不会塞进段落里', () => {
    const state = { value: '正文', from: 2, to: 2 };
    const result = run(state, (s) => insertBlock(s, '```\n代码\n```'));
    expect(result.value).toBe('正文\n```\n代码\n```\n');
  });

  it('插入块级内容：光标在空行时就地插入', () => {
    const state = { value: '正文\n\n后面', from: 3, to: 3 };
    const result = run(state, (s) => insertBlock(s, '| a | b |'));
    expect(result.value).toBe('正文\n| a | b |\n\n后面');
  });
});
