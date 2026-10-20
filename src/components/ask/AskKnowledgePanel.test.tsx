import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AskKnowledgePanel } from '@/components/ask/AskKnowledgePanel';
import { aiService } from '@/services/aiService';
import { askService } from '@/services/askService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import { useNotesStore } from '@/stores/notesStore';
import { useTagsStore } from '@/stores/tagsStore';
import { useUiStore } from '@/stores/uiStore';
import type { AiPolicy, AskAnswer } from '@/types';

vi.mock('@/services/aiService', () => ({
  aiService: { policy: vi.fn() },
}));

vi.mock('@/services/askService', () => ({
  askService: {
    ask: vi.fn(),
    previewAnswerNote: vi.fn(),
    saveAnswer: vi.fn(),
  },
}));

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), info: vi.fn(), error: vi.fn() },
}));

const policy: AiPolicy = {
  enabled: true,
  network: 'localOnly',
  networkConfirmedAt: null,
  allowFetchBody: false,
  domainAllowlist: [],
  domainDenylist: [],
  localModelEndpoint: null,
  localModelName: null,
  localModelCompose: false,
  localModelScore: false,
  remoteModelBaseUrl: null,
  remoteModelName: null,
  remoteModelCompose: false,
  remoteModelScore: false,
  remoteModelAuthorizedAt: null,
  remoteModelKeySet: false,
  confirmEachOutbound: false,
  vectorEnabled: false,
  decisionKeySet: false,
  budget: { maxSubquestions: 5, maxPages: 12, maxDurationSecs: 480 },
  auditMaxEntries: 500,
};

const answer: AskAnswer = {
  question: 'SQLite 索引有什么结论？',
  text: 'SQLite FTS5 使用倒排索引，并通过双字短语支持中文检索。[C1]',
  citations: [
    {
      citationId: 'C1',
      noteId: 'note_a',
      title: '索引设计',
      chunkIndex: 1,
      heading: 'FTS5 > 中文',
      snippet: 'SQLite FTS5 使用倒排索引，并通过双字短语支持中文检索。',
      charStart: 8,
      charEnd: 38,
    },
  ],
  provider: 'template.ask',
  tier: 'template',
  evidenceSufficient: true,
  evidenceNote: '检索到 1 条可引用材料。',
  warnings: [],
  channels: [{ id: 'note', available: true, note: '当前笔记按标题分块' }],
  retrievedCount: 1,
  tookMs: 12,
};

