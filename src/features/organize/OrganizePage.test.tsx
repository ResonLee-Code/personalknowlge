import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { OrganizePage } from '@/features/organize/OrganizePage';
import { organizeService } from '@/services/organizeService';
import { aiService } from '@/services/aiService';
import { useNotesStore } from '@/stores/notesStore';
import type { OrganizeJob, OrganizeJobStatus, OrganizeSuggestion } from '@/types';

vi.mock('@/services/organizeService', () => ({
  organizeService: {
    generate: vi.fn(),
    list: vi.fn(),
    accept: vi.fn(),
    reject: vi.fn(),
    undo: vi.fn(),
    createJob: vi.fn(),
    jobs: vi.fn(),
    runJob: vi.fn(),
    pauseJob: vi.fn(),
    resumeJob: vi.fn(),
    cancelJob: vi.fn(),
    retryJob: vi.fn(),
    recover: vi.fn(),
  },
}));

vi.mock('@/services/aiService', () => ({
  aiService: { policy: vi.fn() },
}));

vi.mock('@/services/api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/services/api')>();
  return { ...actual, subscribe: vi.fn().mockResolvedValue(() => undefined) };
});

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), info: vi.fn(), error: vi.fn() },
}));

const suggestion: OrganizeSuggestion = {
  id: 'suggestion_1',
  noteId: 'note_a',
  noteTitle: '索引设计',
  kind: 'summary',
  status: 'pending',
  model: 'rule.organize',
  promptVersion: 'organize-v1',
  evidence: [
    {
      noteId: 'note_a',
      title: '索引设计',
      heading: 'FTS5',
      snippet: 'SQLite FTS5 使用倒排索引。',
      charStart: 0,
      charEnd: 20,
    },
  ],
  payload: {
    type: 'summary',
    summary: 'SQLite FTS5 使用倒排索引。',
    bullets: ['FTS5'],
  },
  sourceHash: 'hash',
  createdAt: '2026-10-20T10:00:00+08:00',
  updatedAt: '2026-10-20T10:00:00+08:00',
  appliedAt: null,
  undoneAt: null,
  error: null,
  undoAvailable: false,
};

function job(status: OrganizeJobStatus): OrganizeJob {
  return {
    id: `job_${status}`,
    workspaceId: 'workspace_1',
    kind: 'summary',
    status,
    total: 100,
    completed: 37,
    failed: 2,
    payload: {},
    error: status === 'failed' ? '模型请求失败' : null,
    createdAt: '2026-10-20T10:00:00+08:00',
    updatedAt: '2026-10-20T10:01:00+08:00',
    startedAt: '2026-10-20T10:00:10+08:00',
    finishedAt: null,
  };
}

