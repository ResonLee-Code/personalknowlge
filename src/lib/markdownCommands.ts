/**
 * Markdown 文本编辑命令（2026-10-16）。
 *
 * 编辑器工具栏（加粗 / 标题 / 列表…）与插入菜单都靠这里：**纯函数**，输入输出都是
 * `{ value, from, to }`，不碰 CodeMirror —— 这样命令本身可以单元测试，
 * 调用方只负责把结果 dispatch 进编辑器（见 `applyEdit`）。
 */

export interface EditState {
  /** 整篇正文 */
  value: string;
  /** 选区起点（码点下标） */
  from: number;
  /** 选区终点 */
  to: number;
}

export interface EditResult {
  /**
   * **最小改动**（CodeMirror 原样吃这套结构）。
   *
   * 为什么不直接返回"整篇新正文"：整篇替换会让编辑器丢掉视口位置（长文档里会跳回顶部），
   * 也会把一次按键记成一整篇的改动。这里每个命令都只改它真正碰过的那一段。
   */
  changes: Array<{ from: number; to: number; insert: string }>;
  /** 执行后的选区（用于把光标放回合理位置） */
  selectionFrom: number;
  selectionTo: number;
}

/** 把改动应用到一段文本上（给单元测试与"预览结果"用；编辑器那边直接吃 changes）。 */
export function applyChanges(value: string, result: EditResult): string {
  return [...result.changes]
    .sort((left, right) => right.from - left.from)
    .reduce((acc, change) => `${acc.slice(0, change.from)}${change.insert}${acc.slice(change.to)}`, value);
}

/**
 * 用一对标记包住选区；**没有选区时**插入一对标记并把光标放在中间
 * （这样"点一下加粗"可以直接开始打字）。
 */
export function wrapSelection(
  state: EditState,
  before: string,
  after: string = before,
  placeholder = '',
): EditResult {
  const { value, from, to } = state;
  const selected = value.slice(from, to);
  const body = selected || placeholder;
  const start = from + before.length;
  return {
    changes: [{ from, to, insert: `${before}${body}${after}` }],
    selectionFrom: start,
    selectionTo: start + body.length,
  };
}

/**
 * 把选区覆盖的每一行做"加减前缀"：
 * - 全都已经有该前缀 → **去掉**（再点一次取消，符合直觉）；
 * - 否则给每行加上（已经有的不重复加）。
 */
export function toggleLinePrefix(state: EditState, prefix: string): EditResult {
  const { value, from, to } = state;
  const lineStart = value.lastIndexOf('\n', Math.max(0, from - 1)) + 1;
  const lineEndRaw = value.indexOf('\n', to);
  const lineEnd = lineEndRaw === -1 ? value.length : lineEndRaw;
  const block = value.slice(lineStart, lineEnd);
  const lines = block.split('\n');
  const allPrefixed = lines.every((line) => line.trim() === '' || line.startsWith(prefix));
  const nextLines = lines.map((line) => {
    if (line.trim() === '') return line;
    if (allPrefixed) return line.startsWith(prefix) ? line.slice(prefix.length) : line;
    return line.startsWith(prefix) ? line : `${prefix}${line}`;
  });
  const nextBlock = nextLines.join('\n');
  return {
    changes: [{ from: lineStart, to: lineEnd, insert: nextBlock }],
    selectionFrom: lineStart,
    selectionTo: lineStart + nextBlock.length,
  };
}

/**
 * 设置 / 取消标题级别：`level = 0` 表示"正文"。
 * 只作用于选区覆盖的每一行（去掉已有的 `#…` 再按需加新的）。
 */