describe('AskKnowledgePanel（V2.0 M1）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(aiService.policy).mockResolvedValue(policy);
    vi.mocked(askService.ask).mockResolvedValue(answer);
    vi.mocked(askService.previewAnswerNote).mockResolvedValue({
      title: 'SQLite 索引问答',
      content: `${answer.text}\n\n## 来源\n\n- C1 · [[索引设计]]`,
      categoryId: null,
      sources: [
        {
          citationId: 'C1',
          noteId: 'note_a',
          title: '索引设计',
          heading: 'FTS5 > 中文',
        },
      ],
      warnings: ['当前答案来自模板档；保存后仍需人工核对。'],
    });
    vi.mocked(askService.saveAnswer).mockResolvedValue({
      noteId: 'note_answer',
      title: 'SQLite 索引问答',
      filePath: '00_Inbox/SQLite-索引问答.md',
      indexWarning: null,
    });
    useCategoriesStore.setState({ flat: [] });
    useTagsStore.setState({ tags: [] });
    useUiStore.setState({
      askPanelOpen: true,
      pendingHighlight: null,
      askSeed: null,
      researchSeed: null,
    });
    useNotesStore.setState({
      refresh: vi.fn(),
      detail: {
        note: {
          id: 'note_a',
          workspaceId: 'ws',
          title: '索引设计',
          filePath: '00_Inbox/索引设计.md',
          categoryId: null,
          status: 'active',
          contentHash: 'hash',
          wordCount: 20,
          favorite: false,
          createdAt: '2026-10-20T10:00:00+08:00',
          updatedAt: '2026-10-20T10:00:00+08:00',
          lastIndexedAt: '2026-10-20T10:00:00+08:00',
          sourceType: null,
          sourceRef: null,
          sourceReliability: null,
          validUntil: null,
          parentId: null,
          sortOrder: 0,
          pinned: false,
        },
        content: '# FTS5\n\nSQLite FTS5 使用倒排索引，并通过双字短语支持中文检索。',
        category: null,
        categoryPath: '',
        tags: [],
      },
    });
  });

  it('默认对当前笔记提问，展示可解析引用并跳回原文区间', async () => {
    render(
      <MemoryRouter>
        <AskKnowledgePanel />
      </MemoryRouter>,
    );

    const scope = await screen.findByLabelText('提问范围');
    await waitFor(() => expect(scope).toHaveValue('note'));
    const question = screen.getByLabelText('问题');
    await userEvent.type(question, 'SQLite 索引有什么结论？');
    await userEvent.click(screen.getByRole('button', { name: /当前笔记.*中提问/ }));

    await waitFor(() =>
      expect(askService.ask).toHaveBeenCalledWith({
        question: 'SQLite 索引有什么结论？',
        scope: 'note',
        scopeId: 'note_a',
        selection: null,
        history: [],
        confirmOutbound: false,
      }),
    );
    expect(await screen.findByText('有可引用材料')).toBeInTheDocument();
    expect(screen.getAllByText(/SQLite FTS5 使用倒排索引/).length).toBeGreaterThan(0);

    await userEvent.click(screen.getByText('索引设计'));
    expect(useUiStore.getState().pendingHighlight).toEqual({
      noteId: 'note_a',
      charStart: 8,
      charEnd: 38,
    });
  });

  it('证据不足时明确展示“不生成事实性结论”，不显示来源卡片', async () => {
    vi.mocked(askService.ask).mockResolvedValue({
      ...answer,
      text: '证据不足：当前作用域内没有找到可以回答这个问题的材料。',
      citations: [],
      evidenceSufficient: false,
      retrievedCount: 0,
    });
    render(
      <MemoryRouter>
        <AskKnowledgePanel />
      </MemoryRouter>,
    );

    await userEvent.type(screen.getByLabelText('问题'), '库里有没有量子纠缠实验？');
    await userEvent.click(screen.getByRole('button', { name: /当前笔记.*中提问/ }));

    expect(await screen.findByText('证据不足')).toBeInTheDocument();
    expect(screen.getByText(/没有找到可以回答/)).toBeInTheDocument();
    expect(screen.queryByText('来源与段落')).not.toBeInTheDocument();
  });

  it('多轮追问带上上一轮问答作为上下文，同时保留逐轮重新检索的结果', async () => {
    vi.mocked(askService.ask)
      .mockResolvedValueOnce(answer)
      .mockResolvedValueOnce({
        ...answer,
        question: '它有什么限制？',
        text: '限制是中文需要双字短语扩展。[C1]',
      });
    render(
      <MemoryRouter>
        <AskKnowledgePanel />
      </MemoryRouter>,
    );

    await userEvent.type(screen.getByLabelText('问题'), answer.question);
    await userEvent.click(screen.getByRole('button', { name: /当前笔记.*中提问/ }));
    await screen.findByText('第 1 轮 · SQLite 索引有什么结论？');

    await userEvent.type(screen.getByLabelText('问题'), '它有什么限制？');
    await userEvent.click(screen.getByRole('button', { name: /继续追问/ }));
    await screen.findByText('第 2 轮 · 它有什么限制？');

    expect(vi.mocked(askService.ask).mock.calls[1]?.[0]).toMatchObject({
      question: '它有什么限制？',
      history: [{ question: answer.question, answer: answer.text }],
    });
  });

  it('存为笔记必须先预览，确认后携带 confirm=true 落库', async () => {
    render(
      <MemoryRouter>
        <AskKnowledgePanel />
      </MemoryRouter>,
    );

    await userEvent.type(screen.getByLabelText('问题'), answer.question);
    await userEvent.click(screen.getByRole('button', { name: /当前笔记.*中提问/ }));
    await userEvent.click(await screen.findByRole('button', { name: /存为笔记/ }));

    expect(await screen.findByRole('dialog', { name: /把答案存为笔记/ })).toBeInTheDocument();
    expect(askService.previewAnswerNote).toHaveBeenCalledWith({
      answer,
      confirm: false,
    });
    expect(screen.getByText('当前答案来自模板档；保存后仍需人工核对。')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: /确认写入/ }));
    await waitFor(() =>
      expect(askService.saveAnswer).toHaveBeenCalledWith({
        answer,
        title: 'SQLite 索引问答',
        categoryId: null,
        confirm: true,
      }),
    );
  });

  it('从编辑器选区打开时预填问题，并把选中文字作为不可引用上下文发送', async () => {
    useUiStore.setState({
      askSeed: {
        noteId: 'note_a',
        question: '请解释并核对这段内容的要点。',
        selection: 'SQLite FTS5 使用倒排索引',
      },
    });
    render(
      <MemoryRouter>
        <AskKnowledgePanel />
      </MemoryRouter>,
    );

    expect(screen.getByLabelText('问题')).toHaveValue('请解释并核对这段内容的要点。');
    expect(screen.getByText('SQLite FTS5 使用倒排索引')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /当前笔记.*中提问/ }));
    await waitFor(() =>
      expect(askService.ask).toHaveBeenCalledWith(
        expect.objectContaining({
          scope: 'note',
          scopeId: 'note_a',
          selection: 'SQLite FTS5 使用倒排索引',
        }),
      ),
    );
  });

  it('Ask 答案可把当前问题带入 Research 2.0', async () => {
    render(
      <MemoryRouter>
        <AskKnowledgePanel />
      </MemoryRouter>,
    );

    await userEvent.type(screen.getByLabelText('问题'), answer.question);
    await userEvent.click(screen.getByRole('button', { name: /当前笔记.*中提问/ }));
    await userEvent.click(await screen.findByRole('button', { name: '深入调研' }));

    expect(useUiStore.getState().askPanelOpen).toBe(false);
    expect(useUiStore.getState().researchSeed).toEqual({
      topic: answer.question,
      scope: '从 Ask 会话发起的深入调研',
    });
  });
});
