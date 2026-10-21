import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { WritingPanel } from '@/components/editor/WritingPanel';
import { writingService } from '@/services/writingService';
import type { NoteDetail, WritingDraft } from '@/types';

vi.mock('@/services/writingService', () => ({
  writingService: {
    draft: vi.fn(),
    applyPreview: vi.fn(),
    apply: vi.fn(),
  },
}));

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), error: vi.fn(), info: vi.fn(), warning: vi.fn() },
}));

const citation = {
  citationId: 'C1',
  noteId: 'note-1',
  title: '写作原则',
  chunkIndex: 0,
  heading: '原则',
  snippet: '引用必须可回溯。',
  charStart: 0,
  charEnd: 8,
};

const draft: WritingDraft = {
  noteId: 'note-1',
  mode: 'outline',
  text: '- 第一项 [1]\n\n- 第二项 [1]',
  segments: [
    { id: 'W1', text: '- 第一项 [1]', citations: [citation] },
    { id: 'W2', text: '- 第二项 [1]', citations: [citation] },
  ],
  citations: [citation],
  provider: 'template.writing',
  tier: 'template',
  evidenceSufficient: true,
  warnings: ['模板档需要人工核对。'],
  baseContentHash: 'hash-before',
  targetStart: 17,
  targetEnd: 17,
  inputTokens: 100,
  tookMs: 5,
};

const detail: NoteDetail = {
  note: {
    id: 'note-1',
    workspaceId: 'ws',
    title: '写作原则',
    filePath: '写作原则.md',
    categoryId: null,
    status: 'active',
    contentHash: 'hash-after',
    wordCount: 50,
    favorite: false,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:01Z',
  },
  content: '# 写作原则\n\n- 第二项 [1]\n\n## 写作来源\n\n- [[写作原则]]',
  categoryPath: '',
  tags: [],
};

describe('WritingPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(writingService.draft).mockResolvedValue(draft);
    vi.mocked(writingService.applyPreview).mockResolvedValue({
      noteId: 'note-1',
      content: '# 写作原则\n\n- 第二项 [1]\n\n## 写作来源\n\n- [[写作原则]]',
      addedChars: 12,
      removedChars: 0,
      acceptedSegments: 1,
      citations: [citation],
      warnings: [],
    });
    vi.mocked(writingService.apply).mockResolvedValue({
      detail,
      acceptedSegments: 1,
      sourceCount: 1,
    });
  });

  it('逐段拒绝后必须先生成 diff，再确认写入', async () => {
    const onApplied = vi.fn();
    render(
      <WritingPanel
        noteId="note-1"
        noteTitle="写作原则"
        content={'# 写作原则\n\n正文'}
        baseContentHash="hash-before"
        selection={null}
        selectionStart={null}
        selectionEnd={null}
        onClose={vi.fn()}
        onApplied={onApplied}
      />,
    );

    await userEvent.click(screen.getByRole('button', { name: '生成草稿' }));
    expect(await screen.findByTestId('writing-segment-W1')).toBeInTheDocument();
    expect(screen.getByText(/模板档需要人工核对/)).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: '[C1] 写作原则' })).toHaveLength(2);

    await userEvent.click(screen.getByRole('button', { name: '拒绝段落 W1' }));
    const applyButton = screen.getByRole('button', { name: /确认写入 1 段/ });
    expect(applyButton).toBeDisabled();
    await userEvent.click(screen.getByRole('button', { name: '预览 diff' }));
    expect(await screen.findByTestId('writing-diff')).toBeInTheDocument();
    expect(screen.getAllByTestId('writing-diff-add').length).toBeGreaterThan(0);
    expect(applyButton).toBeEnabled();
    await userEvent.click(applyButton);

    await waitFor(() =>
      expect(writingService.apply).toHaveBeenCalledWith(
        expect.objectContaining({
          confirm: true,
          acceptedSegments: [draft.segments[1]],
        }),
      ),
    );
    expect(onApplied).toHaveBeenCalledWith(detail);
  });
});
