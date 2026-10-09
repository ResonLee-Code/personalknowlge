/**
 * 极简行级 diff（LCS），用于「历史版本对比」（对标语雀的「对比任意两个版本」）。
 *
 * 不引第三方依赖：版本对比只需要「哪些行增删了」，LCS 足够，
 * 并且行数上限（各 4000 行）有保护，避免大文档下 O(n·m) 卡死界面。
 */

export type DiffKind = 'same' | 'add' | 'remove';

export interface DiffRow {
  kind: DiffKind;
  text: string;
  /** 左侧（基准版本）行号，1 起；新增行为 null */
  leftNo: number | null;
  /** 右侧（目标版本）行号，1 起；删除行为 null */
  rightNo: number | null;
}

export interface DiffStats {
  added: number;
  removed: number;
  unchanged: number;
}

const MAX_LINES = 4000;

function toLines(text: string): string[] {
  const normalized = text.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
  // 空文档 = 0 行（否则会被当成「一个空行」，diff 里凭空多一行删除）
  if (normalized === '') return [];
  const lines = normalized.split('\n');
  // 末尾换行会切出一个空串，去掉它避免凭空多一行
  if (lines.length > 1 && lines[lines.length - 1] === '') lines.pop();
  return lines.slice(0, MAX_LINES);
}

/**
 * 计算两个文本的行级差异。
 * `left` 是基准版本（旧），`right` 是目标版本（新）。
 */
export function diffLines(left: string, right: string): DiffRow[] {
  const a = toLines(left);
  const b = toLines(right);
  const n = a.length;
  const m = b.length;

  // LCS 长度表（(n+1) x (m+1)）
  const table: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i -= 1) {
    for (let j = m - 1; j >= 0; j -= 1) {
      table[i]![j] = a[i] === b[j] ? table[i + 1]![j + 1]! + 1 : Math.max(table[i + 1]![j]!, table[i]![j + 1]!);
    }
  }

  const rows: DiffRow[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      rows.push({ kind: 'same', text: a[i]!, leftNo: i + 1, rightNo: j + 1 });
      i += 1;
      j += 1;
    } else if (table[i + 1]![j]! >= table[i]![j + 1]!) {
      rows.push({ kind: 'remove', text: a[i]!, leftNo: i + 1, rightNo: null });
      i += 1;
    } else {
      rows.push({ kind: 'add', text: b[j]!, leftNo: null, rightNo: j + 1 });
      j += 1;
    }
  }
  while (i < n) {
    rows.push({ kind: 'remove', text: a[i]!, leftNo: i + 1, rightNo: null });
    i += 1;
  }
  while (j < m) {
    rows.push({ kind: 'add', text: b[j]!, leftNo: null, rightNo: j + 1 });
    j += 1;
  }
  return rows;
}

/** 统计增删行数（用于「+3 −1」这样的摘要）。 */
export function diffStats(rows: DiffRow[]): DiffStats {
  return rows.reduce<DiffStats>(
    (acc, row) => {
      if (row.kind === 'add') acc.added += 1;
      else if (row.kind === 'remove') acc.removed += 1;
      else acc.unchanged += 1;
      return acc;
    },
    { added: 0, removed: 0, unchanged: 0 },
  );
}
