import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { HistoryDialog } from '@/components/editor/HistoryDialog';
import { noteService } from '@/services/noteService';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

const entries = [
  { entryId: 'e2', savedAt: '2026-10-10T10:00:00+08:00', bytes: 10, title: '笔记', preview: '第二版' },
  { entryId: 'e1', savedAt: '2026-10-10T09:00:00+08:00', bytes: 8, title: '笔记', preview: '第一版' },
];

function mockApi() {
  vi.spyOn(noteService, 'historyList').mockResolvedValue(entries);
  vi.spyOn(noteService, 'historyRead').mockImplementation(async (_noteId, entryId) => ({
    entryId,
    content: '',
    body: entryId === 'e1' ? '第一版正文\n共同行' : '第二版正文\n共同行',
  }));
}

describe('HistoryDialog', () => {
  it('列出快照并与当前版本对比，标出增删行', async () => {
    mockApi();
    render(
      <HistoryDialog
        noteId="note_1"
        currentContent={'第三版正文\n共同行'}
        onClose={() => {}}
      />,
    );

    expect(await screen.findAllByTestId('history-entry')).toHaveLength(2);
    // 默认对比：最新快照（第二版）→ 当前版本（第三版）
    await waitFor(() => expect(screen.getAllByTestId('diff-row-add').length).toBeGreaterThan(0));
    expect(screen.getAllByTestId('diff-row-remove').length).toBeGreaterThan(0);

    // 切到更早的版本作为基准：差异应包含「第一版正文」
    await userEvent.click(screen.getByText('第一版'));
    await waitFor(() =>
      expect(
        screen.getAllByTestId('diff-row-remove').some((row) => row.textContent?.includes('第一版正文')),
      ).toBe(true),
    );
  });

  it('还原会调用 historyRestore 并关闭弹窗', async () => {
    mockApi();
    const restore = vi.spyOn(noteService, 'historyRestore').mockResolvedValue({} as never);
    vi.spyOn(window, 'confirm').mockReturnValue(true);
    const onRestored = vi.fn();

    render(
      <HistoryDialog
        noteId="note_1"
        currentContent="当前正文"
        onClose={() => {}}
        onRestored={onRestored}
      />,
    );
    await screen.findAllByTestId('history-entry');

    await userEvent.click(screen.getByTestId('history-restore'));
    await waitFor(() => expect(restore).toHaveBeenCalledWith('note_1', 'e2'));
    expect(onRestored).toHaveBeenCalled();
  });

  it('没有历史时给出说明，还原按钮不可用', async () => {
    vi.spyOn(noteService, 'historyList').mockResolvedValue([]);
    render(<HistoryDialog noteId="note_1" currentContent="" onClose={() => {}} />);

    expect(await screen.findByText(/还没有历史版本/)).toBeInTheDocument();
    expect(screen.getByTestId('history-restore')).toBeDisabled();
  });
});

describe('HistoryDialog 命名版本（对标语雀「保存为版本」）', () => {
  it('可以把当前内容存成命名版本，并在列表里显示版本名', async () => {
    mockApi();
    const saveVersion = vi
      .spyOn(noteService, 'historySaveVersion')
      .mockResolvedValue({ ...entries[0]!, entryId: 'e3', label: 'v1 交付版' });
    // 保存后重新拉取列表应带上新版本
    vi.mocked(noteService.historyList)
      .mockResolvedValueOnce(entries)
      .mockResolvedValue([{ ...entries[0]!, entryId: 'e3', label: 'v1 交付版' }, ...entries]);

    render(<HistoryDialog noteId="note_1" currentContent={'当前正文'} onClose={() => {}} />);
    await screen.findAllByTestId('history-entry');

    await userEvent.type(screen.getByTestId('history-version-name'), 'v1 交付版');
    await userEvent.click(screen.getByTestId('history-save-version'));

    await waitFor(() => expect(saveVersion).toHaveBeenCalledWith('note_1', 'v1 交付版'));
    expect(await screen.findAllByTestId('history-entry-label')).toHaveLength(1);
    expect(screen.getByTestId('history-entry-label')).toHaveTextContent('v1 交付版');
  });

  it('可以给已有快照命名与改名（留空即清除）', async () => {
    mockApi();
    const label = vi
      .spyOn(noteService, 'historyLabel')
      .mockResolvedValue([{ ...entries[0]!, label: '重要节点' }, entries[1]!]);

    render(<HistoryDialog noteId="note_1" currentContent={''} onClose={() => {}} />);
    const items = await screen.findAllByTestId('history-entry');

    await userEvent.click(items[0]!.querySelector('[data-testid="history-label-edit"]') as HTMLElement);
    await userEvent.type(screen.getByTestId('history-label-input'), '重要节点');
    await userEvent.click(screen.getByTestId('history-label-save'));

    await waitFor(() => expect(label).toHaveBeenCalledWith('note_1', 'e2', '重要节点'));
    expect(await screen.findByTestId('history-entry-label')).toHaveTextContent('重要节点');
  });
});
