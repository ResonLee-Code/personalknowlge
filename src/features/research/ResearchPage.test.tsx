import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ResearchPage } from '@/features/research/ResearchPage';
import { aiService } from '@/services/aiService';
import { researchService } from '@/services/researchService';
import type { AiPolicy, ResearchSession } from '@/types';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

const enabledPolicy: AiPolicy = {
  enabled: true,
  network: 'localOnly',
  networkConfirmedAt: null,
  allowFetchBody: false,
  domainAllowlist: [],
  domainDenylist: [],
  budget: { maxSubquestions: 5, maxPages: 12, maxDurationSecs: 480 },
  auditMaxEntries: 500,
};

function planned(): ResearchSession {
  return {
    id: 'research_1',
    request: { topic: 'Rust 异步运行时调度', writeBack: true },
    status: 'planned',
    createdAt: '2026-10-11T10:00:00+08:00',
    updatedAt: '2026-10-11T10:00:00+08:00',
    plan: {
      topic: 'Rust 异步运行时调度',
      timeWindowDays: 730,
      budget: enabledPolicy.budget,
      subQuestions: [
        { id: 'q1', question: '「Rust 异步运行时调度」的核心概念与定义是什么？', keywords: ['Rust'], localHits: 0 },
        { id: 'q2', question: '「Rust 异步运行时调度」有哪些主流方案或做法？', keywords: ['Rust'], localHits: 0 },
      ],
      localBaseline: [
        { noteId: 'note_1', title: 'Rust 异步调度实现要点', snippet: '实现要点…' },
      ],
      notes: ['本次只用本地知识库材料（外部检索属于阶段 D3）。'],
    },
    sources: [],
    report: null,
    proposal: null,
    steps: [],
    elapsedMs: 0,
  };
}

function composed(): ResearchSession {
  const session = planned();
  return {
    ...session,
    status: 'composed',
    sources: [
      {
        reference: 1,
        title: 'Rust 异步调度实现要点',
        origin: 'local',
        noteId: 'note_1',
        subQuestionId: 'q2',
        level: 'A',
        reliability: 5,
        gradeReason: '沿用笔记已标注的 sourceReliability=5',
        fetchedAt: '2026-10-10T10:00:00+08:00',
        snippet: '实现要点在于任务唤醒。',
        stale: false,
      },
      {
        reference: 2,
        title: 'Rust 异步调度限制',
        origin: 'local',
        noteId: 'note_2',
        subQuestionId: 'q3',
        level: 'D',
        reliability: 2,
        gradeReason: '笔记未标注 sourceReliability：按「弱」处理',
        fetchedAt: '2026-10-09T10:00:00+08:00',
        snippet: '阻塞调用会拖垮运行时。',
        stale: false,
      },
    ],
    report: {
      generatedAt: '2026-10-11T10:05:00+08:00',
      dataCutoff: '2026-10-11',
      composer: 'template.draft',
      conclusions: [],
      keyFacts: [],
      conflicts: [],
      kbRelations: [],
      pending: [],
      references: [],
      speculation: ['（模板草稿·未接模型）以下是按材料顺序罗列的待确认方向。'],
      warnings: [
        { kind: 'singleSource', message: '有 1 条来源是单一来源、没有交叉验证。', references: [2] },
        { kind: 'templateGenerated', message: '本报告由模板实现生成（未接入模型）。', references: [] },
      ],
      sourceReliability: 2,
      validUntil: '2027-04-09',
      markdown: '# 调研报告：Rust 异步运行时调度\n\n> **资料截至 2026-10-11** · 生成方式 `template.draft`',
    },
  };
}

