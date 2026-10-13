import { beforeEach, describe, expect, it, vi } from 'vitest';

import { logAppFailure } from '@/lib/appLog';
import { call } from '@/services/api';

vi.mock('@/services/api', () => ({ call: vi.fn() }));

/**
 * Track B3 / E4：界面上看得见的失败，必须能在应用日志里查到。
 * 这个封装是唯一入口，所以三条纪律都在这里钉住：
 * 只记原因、永不抛错、按 scope 归类。
 */
describe('logAppFailure', () => {
  beforeEach(() => {
    vi.mocked(call).mockReset();
  });

  it('把原因写进应用日志（带 scope 与级别）', async () => {
    vi.mocked(call).mockResolvedValue(undefined as never);

    await logAppFailure('note_save', { code: 'NOTE_CONFLICT', message: '文件已被其他程序修改' });

    expect(call).toHaveBeenCalledWith('app_log', {
      level: 'error',
      scope: 'note_save',
      message: '文件已被其他程序修改',
    });
  });

  it('级别可降为 warn（例如扫描里的跳过文件）', async () => {
    vi.mocked(call).mockResolvedValue(undefined as never);
    await logAppFailure('scan', new Error('跳过 2 个文件'), 'warn');
    expect(call).toHaveBeenCalledWith('app_log', {
      level: 'warn',
      scope: 'scan',
      message: '跳过 2 个文件',
    });
  });

  it('写日志本身失败时不能抛出去（排障通道不能变成新故障源）', async () => {
    vi.mocked(call).mockRejectedValue(new Error('IPC 不通'));
    await expect(logAppFailure('export', new Error('磁盘已满'))).resolves.toBeUndefined();
  });

  it('空原因不写（避免日志里全是空条目）', async () => {
    vi.mocked(call).mockResolvedValue(undefined as never);
    await logAppFailure('export', { message: '   ' });
    expect(call).not.toHaveBeenCalled();
  });
});
