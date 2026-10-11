import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

import { toast } from 'sonner';

import { reportAction } from '@/lib/actionFeedback';

describe('reportAction（失败不能报成功）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('成功时报成功提示', () => {
    reportAction(true, '已收藏', null);
    expect(toast.success).toHaveBeenCalledWith('已收藏');
    expect(toast.error).not.toHaveBeenCalled();
  });

  it('失败时报错并带上 store 里的原因', () => {
    reportAction(false, '已收藏', '笔记不存在或已被删除。', '收藏失败');
    expect(toast.success).not.toHaveBeenCalled();
    expect(toast.error).toHaveBeenCalledWith('收藏失败：笔记不存在或已被删除。');
  });

  it('失败且没有原因时用兜底文案', () => {
    reportAction(false, '已永久删除', '   ', '删除失败');
    expect(toast.error).toHaveBeenCalledWith('删除失败');
  });
});