function proposed(): ResearchSession {
  const session = composed();
  return {
    ...session,
    status: 'proposed',
    proposal: {
      title: '调研报告：Rust 异步运行时调度',
      categoryId: null,
      tags: ['调研报告', '待复核'],
      sourceType: 'research',
      sourceRef: '[1] Rust 异步调度实现要点 (A 权威 笔记)',
      sourceReliability: 2,
      validUntil: '2027-04-09',
      body: '# 调研报告：Rust 异步运行时调度\n\n> **资料截至 2026-10-11**',
      diffPreview: '+# 调研报告：Rust 异步运行时调度\n+> **资料截至 2026-10-11**',
      duplicates: [
        { noteId: 'note_9', title: '调研报告：Rust 异步运行时调度', updatedAt: '2026-10-08T10:00:00+08:00' },
      ],
      suggestSourceNote: false,
    },
  };
}

function renderPage() {
  return render(
    <MemoryRouter>
      <ResearchPage />
    </MemoryRouter>,
  );
}

describe('ResearchPage（V1.2.1 D2）', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(aiService, 'policy').mockResolvedValue(enabledPolicy);
    vi.spyOn(researchService, 'list').mockResolvedValue([]);
    vi.spyOn(researchService, 'get').mockResolvedValue(proposed());
  });

  it('AI 关闭时明确提示「不会自行打开或联网」并给出设置入口', async () => {
    vi.spyOn(aiService, 'policy').mockResolvedValue({ ...enabledPolicy, enabled: false });
    renderPage();

    const hint = await screen.findByTestId('research-ai-disabled');
    expect(hint).toHaveTextContent('不会自行打开或联网');
    expect(within(hint).getByRole('link', { name: '设置 → AI' })).toHaveAttribute(
      'href',
      '/settings',
    );
  });

  it('宽泛课题会显示后端给出的补充范围提示', async () => {
    vi.spyOn(researchService, 'start').mockRejectedValue({
      code: 'INVALID_INPUT',
      message: '课题「知识库」过于宽泛：请补充范围（领域、关注点或时间窗）。',
    });
    renderPage();

    await userEvent.type(await screen.findByTestId('research-topic'), '知识库');
    await userEvent.click(screen.getByTestId('research-start'));

    expect(await screen.findByTestId('research-error')).toHaveTextContent('过于宽泛');
    expect(researchService.start).toHaveBeenCalledWith(
      expect.objectContaining({ topic: '知识库', writeBack: true }),
    );
  });

  it('五步逐步推进：计划 → 收集 → 验证 → 报告 → 提案', async () => {
    vi.spyOn(researchService, 'start').mockResolvedValue(planned());
    const collect = vi
      .spyOn(researchService, 'collect')
      .mockResolvedValue({ ...planned(), status: 'collected' });
    const verify = vi.spyOn(researchService, 'verify').mockResolvedValue({
      ...composed(),
      status: 'verified',
      report: { ...composed().report!, markdown: '' },
    });
    const compose = vi.spyOn(researchService, 'compose').mockResolvedValue(composed());
    const proposal = vi.spyOn(researchService, 'proposal').mockResolvedValue(proposed());

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));

    const plan = await screen.findByTestId('research-plan');
    expect(plan).toHaveTextContent('核心概念与定义');

    await userEvent.click(await screen.findByTestId('research-collect'));
    await waitFor(() => expect(collect).toHaveBeenCalledWith('research_1', false));

    await userEvent.click(await screen.findByTestId('research-verify'));
    await waitFor(() => expect(verify).toHaveBeenCalledWith('research_1'));

    await userEvent.click(await screen.findByTestId('research-compose'));
    await waitFor(() => expect(compose).toHaveBeenCalledWith('research_1'));
    expect(await screen.findByTestId('research-report')).toHaveTextContent('资料截至 2026-10-11');
    expect(screen.getByTestId('research-warnings')).toHaveTextContent('单一来源');

    await userEvent.click(await screen.findByTestId('research-proposal'));
    await waitFor(() => expect(proposal).toHaveBeenCalledWith('research_1'));
    expect(await screen.findByTestId('research-proposal')).toHaveTextContent('不覆盖');

    // 打开写入确认：标题可改、diff 可读、同标题时默认选「追加」
    await userEvent.click(screen.getByTestId('research-write-open'));
    const dialog = await screen.findByTestId('research-write-dialog');
    expect(within(dialog).getByTestId('research-write-title')).toHaveValue(
      '调研报告：Rust 异步运行时调度',
    );
    expect(within(dialog).getByTestId('research-write-diff')).toHaveTextContent('+');
    expect(within(dialog).getByTestId('research-write-mode-append')).toBeChecked();
  });

  it('确认写入后显示「已写入知识库」与打开笔记入口', async () => {
    vi.spyOn(researchService, 'start').mockResolvedValue(proposed());
    const write = vi.spyOn(researchService, 'write').mockResolvedValue({
      sessionId: 'research_1',
      outcome: 'created',
      noteId: 'note_new',
      noteTitle: '调研报告：Rust 异步运行时调度',
      sourceNoteId: null,
      sourceReliability: 2,
      validUntil: '2027-04-09',
      diffPreview: '+# 调研报告',
    });
    vi.spyOn(researchService, 'get').mockResolvedValue({
      ...proposed(),
      status: 'written',
      writtenNoteId: 'note_new',
    });

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));

    await userEvent.click(await screen.findByTestId('research-write-open'));
    await userEvent.click(await screen.findByTestId('research-write-mode-create'));
    await userEvent.click(await screen.findByTestId('research-write-confirm'));

    await waitFor(() =>
      expect(write).toHaveBeenCalledWith(
        expect.objectContaining({ sessionId: 'research_1', mode: 'create', includeSpeculation: false }),
      ),
    );
    expect(await screen.findByTestId('research-written')).toHaveTextContent('已写入知识库');
  });

  it('中止会话会保留草稿并回到可继续的状态', async () => {
    vi.spyOn(researchService, 'start').mockResolvedValue(planned());
    const abort = vi.spyOn(researchService, 'abort').mockResolvedValue({
      ...planned(),
      status: 'aborted',
      abortReason: '用户中止',
    });

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));
    await userEvent.click(await screen.findByTestId('research-abort'));

    await waitFor(() => expect(abort).toHaveBeenCalledWith('research_1', '用户中止'));
    expect(await screen.findByTestId('research-history')).toBeInTheDocument();
  });

  it('联网档位打开时，收集前先弹一次说明并在确认后带 confirmed 调用（§9.7）', async () => {
    vi.spyOn(aiService, 'policy').mockResolvedValue({
      ...enabledPolicy,
      network: 'allowNetwork',
      networkConfirmedAt: '2026-10-11T09:00:00+08:00',
      searchEndpoint: 'https://searx.example/search?q={query}',
    });
    vi.spyOn(researchService, 'start').mockResolvedValue(planned());
    const collect = vi
      .spyOn(researchService, 'collect')
      .mockResolvedValue({ ...planned(), status: 'collected' });

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));
    await userEvent.click(await screen.findByTestId('research-collect'));

    // 先确认，再联网：此时**还没有**调用后端
    const dialog = await screen.findByTestId('research-network-confirm');
    expect(dialog).toHaveTextContent('本次将联网检索');
    expect(dialog).toHaveTextContent('searx.example');
    expect(collect).not.toHaveBeenCalled();

    await userEvent.click(screen.getByTestId('research-network-ok'));
    await waitFor(() => expect(collect).toHaveBeenCalledWith('research_1', true));
  });

  it('联网说明里选「暂不联网」时只用本地材料', async () => {
    vi.spyOn(aiService, 'policy').mockResolvedValue({
      ...enabledPolicy,
      network: 'allowNetwork',
      networkConfirmedAt: '2026-10-11T09:00:00+08:00',
      searchEndpoint: 'https://searx.example/search?q={query}',
    });
    vi.spyOn(researchService, 'start').mockResolvedValue(planned());
    const collect = vi
      .spyOn(researchService, 'collect')
      .mockResolvedValue({ ...planned(), status: 'collected' });

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));
    await userEvent.click(await screen.findByTestId('research-collect'));
    await userEvent.click(await screen.findByTestId('research-network-cancel'));

    await waitFor(() => expect(collect).not.toHaveBeenCalled());
    expect(screen.queryByTestId('research-network-confirm')).not.toBeInTheDocument();
  });
});