describe('OrganizePage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(organizeService.list).mockResolvedValue([suggestion]);
    vi.mocked(organizeService.jobs).mockResolvedValue([]);
    vi.mocked(aiService.policy).mockResolvedValue({
      enabled: true,
      network: 'localOnly',
      networkConfirmedAt: null,
      allowFetchBody: false,
      domainAllowlist: [],
      domainDenylist: [],
      localModelCompose: false,
      localModelScore: false,
      remoteModelCompose: false,
      remoteModelScore: false,
      remoteModelAuthorizedAt: null,
      remoteModelKeySet: false,
      confirmEachOutbound: false,
      vectorEnabled: false,
      decisionKeySet: false,
      budget: { maxSubquestions: 5, maxPages: 12, maxDurationSecs: 480 },
      auditMaxEntries: 500,
    });
    vi.mocked(organizeService.accept).mockResolvedValue({
      ...suggestion,
      status: 'accepted',
      undoAvailable: true,
      appliedAt: '2026-10-20T10:01:00+08:00',
    });
    vi.mocked(organizeService.undo).mockResolvedValue({
      ...suggestion,
      status: 'accepted',
      undoAvailable: false,
      undoneAt: '2026-10-20T10:02:00+08:00',
    });
    useNotesStore.setState({
      items: [],
      selection: [],
      detail: null,
      refresh: vi.fn(),
    });
  });

  it('建议卡展示增量，接受前只调用建议命令', async () => {
    render(
      <MemoryRouter>
        <OrganizePage />
      </MemoryRouter>,
    );

    expect(await screen.findByText('索引设计')).toBeInTheDocument();
    expect(screen.getByText('## 摘要')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '接受' }));
    await waitFor(() => expect(organizeService.accept).toHaveBeenCalledWith('suggestion_1'));
  });

  it('已接受建议提供撤销入口', async () => {
    vi.mocked(organizeService.list).mockResolvedValue([
      {
        ...suggestion,
        status: 'accepted',
        undoAvailable: true,
        appliedAt: '2026-10-20T10:01:00+08:00',
      },
    ]);
    render(
      <MemoryRouter>
        <OrganizePage />
      </MemoryRouter>,
    );

    await userEvent.selectOptions(await screen.findByLabelText('队列状态'), 'accepted');
    await userEvent.click(await screen.findByRole('button', { name: '撤销' }));
    await waitFor(() => expect(organizeService.undo).toHaveBeenCalledWith('suggestion_1'));
  });

  it('远端嵌入启用逐次确认时也展示零请求确认框', async () => {
    vi.mocked(aiService.policy).mockResolvedValue({
      enabled: true,
      network: 'localOnly',
      networkConfirmedAt: null,
      allowFetchBody: false,
      domainAllowlist: [],
      domainDenylist: [],
      localModelCompose: false,
      localModelScore: false,
      remoteModelCompose: false,
      remoteModelScore: false,
      remoteModelAuthorizedAt: null,
      remoteModelKeySet: true,
      confirmEachOutbound: true,
      vectorEnabled: true,
      remoteEmbeddingModel: 'text-embedding-3-small',
      vectorAuthorizedAt: '2026-10-20T10:00:00+08:00',
      decisionKeySet: false,
      budget: { maxSubquestions: 5, maxPages: 12, maxDurationSecs: 480 },
      auditMaxEntries: 500,
    });

    render(
      <MemoryRouter>
        <OrganizePage />
      </MemoryRouter>,
    );

    expect(await screen.findByRole('checkbox', { name: /本次允许/ })).not.toBeChecked();
    expect(screen.getByText(/嵌入端点.*零请求/)).toBeInTheDocument();
  });

  it.each([
    ['running', '暂停', 'pauseJob'],
    ['paused', '继续', 'resumeJob'],
    ['failed', '重试', 'retryJob'],
  ] as const)('任务状态 %s 只执行允许的「%s」动作', async (status, label, command) => {
    const current = job(status);
    vi.mocked(organizeService.jobs).mockResolvedValue([current]);
    vi.mocked(organizeService[command]).mockResolvedValue(current);

    render(
      <MemoryRouter>
        <OrganizePage />
      </MemoryRouter>,
    );

    await userEvent.click(await screen.findByRole('button', { name: label }));

    await waitFor(() => expect(organizeService[command]).toHaveBeenCalledWith(current.id));
    expect(organizeService.jobs).toHaveBeenCalled();
  });

  it('运行中与暂停中的任务可以取消，失败任务只允许重试', async () => {
    const running = job('running');
    vi.mocked(organizeService.jobs).mockResolvedValue([running]);
    vi.mocked(organizeService.cancelJob).mockResolvedValue(running);

    const { unmount } = render(
      <MemoryRouter>
        <OrganizePage />
      </MemoryRouter>,
    );

    await userEvent.click(await screen.findByRole('button', { name: '取消' }));
    await waitFor(() => expect(organizeService.cancelJob).toHaveBeenCalledWith(running.id));
    unmount();

    vi.mocked(organizeService.jobs).mockResolvedValue([job('failed')]);
    render(
      <MemoryRouter>
        <OrganizePage />
      </MemoryRouter>,
    );
    await screen.findByRole('button', { name: '重试' });
    expect(screen.queryByRole('button', { name: '取消' })).not.toBeInTheDocument();
  });
});
