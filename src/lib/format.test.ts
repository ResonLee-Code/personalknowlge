import { describe, expect, it } from 'vitest';

import { errorMessage, formatBytes, formatDateTime, formatRelative, parseDate } from './format';

describe('时间格式化', () => {
  it('解析 ISO 8601 字符串', () => {
    const date = parseDate('2026-09-23T12:00:00+08:00');
    expect(date).not.toBeNull();
    expect(date?.getFullYear()).toBe(2026);
    expect(parseDate(null)).toBeNull();
    expect(parseDate('不是时间')).toBeNull();
  });

  it('格式化日期时间', () => {
    expect(formatDateTime('2026-09-23T12:00:00+08:00')).toMatch(/2026-09-23/);
    expect(formatDateTime(null)).toBe('—');
    expect(typeof formatRelative('2026-09-23T12:00:00+08:00')).toBe('string');
  });
});

describe('formatBytes', () => {
  it('按单位换算', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(1024)).toBe('1.0 KB');
    expect(formatBytes(1024 * 1024 * 3)).toBe('3.0 MB');
  });
});

describe('errorMessage', () => {
  it('优先使用后端 message', () => {
    expect(errorMessage({ code: 'NOTE_NOT_FOUND', message: '笔记不存在' })).toBe('笔记不存在');
    expect(errorMessage('字符串错误')).toBe('字符串错误');
    expect(errorMessage(undefined)).toBe('操作失败，请稍后重试。');
  });
});
