import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AttachmentsPage } from '@/features/attachments/AttachmentsPage';
import { attachmentService } from '@/services/attachmentService';

vi.mock('@/services/attachmentService', () => ({
  attachmentService: { list: vi.fn(), remove: vi.fn(), references: vi.fn() },
}));

const list = vi.mocked(attachmentService.list);
const remove = vi.mocked(attachmentService.remove);

function entry(overrides: Record<string, unknown> = {}) {
  return {
    relativePath: 'attachments/2026-10/shot-1.png',
    fileName: 'shot-1.png',
    month: '2026-10',
    bytes: 2048,
    modifiedAt: '2026-10-07T10:00:00+08:00',
    references: [],
    ...overrides,
  };
}

describe('AttachmentsPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.spyOn(window, 'confirm').mockReturnValue(true);
  });

  it('没有附件时给出引导', async () => {
    list.mockResolvedValue({ items: [], totalBytes: 0, referencedCount: 0 });
    render(
      <MemoryRouter>
        <AttachmentsPage />
      </MemoryRouter>,
    );
    expect(await screen.findByText('还没有附件')).toBeInTheDocument();
  });

  it('按月份分组展示，并标记引用与未引用', async () => {
    list.mockResolvedValue({
      items: [
        entry({
          relativePath: 'attachments/2026-10/shot-1.png',
          fileName: 'shot-1.png',
          references: [{ noteId: 'note_a', title: '索引设计笔记' }],
        }),
        entry({
          relativePath: 'attachments/2026-09/shot-2.png',
          fileName: 'shot-2.png',
          month: '2026-09',
        }),
      ],
      totalBytes: 4096,
      referencedCount: 1,
    });

    render(
      <MemoryRouter>
        <AttachmentsPage />
      </MemoryRouter>,
    );

    expect(await screen.findByText(/2026 年 10 月/)).toBeInTheDocument();
    expect(screen.getByText(/2026 年 09 月/)).toBeInTheDocument();
    expect(screen.getByText('索引设计笔记')).toBeInTheDocument();
    expect(screen.getByText('未引用')).toBeInTheDocument();
    expect(screen.getByText(/被引用 1 · 未引用 1/)).toBeInTheDocument();
  });

  it('删除附件后重新拉取清单', async () => {
    list.mockResolvedValue({
      items: [entry()],
      totalBytes: 2048,
      referencedCount: 0,
    });
    remove.mockResolvedValue(2048);

    render(
      <MemoryRouter>
        <AttachmentsPage />
      </MemoryRouter>,
    );

    await screen.findByTestId('attachment-row');
    await userEvent.click(screen.getByRole('button', { name: '删除附件：shot-1.png' }));

    await waitFor(() =>
      expect(remove).toHaveBeenCalledWith('attachments/2026-10/shot-1.png'),
    );
    await waitFor(() => expect(list.mock.calls.length).toBeGreaterThan(1));
  });

  /** F3：孤儿（未被任何笔记引用）是清理目标，要能单独筛出来看，并显示占多少空间。 */
  it('可以只看未引用附件（清理时用）', async () => {
    list.mockResolvedValue({
      items: [
        entry({
          relativePath: 'attachments/2026-10/used.png',
          fileName: 'used.png',
          references: [{ noteId: 'note_a', title: '在用的笔记' }],
        }),
        entry({
          relativePath: 'attachments/2026-10/orphan.png',
          fileName: 'orphan.png',
          bytes: 4096,
        }),
      ],
      totalBytes: 6144,
      referencedCount: 1,
    });

    render(
      <MemoryRouter>
        <AttachmentsPage />
      </MemoryRouter>,
    );

    const toggle = await screen.findByTestId('attachments-only-orphans');
    expect(toggle).toHaveTextContent('只看未引用（1 个 · 4.0 KB）');
    expect(screen.getAllByTestId('attachment-row')).toHaveLength(2);

    await userEvent.click(toggle);
    expect(toggle).toHaveAttribute('aria-pressed', 'true');
    const rows = screen.getAllByTestId('attachment-row');
    expect(rows).toHaveLength(1);
    expect(rows[0]).toHaveTextContent('orphan.png');

    // 再点一次恢复全部——筛选不该是"一去不回"的单向动作
    await userEvent.click(toggle);
    expect(screen.getAllByTestId('attachment-row')).toHaveLength(2);
  });
});
