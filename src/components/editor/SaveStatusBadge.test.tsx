import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { SaveStatusBadge } from '@/components/editor/SaveStatusBadge';

describe('SaveStatusBadge', () => {
  it('正常保存显示「已保存」，不提示任何降级', () => {
    render(<SaveStatusBadge status="saved" />);
    const badge = screen.getByTestId('save-status-badge');
    expect(badge).toHaveTextContent('已保存');
    expect(badge.getAttribute('data-degraded')).toBeNull();
  });

  /**
   * 回归：`note_save` 成功但**索引更新失败**时会带回 `indexWarning`
   * （正文已写盘、检索里却找不到这篇）。以前徽标和正常保存一模一样，
   * 原因只藏在悬停提示里——用户不会去悬停。
   */
  it('保存成功但索引没跟上时必须显式可见（不是只藏在 tooltip）', () => {
    render(<SaveStatusBadge status="saved" detail="建立索引失败：磁盘已满" />);
    const badge = screen.getByTestId('save-status-badge');
    expect(badge).toHaveTextContent('索引待重建');
    expect(badge).toHaveAttribute('data-degraded', 'true');
    expect(badge).toHaveAttribute('title', '建立索引失败：磁盘已满');
  });

  it('空白原因不算降级（避免噪声）', () => {
    render(<SaveStatusBadge status="saved" detail="   " />);
    expect(screen.getByTestId('save-status-badge')).toHaveTextContent('已保存');
  });

  it('保存失败仍按失败展示', () => {
    render(<SaveStatusBadge status="error" detail="磁盘只读" />);
    const badge = screen.getByTestId('save-status-badge');
    expect(badge).toHaveTextContent('保存失败');
    expect(badge.getAttribute('data-degraded')).toBeNull();
  });
});
