import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { NoteEditorPage } from '@/components/notes/NoteEditorPage';
import { TooltipProvider } from '@/components/ui/tooltip';
import { linkService } from '@/services/linkService';
import { noteService } from '@/services/noteService';
import { relationService } from '@/services/relationService';
import { useNotesStore } from '@/stores/notesStore';
import { useUiStore } from '@/stores/uiStore';
import { useWorkspaceStore } from '@/stores/workspaceStore';
import type { NoteDetail } from '@/types';

vi.mock('@/components/editor/MarkdownEditor', () => ({
  MarkdownEditor: () => <div data-testid="markdown-editor-stub" />,
}));
vi.mock('@/components/editor/BacklinkPanel', () => ({ BacklinkPanel: () => null }));
vi.mock('@/components/editor/ConflictDialog', () => ({ ConflictDialog: () => null }));
vi.mock('@/components/editor/HistoryDialog', () => ({ HistoryDialog: () => null }));
vi.mock('@/components/editor/RenameFileDialog', () => ({ RenameFileDialog: () => null }));
vi.mock('@/components/editor/SaveTemplateDialog', () => ({ SaveTemplateDialog: () => null }));
vi.mock('@/components/editor/WritingPanel', () => ({ WritingPanel: () => null }));
vi.mock('@/components/editor/SelectionContextMenu', () => ({ SelectionContextMenu: () => null }));
vi.mock('@/components/editor/SourceDialog', () => ({ SourceDialog: () => null }));
vi.mock('@/components/editor/OutlinePanel', () => ({ OutlinePanel: () => null }));
vi.mock('@/components/notes/ExportNoteDialog', () => ({ ExportNoteDialog: () => null }));
vi.mock('@/components/notes/MoveNoteDialog', () => ({ MoveNoteDialog: () => null }));

const detail: NoteDetail = {
  note: {
    id: 'note_1',
    workspaceId: 'workspace_1',
    title: 'V2 阅读列与大纲',
    filePath: '02_V2/editor.md',
    categoryId: 'cat_1',
    status: 'active',
    contentHash: 'hash-1',
    wordCount: 120,
    favorite: false,
    createdAt: '2026-10-20T01:00:00+08:00',
    updatedAt: '2026-10-25T01:00:00+08:00',
    lastIndexedAt: '2026-10-25T01:01:00+08:00',
    sourceType: 'web',
    sourceRef: 'https://example.com/article',
    sourceReliability: 4,
    validUntil: '2026-10-01T00:00:00+08:00',
  },
  content: '# V2 阅读列与大纲\n\n正文。',
  category: null,
  categoryPath: '产品/V2',
  tags: [
    {
      id: 'tag_1',
      workspaceId: 'workspace_1',
      name: 'V2',
      createdAt: '2026-10-25T01:00:00+08:00',
      noteCount: 1,
    },
  ],
};

describe('NoteEditorPage', () => {
  beforeEach(() => {
    useNotesStore.setState({
      detail,
      detailLoading: false,
      saveStatus: 'saved',
      keyword: '',
    });
    useWorkspaceStore.setState({ settings: null });
    useUiStore.setState({
      editorMode: 'split',
      outlinePlacement: 'list',
      outlineMode: 'pinned',
      noteActionRequest: null,
      pendingHighlight: null,
    });
    vi.spyOn(noteService, 'list').mockResolvedValue({
      items: [],
      total: 0,
      page: 1,
      pageSize: 500,
    });
    vi.spyOn(linkService, 'outgoing').mockResolvedValue([]);
    vi.spyOn(linkService, 'counts').mockResolvedValue({
      backlinks: 2,
      outgoing: 3,
      unresolved: 1,
    });
    vi.spyOn(relationService, 'list').mockResolvedValue({
      outgoing: [
        {
          id: 'relation_1',
          relationType: 'SUPERSEDES',
          targetNoteId: 'note_old',
          targetTitle: '旧方案',
        },
      ],
      incoming: [
        {
          id: 'relation_2',
          relationType: 'RELATED_TO',
          sourceNoteId: 'note_new',
          sourceTitle: '新方案',
        },
      ],
    });
  });

  it('标题与元信息使用可读的 900px 居中列，并保留来源与时效状态', async () => {
    render(
      <TooltipProvider delayDuration={0}>
        <MemoryRouter initialEntries={['/notes/note_1']}>
          <NoteEditorPage />
        </MemoryRouter>
      </TooltipProvider>,
    );

    expect(screen.getByTestId('editor-header')).toHaveTextContent('可能已过期');
    await waitFor(() =>
      expect(screen.getByTestId('editor-header')).toHaveTextContent('被 2 篇引用'),
    );
    expect(screen.getByTestId('editor-header').firstElementChild).toHaveClass(
      'mx-auto',
      'w-full',
      'max-w-[900px]',
      'px-6',
    );
    expect(screen.getByLabelText('笔记标题')).toHaveValue('V2 阅读列与大纲');
    expect(screen.getByTestId('markdown-preview')).toHaveClass('max-w-[900px]');
    await waitFor(() => expect(screen.getByText('已取代「旧方案」')).toBeInTheDocument());
    await userEvent.click(screen.getByLabelText('来源与时效'));
    expect(useUiStore.getState().noteActionRequest).toBe('source');
  });
});