export function setHeading(state: EditState, level: 0 | 1 | 2 | 3): EditResult {
  const { value, from, to } = state;
  const lineStart = value.lastIndexOf('\n', Math.max(0, from - 1)) + 1;
  const lineEndRaw = value.indexOf('\n', to);
  const lineEnd = lineEndRaw === -1 ? value.length : lineEndRaw;
  const block = value.slice(lineStart, lineEnd);
  const prefix = level === 0 ? '' : `${'#'.repeat(level)} `;
  const nextBlock = block
    .split('\n')
    .map((line) => `${prefix}${line.replace(/^#{1,6}\s+/, '')}`)
    .join('\n');
  return {
    changes: [{ from: lineStart, to: lineEnd, insert: nextBlock }],
    selectionFrom: lineStart,
    selectionTo: lineStart + nextBlock.length,
  };
}

/** 整块缩进 / 反缩进（每行加减两个空格）；缩进时为"减少到没有"为止。 */
export function changeIndent(state: EditState, delta: 1 | -1): EditResult {
  const { value, from, to } = state;
  const indent = '  ';
  const lineStart = value.lastIndexOf('\n', Math.max(0, from - 1)) + 1;
  const lineEndRaw = value.indexOf('\n', to);
  const lineEnd = lineEndRaw === -1 ? value.length : lineEndRaw;
  const block = value.slice(lineStart, lineEnd);
  const nextBlock = block
    .split('\n')
    .map((line) => {
      if (delta === 1) return line.trim() === '' ? line : `${indent}${line}`;
      if (line.startsWith(indent)) return line.slice(indent.length);
      if (line.startsWith(' ')) return line.slice(1);
      return line;
    })
    .join('\n');
  return {
    changes: [{ from: lineStart, to: lineEnd, insert: nextBlock }],
    selectionFrom: lineStart,
    selectionTo: lineStart + nextBlock.length,
  };
}

/**
 * 在光标处插入一段**块级**内容（表格 / 代码块 / 图…）：
 * 前后各补一个换行，保证它是独立的一块，不会粘在上一行文字后面。
 */
export function insertBlock(state: EditState, block: string): EditResult {
  const { value, from, to } = state;
  const lineStart = value.lastIndexOf('\n', Math.max(0, from - 1)) + 1;
  const lineEndRaw = value.indexOf('\n', to);
  const lineEnd = lineEndRaw === -1 ? value.length : lineEndRaw;
  const lineContent = value.slice(lineStart, lineEnd);
  /**
   * 插入位置：
   * - 光标所在行**是空行** → 就地插（正是"另起一块"的语境）；
   * - 否则 → 插到**本行之后**（2026-10-16 修：直接插在光标处会把表格塞进段落中间，
   *   甚至塞进 `**加粗**` 的标记里——实测就是这么坏的）。
   */
  const at = lineContent.trim() === '' ? lineStart : lineEnd;
  const needsLeadingBreak = at > 0 && value[at - 1] !== '\n';
  const text = `${needsLeadingBreak ? '\n' : ''}${block}\n`;
  const caret = at + text.length;
  return { changes: [{ from: at, to: at, insert: text }], selectionFrom: caret, selectionTo: caret };
}

/** 插入菜单里的常用块（模板集中放这里，菜单与测试共用一份）。 */
export const INSERT_BLOCKS = {
  table: '| 列 1 | 列 2 |\n| --- | --- |\n| 内容 | 内容 |',
  code: '```\n在这里写代码\n```',
  math: '$$\nE = mc^2\n$$',
  flowchart: '```mermaid\nflowchart LR\n  A[开始] --> B{判断}\n  B -->|是| C[结束]\n  B -->|否| A\n```',
  sequence: '```mermaid\nsequenceDiagram\n  参与者A->>参与者B: 发消息\n  参与者B-->>参与者A: 回消息\n```',
  classDiagram: '```mermaid\nclassDiagram\n  class 笔记 {\n    +标题\n    +正文\n  }\n  笔记 --> 标签\n```',
  mindmap: '```mermaid\nmindmap\n  root((主题))\n    分支A\n    分支B\n```',
  quote: '> 引用内容',
  todo: '- [ ] 待办事项',
  divider: '---',
} as const;
