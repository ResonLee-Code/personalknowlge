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

  /**
   * 回归（2026-10-11 真机验收，Track B1）：**保存失败的原因不能只藏在悬停提示里**。
   * 现场是「头部只显示一个红的『保存失败』，没有任何原因」；实际原因存在于 `title`
   * （原生 tooltip），但没人会去悬停——和上面「索引待重建」那条是同一类问题。
   */
  it('保存失败时原因必须显式可见（不靠悬停）', () => {
    const reason = '文件已被其他程序修改，请选择重新加载或覆盖保存。';
    render(<SaveStatusBadge status="error" detail={reason} />);
    const badge = screen.getByTestId('save-status-badge');
    expect(badge).toHaveTextContent('保存失败');
    expect(badge).toHaveTextContent('文件已被其他程序修改');
    expect(badge).toHaveAttribute('title', reason);
  });

  it('没有原因时不拖一个空冒号', () => {
    render(<SaveStatusBadge status="error" />);
    expect(screen.getByTestId('save-status-badge')).toHaveTextContent(/^保存失败$/);
  });
});
