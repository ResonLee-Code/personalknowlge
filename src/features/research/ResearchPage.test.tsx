import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ResearchPage } from '@/features/research/ResearchPage';
import { aiService } from '@/services/aiService';
import { subscribe } from '@/services/api';
import { researchService } from '@/services/researchService';
import type { AiPolicy, ResearchSession } from '@/types';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

vi.mock('@/services/api', async () => {
  const actual = await vi.importActual<typeof import('@/services/api')>('@/services/api');
  return { ...actual, subscribe: vi.fn(async () => () => undefined) };
});

const enabledPolicy: AiPolicy = {
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
        credibility: 0.9,
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
      // D5/N9：证据不足时界面要**直接**说清楚差在哪（不能只写在 markdown 里让人自己找）
      evidence: {
        sufficient: false,
        score: 0.25,
        coveredSubQuestions: 1,
        totalSubQuestions: 5,
        independentSources: 1,
        usableSources: 2,
        gaps: [
          {
            kind: 'partialCoverage',
            message: '5 个子问题里有 4 个**一条材料都没有**：这部分不是「查过没有」，而是「没查」。',
            subQuestionIds: ['q2', 'q3', 'q4', 'q5'],
            references: [],
          },
        ],
        verdict: '**证据不足**：1/5 个子问题有材料，1 个独立来源 —— 本次**不给结论**，缺口见下。',
        recommendation: '缺口就是上面每条 `gaps`：按子问题补材料后重跑收集。',
      },
      composer: 'template.draft',
      decisionProvider: 'rule',
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
    await waitFor(() => expect(verify).toHaveBeenCalledWith('research_1', false));

    await userEvent.click(await screen.findByTestId('research-compose'));
    await waitFor(() => expect(compose).toHaveBeenCalledWith('research_1', false));
    expect(await screen.findByTestId('research-report')).toHaveTextContent('资料截至 2026-10-11');
    expect(screen.getByTestId('research-warnings')).toHaveTextContent('单一来源');
    // D5/N9：证据充分性要**直接显示**（够不够、差在哪），不用去 markdown 里翻
    const evidence = screen.getByTestId('research-evidence');
    expect(evidence).toHaveTextContent('证据不足');
    expect(evidence).toHaveTextContent('1/5 个子问题');
    expect(evidence).toHaveTextContent('一条材料都没有');
    expect(evidence).toHaveTextContent('建议：');

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

  it('报告与来源展示决策层打分：置信度与可信度（D5 规则档）', async () => {
    const withScores = composed();
    withScores.report = {
      ...withScores.report!,
      keyFacts: [
        {
          subQuestionId: 'q2',
          text: '关于「主流方案」：任务唤醒与批量窃取',
          citations: [1, 2],
          bestLevel: 'A',
          crossVerified: true,
          fresh: true,
          relatedNotes: ['note_1'],
          confidence: 0.85,
        },
      ],
    };
    vi.spyOn(researchService, 'start').mockResolvedValue(withScores);

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));

    // 来源列表显示可信度（来自决策层，不是又一套前端规则）
    const sources = await screen.findByTestId('research-sources');
    expect(sources).toHaveTextContent('可信度 90%');

    // 报告头显示打分来源；关键事实表带置信度列
    const report = screen.getByTestId('research-report');
    expect(report).toHaveTextContent('打分 rule');
    // 平均置信度由后端分数汇总得到（0.85 → 高）
    expect(screen.getByTestId('research-avg-confidence')).toHaveTextContent('平均置信度 高 0.85');
  });

  // 验收 1（§9.9）：结论段每一句都能点开对应引用——
  // 本地来源跳到笔记，外部来源是带了 target=_blank 的外链，编号与参考来源一一对应。
  it('结论与引用可点开：本地来源跳笔记、外部来源是新窗口外链', async () => {
    const session = composed();
    session.report = {
      ...session.report!,
      conclusions: [
        {
          subQuestionId: 'q2',
          text: '主流方案集中在工作窃取队列。',
          citations: [1],
          bestLevel: 'A',
          crossVerified: true,
          fresh: true,
          relatedNotes: ['note_1'],
          confidence: 0.8,
        },
      ],
      keyFacts: [
        {
          subQuestionId: 'q3',
          text: '阻塞调用会拖垮运行时。',
          citations: [2],
          bestLevel: 'D',
          crossVerified: true,
          fresh: true,
          relatedNotes: [],
          confidence: 0.5,
        },
      ],
      references: [
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
          credibility: 0.9,
        },
        {
          reference: 2,
          title: '工程博客：任务窃取',
          url: 'https://weak.test/post',
          origin: 'web',
          noteId: null,
          subQuestionId: 'q3',
          level: 'D',
          reliability: 2,
          gradeReason: '技术社区博客：按弱来源处理',
          fetchedAt: '2026-10-11T09:00:00+08:00',
          snippet: '任务窃取的实现细节。',
          stale: true,
          credibility: 0.4,
        },
      ],
    };
    vi.spyOn(researchService, 'start').mockResolvedValue(session);

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));

    const block = await screen.findByTestId('research-citations');
    // 每一句结论/事实都带上了对应的引用编号
    expect(block).toHaveTextContent('主流方案集中在工作窃取队列。');
    expect(block).toHaveTextContent('阻塞调用会拖垮运行时。');

    const local = within(block).getByTestId('citation-note');
    expect(local).toHaveAttribute('href', '/notes/note_1');
    expect(local).toHaveTextContent('[1] Rust 异步调度实现要点');
    // 悬停提示带上级别与获取时间（「URL + 抓取时间」可核对）
    expect(local.getAttribute('title')).toContain('获取');

    const external = within(block).getByTestId('citation-url');
    expect(external).toHaveAttribute('href', 'https://weak.test/post');
    expect(external).toHaveAttribute('target', '_blank');
    expect(external.getAttribute('title')).toContain('可能已过时');
  });

  // §9.2：长步骤（联网抓取可达预算上限）要能看到实时进度——
  // 后端逐步推 `ai://research-progress`（只含元数据），页面显示当前说明 / 已抓页数 / 来源数。
  it('收集过程中显示后端推来的实时进度（步骤说明 / 已抓页数 / 来源数）', async () => {
    vi.spyOn(researchService, 'start').mockResolvedValue(planned());
    // 收集一直不返回：界面停在「进行中」，正是需要看到进度的时刻
    vi.spyOn(researchService, 'collect').mockImplementation(() => new Promise(() => {}));

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));
    await userEvent.click(await screen.findByTestId('research-collect'));

    // 取出页面注册的进度事件处理函数，模拟后端推事件
    const handler = vi
      .mocked(subscribe)
      .mock.calls.find(([event]) => event === 'ai://research-progress')?.[1];
    expect(handler, '页面必须订阅 ai://research-progress').toBeTruthy();

    act(() => {
      handler?.({
        sessionId: 'research_1',
        step: 'collect',
        detail: '已抓取正文 3 页（预算 ≤12 页）',
        pagesFetched: 3,
        sources: 5,
      });
    });

    const line = await screen.findByTestId('research-progress');
    expect(line).toHaveTextContent('已抓取正文 3 页（预算 ≤12 页）');
    expect(line).toHaveTextContent('已抓 3 页');
    expect(line).toHaveTextContent('来源 5 条');
  });

  // Track C1：推测段流式——逐块预览；这一档失败时清空，**不留下半截内容**。
  it('推测段生成中逐块预览，且失败时不留半截内容', async () => {
    const verified = {
      ...composed(),
      status: 'verified' as const,
      report: { ...composed().report!, markdown: '' },
    };
    vi.spyOn(researchService, 'start').mockResolvedValue(verified);
    // 生成一直不返回：界面停在「生成中」，正是需要看到流式预览的时刻
    vi.spyOn(researchService, 'compose').mockImplementation(() => new Promise(() => {}));

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));
    await userEvent.click(await screen.findByTestId('research-compose'));

    const handler = vi
      .mocked(subscribe)
      .mock.calls.find(([event]) => event === 'ai://research-compose-delta')?.[1];
    expect(handler, '页面必须订阅 ai://research-compose-delta').toBeTruthy();

    act(() => {
      handler?.({
        sessionId: 'research_1',
        providerId: 'local.ollama:qwen2.5:3b',
        text: '第一块',
        state: 'streaming',
      });
    });
    const preview = await screen.findByTestId('research-compose-preview');
    expect(preview).toHaveTextContent('local.ollama:qwen2.5:3b');
    expect(preview).toHaveTextContent('第一块');

    // 第二块到达：显示的是**累计文本**（界面直接替换，不做拼接）
    act(() => {
      handler?.({
        sessionId: 'research_1',
        providerId: 'local.ollama:qwen2.5:3b',
        text: '第一块第二块',
        state: 'streaming',
      });
    });
    expect(screen.getByTestId('research-compose-preview')).toHaveTextContent('第一块第二块');

    // 这一档失败：后端发空串 + failed —— 预览必须清空，不留半截内容
    act(() => {
      handler?.({
        sessionId: 'research_1',
        providerId: 'local.ollama:qwen2.5:3b',
        text: '',
        state: 'failed',
      });
    });
    const afterFailure = screen.getByTestId('research-compose-preview');
    expect(afterFailure).toHaveTextContent('这一档失败，正在降级');
    expect(afterFailure).not.toHaveTextContent('第一块第二块');
  });

  // Track C2：开启「逐次外发确认」后，先把**将要发送的片段 + 目标主机**摆出来；
  // 未确认就不提交（后端另有零请求保证），取消也不提交。
  it('逐次外发确认：先展示片段与主机，确认后才提交', async () => {
    vi.spyOn(aiService, 'policy').mockResolvedValue({
      ...enabledPolicy,
      remoteModelCompose: true,
      remoteModelAuthorizedAt: '2026-10-11T09:00:00+08:00',
      confirmEachOutbound: true,
    });
    vi.spyOn(researchService, 'start').mockResolvedValue({
      ...composed(),
      status: 'verified',
      report: { ...composed().report!, markdown: '' },
    });
    const outboundPreview = vi.spyOn(researchService, 'outboundPreview').mockResolvedValue({
      willSend: true,
      host: 'api.example.com',
      snippets: [
        { title: 'Rust 异步调度实现要点', snippet: '实现要点在于任务唤醒与批量窃取。' },
      ],
      reason: '本次会把下列片段交给在线模型（已开启逐次确认：要点确认才会发送）。',
    });
    const compose = vi.spyOn(researchService, 'compose').mockResolvedValue(composed());

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));
    await userEvent.click(await screen.findByTestId('research-compose'));

    await waitFor(() => expect(outboundPreview).toHaveBeenCalledWith('research_1'));
    const dialog = await screen.findByTestId('research-outbound-confirm');
    expect(screen.getByTestId('research-outbound-host')).toHaveTextContent('api.example.com');
    expect(dialog).toHaveTextContent('实现要点在于任务唤醒与批量窃取。');
    expect(compose, '没确认之前不能提交').not.toHaveBeenCalled();

    // 取消：不提交、不留下对话框
    await userEvent.click(screen.getByTestId('research-outbound-cancel'));
    expect(compose).not.toHaveBeenCalled();
    expect(screen.queryByTestId('research-outbound-confirm')).not.toBeInTheDocument();

    // 再来一次并确认：这次才带着「本次已确认」提交
    await userEvent.click(screen.getByTestId('research-compose'));
    await userEvent.click(await screen.findByTestId('research-outbound-ok'));
    await waitFor(() => expect(compose).toHaveBeenCalledWith('research_1', true));
  });

  // Track C5：报告要注明冲突判定到底用的哪一档（规则档 / 模型档）。
  it('报告注明冲突判定用的档位', async () => {
    vi.spyOn(researchService, 'start').mockResolvedValue({
      ...composed(),
      report: { ...composed().report!, conflictProvider: 'local.ollama' },
    });

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));

    const line = await screen.findByTestId('research-conflict-provider');
    expect(line).toHaveTextContent('local.ollama');
    expect(line).toHaveTextContent('模型档');
  });

  // Track D2/N6：报告要显示打分的**决策 Trace**（缓存是否命中、试过哪些档、谁失败了）。
  it('报告显示打分的决策 Trace', async () => {
    vi.spyOn(researchService, 'start').mockResolvedValue({
      ...composed(),
      report: {
        ...composed().report!,
        decisionTrace: {
          kind: 'RELEVANCE',
          cache: 'miss',
          provider: 'rule',
          candidates: 3,
          attempts: [
            { provider: 'local.ollama', ok: false, detail: '连不上' },
            { provider: 'rule', ok: true, detail: '返回 3 条' },
          ],
          tookMs: 2,
        },
      },
    });

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));

    const line = await screen.findByTestId('research-decision-trace');
    expect(line).toHaveTextContent('rule');
    expect(line).toHaveTextContent('现算');
    expect(line).toHaveTextContent('local.ollama✗');
    expect(line).toHaveTextContent('rule✓');
  });

  // §9.7「运行中可随时中止」：忙碌时中止按钮必须仍可点（否则只能等这一步跑完）。
  it('步骤进行中「中止」仍可点击，并调用后端中止', async () => {
    vi.spyOn(researchService, 'start').mockResolvedValue(planned());
    vi.spyOn(researchService, 'collect').mockImplementation(() => new Promise(() => {}));
    const abort = vi.spyOn(researchService, 'abort').mockResolvedValue({
      ...planned(),
      status: 'aborted',
      abortReason: '用户中止',
    });

    renderPage();
    await userEvent.type(await screen.findByTestId('research-topic'), 'Rust 异步运行时调度');
    await userEvent.click(screen.getByTestId('research-start'));
    await userEvent.click(await screen.findByTestId('research-collect'));

    const button = screen.getByTestId('research-abort');
    expect(button).toBeEnabled();
    await userEvent.click(button);

    await waitFor(() => expect(abort).toHaveBeenCalledWith('research_1', '用户中止'));
  });
});
