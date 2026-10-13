import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { FailureDetail, failureDetailText } from '@/components/ui/failure-detail';

describe('FailureDetail（Track F2：失败原因要"点得开"）', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('点开「详情」能看到**完整**原因（界面上的摘要可以是截断的，这里不能）', async () => {
    const detail = `${'很长的失败原因。'.repeat(20)}末尾的字段在这里。`;
    render(<FailureDetail summary="保存失败" detail={detail} testId="demo" />);

    expect(screen.getByTestId('demo-open')).toHaveTextContent('详情');
    await userEvent.click(screen.getByTestId('demo-open'));

    const full = await screen.findByTestId('demo-full');
    expect(full).toHaveTextContent('末尾的字段在这里。');
    expect(full.textContent).toHaveLength(detail.length);
  });

  it('错误码与补充信息（文件路径等）也一并显示', async () => {
    render(
      <FailureDetail
        summary="保存失败"
        detail="文件已被其他程序修改。"
        code="NOTE_CONFLICT"
        extra={['文件：00_Inbox/示例.md', null, '缺少的：file_mtime']}
        testId="demo"
      />,
    );
    await userEvent.click(screen.getByTestId('demo-open'));

    expect(await screen.findByTestId('demo-code')).toHaveTextContent('NOTE_CONFLICT');
    const extra = screen.getByTestId('demo-extra');
    expect(extra).toHaveTextContent('00_Inbox/示例.md');
    expect(extra).toHaveTextContent('缺少的：file_mtime');
  });

  it('没有可展开的内容时不摆一个点了没反应的按钮', () => {
    render(<FailureDetail summary="保存失败" detail="   " testId="demo" />);
    expect(screen.queryByTestId('demo-open')).toBeNull();
  });

  it('「复制详情」把摘要、原因、错误码与补充信息一起复制（方便直接贴进报障）', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText },
      configurable: true,
    });

    render(
      <FailureDetail
        summary="保存失败"
        detail="文件已被其他程序修改。"
        code="NOTE_CONFLICT"
        extra={['文件：a.md']}
        testId="demo"
      />,
    );
    await userEvent.click(screen.getByTestId('demo-open'));
    await userEvent.click(screen.getByRole('button', { name: /复制详情/ }));

    expect(writeText).toHaveBeenCalledWith(
      ['保存失败 [NOTE_CONFLICT]', '文件已被其他程序修改。', '文件：a.md'].join('\n'),
    );
    expect(await screen.findByRole('button', { name: /已复制/ })).toBeInTheDocument();
  });

  it('剪贴板不可用时不抛错（不能变成新的失败源）', async () => {
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: vi.fn().mockRejectedValue(new Error('not allowed')) },
      configurable: true,
    });
    render(<FailureDetail summary="失败" detail="原因" testId="demo" />);
    await userEvent.click(screen.getByTestId('demo-open'));
    await userEvent.click(screen.getByRole('button', { name: /复制详情/ }));
    // 还在，没有崩，也没假装复制成功
    expect(screen.getByRole('button', { name: /复制详情/ })).toBeInTheDocument();
  });
});

describe('failureDetailText', () => {
  it('只拼有内容的部分，不留空行', () => {
    expect(failureDetailText({ summary: '失败', detail: null, code: null, extra: [null, ' '] })).toBe(
      '失败',
    );
    expect(failureDetailText({ summary: '', detail: '原因', code: 'CODE' })).toBe('[CODE]\n原因');
  });
});
