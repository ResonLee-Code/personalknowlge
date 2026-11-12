import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import { toast } from 'sonner';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { ContentDiscoveryPage } from '@/features/discover/ContentDiscoveryPage';
import { aiService } from '@/services/aiService';
import * as api from '@/services/api';
import { contentDiscoveryService } from '@/services/contentDiscoveryService';
import { useCategoriesStore } from '@/stores/categoriesStore';
import type { AiPolicy, ContentCapture } from '@/types';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

const policy: AiPolicy = {
  enabled: true,
  network: 'allowNetwork',
  networkConfirmedAt: '2026-10-25T01:00:00+08:00',
  allowFetchBody: true,
  domainAllowlist: [],
  domainDenylist: [],
  localModelEndpoint: null,
  localModelName: null,
  localModelCompose: false,
  localModelScore: false,
  remoteModelBaseUrl: 'https://api.deepseek.com',
  remoteModelName: 'deepseek-flash',
  remoteModelCompose: true,
  remoteModelScore: false,
  remoteModelAuthorizedAt: '2026-10-25T01:00:00+08:00',
  remoteModelKeySet: true,
  confirmEachOutbound: false,
  vectorEnabled: false,
  decisionProvider: 'system_one',
  decisionBaseUrl: 'https://api.perplexity.ai',
  decisionModel: 'pplx-decider-v1.1-27b',
  decisionAuthorizedAt: '2026-10-25T01:00:00+08:00',
  decisionKeySet: true,
  budget: { maxSubquestions: 5, maxPages: 12, maxDurationSecs: 480 },
  auditMaxEntries: 500,
};

function capture(): ContentCapture {
  return {
    id: 'capture_1',
    workspaceId: 'workspace_1',
    requestedUrl: 'https://example.com/article',
    finalUrl: 'https://example.com/article',
    canonicalUrl: 'https://example.com/article',
    sourceKind: 'web',
    title: 'Rust 异步调度指南',
    author: null,
    site: 'example.com',
    publishedAt: null,
    capturedAt: '2026-10-25T01:00:00+08:00',
    userNote: '用于调度方案整理',
    status: 'awaitingSelection',
    summary: '介绍 Rust 异步调度的主要策略。',
    keyPoints: ['工作窃取', '阻塞调用限制'],
    tags: ['网页发现'],
    relatedNoteIds: ['note_related'],
    relatedNotes: [{ noteId: 'note_related', title: '已有异步调度笔记' }],
    suggestedCategoryId: null,
    decisionProvider: 'system_one',
    keepFullText: false,
    pageCount: 1,
    wordCount: 420,
    cacheDir: '.knowledge/content-discovery/capture_1',
    error: null,
    noteId: null,
    duplicateOf: null,
    createdAt: '2026-10-25T01:00:00+08:00',
    updatedAt: '2026-10-25T01:00:00+08:00',
    savedAt: null,
    sourceChanged: false,
    lastRefreshedAt: null,
    candidates: [
      {
        url: 'https://example.com/guide',
        canonicalUrl: 'https://example.com/guide',
        parentUrl: 'https://example.com/article',
        depth: 1,
        title: '调度指南',
        anchorText: '调度指南',
        context: '相关链接',
        hidden: false,
        sameOrigin: true,
        sameSite: true,
        eligible: true,
        blockedReason: null,
        relevanceScore: 0.8,
        relevanceReason: '直接相关',
        selected: false,
      },
      {
        url: 'https://example.com/hidden',
        canonicalUrl: 'https://example.com/hidden',
        parentUrl: 'https://example.com/article',
        depth: 1,
        title: '隐藏资料',
        anchorText: '隐藏资料',
        context: '折叠内容',
        hidden: true,
        sameOrigin: true,
        sameSite: true,
        eligible: false,
        blockedReason: 'hidden',
        relevanceScore: null,
        relevanceReason: null,
        selected: false,
      },
    ],
    pages: [],
    markdown: '# Rust 异步调度指南\n\n正文内容。',
  };
}

function renderPage(initialEntry = '/discover') {
  return render(
    <MemoryRouter initialEntries={[initialEntry]}>
      <ContentDiscoveryPage />
    </MemoryRouter>,
  );
}

describe('ContentDiscoveryPage', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(aiService, 'policy').mockResolvedValue(policy);
    vi.spyOn(contentDiscoveryService, 'list').mockResolvedValue([]);
    useCategoriesStore.setState({ flat: [], tree: [], loading: false, error: null });
    vi.spyOn(useCategoriesStore.getState(), 'refresh').mockResolvedValue();
  });

  it('联网确认后才创建内容并显示候选链接', async () => {
    const created = capture();
    const create = vi.spyOn(contentDiscoveryService, 'create').mockResolvedValue(created);
    vi.spyOn(contentDiscoveryService, 'get').mockResolvedValue(created);
    renderPage();

    await userEvent.type(
      await screen.findByTestId('content-discovery-url'),
      'https://example.com/article',
    );
    await userEvent.click(screen.getByTestId('content-discovery-create'));
    expect(await screen.findByTestId('content-discovery-network-confirm')).toBeInTheDocument();
    expect(create).not.toHaveBeenCalled();

    await userEvent.click(screen.getByTestId('content-discovery-network-confirm-ok'));
    await waitFor(() =>
      expect(create).toHaveBeenCalledWith(
        expect.objectContaining({
          url: 'https://example.com/article',
          confirmed: true,
        }),
      ),
    );
    const detail = await screen.findByTestId('content-discovery-detail');
    expect(detail).toHaveTextContent('Rust 异步调度指南');
    expect(detail).toHaveTextContent('内容摘要');
    expect(screen.getByRole('link', { name: '已有异步调度笔记' })).toHaveAttribute(
      'href',
      '/notes/note_related',
    );
  });

  it('默认只允许普通候选，勾选后才提交深抓', async () => {
    const created = capture();
    vi.spyOn(contentDiscoveryService, 'list').mockResolvedValue([created]);
    vi.spyOn(contentDiscoveryService, 'get').mockResolvedValue(created);
    const crawled = {
      ...created,
      status: 'ready' as const,
      pageCount: 2,
      pages: [
        {
          url: created.finalUrl,
          canonicalUrl: created.canonicalUrl,
          parentUrl: null,
          depth: 0,
          title: created.title,
          site: created.site,
          contentHash: 'root',
          wordCount: 420,
          bytes: 1000,
          cachePath: '',
          status: 'ready',
          fetchedAt: created.capturedAt,
          error: null,
        },
      ],
    };
    const crawl = vi.spyOn(contentDiscoveryService, 'crawlSelected').mockResolvedValue(crawled);
    renderPage();

    const candidates = await screen.findAllByTestId('content-candidate');
    expect(candidates).toHaveLength(2);
    expect(candidates[1]).toBeDisabled();
    await userEvent.click(candidates[0]!);
    await userEvent.click(screen.getByTestId('content-discovery-crawl'));
    await userEvent.click(screen.getByTestId('content-discovery-network-confirm-ok'));

    await waitFor(() =>
      expect(crawl).toHaveBeenCalledWith(
        expect.objectContaining({
          captureId: 'capture_1',
          selectedUrls: ['https://example.com/guide'],
          confirmed: true,
        }),
      ),
    );
  });

  it('开启子域名时仍不允许跨站候选通过', async () => {
    const created = capture();
    created.candidates.push({
      ...created.candidates[0]!,
      url: 'https://other.example/article',
      canonicalUrl: 'https://other.example/article',
      title: '外部站点文章',
      sameOrigin: false,
      sameSite: false,
      blockedReason: 'external',
    });
    vi.spyOn(contentDiscoveryService, 'list').mockResolvedValue([created]);
    vi.spyOn(contentDiscoveryService, 'get').mockResolvedValue(created);
    renderPage();

    const candidates = await screen.findAllByTestId('content-candidate');
    expect(candidates[2]).toBeDisabled();
    await userEvent.click(screen.getByRole('switch', { name: '包含子域名' }));
    expect(candidates[2]).toBeDisabled();
  });

  it('从 URL Scheme 参数预填地址和保存备注', async () => {
    renderPage(
      '/discover?url=https%3A%2F%2Fexample.com%2Fshared&note=%E7%A8%8D%E5%90%8E%E9%98%85%E8%AF%BB',
    );

    expect(await screen.findByTestId('content-discovery-url')).toHaveValue(
      'https://example.com/shared',
    );
    expect(screen.getByTestId('content-discovery-note')).toHaveValue('稍后阅读');
  });

  it('允许在入库前调整建议标签', async () => {
    const created = capture();
    vi.spyOn(contentDiscoveryService, 'list').mockResolvedValue([created]);
    vi.spyOn(contentDiscoveryService, 'get').mockResolvedValue(created);
    const save = vi.spyOn(contentDiscoveryService, 'save').mockResolvedValue({
      captureId: created.id,
      noteId: 'note_1',
      noteTitle: created.title,
    });
    renderPage();

    await userEvent.click(await screen.findByTestId('content-discovery-save-open'));
    const tags = await screen.findByLabelText('标签（逗号分隔）');
    await userEvent.clear(tags);
    await userEvent.type(tags, 'Rust, 调度');
    await userEvent.click(screen.getByRole('switch', { name: '保存完整抓取正文' }));
    await userEvent.click(screen.getByTestId('content-discovery-save'));

    await waitFor(() =>
      expect(save).toHaveBeenCalledWith(
        expect.objectContaining({
          captureId: created.id,
          tags: ['Rust', '调度'],
          keepFullText: true,
        }),
      ),
    );
    await waitFor(() => expect(contentDiscoveryService.get).toHaveBeenCalled());
  });

  it('检查来源更新只刷新预览并明确提示不覆盖笔记', async () => {
    const created = capture();
    vi.spyOn(contentDiscoveryService, 'list').mockResolvedValue([created]);
    vi.spyOn(contentDiscoveryService, 'get').mockResolvedValue(created);
    const refresh = vi.spyOn(contentDiscoveryService, 'refresh').mockResolvedValue({
      ...created,
      sourceChanged: true,
      lastRefreshedAt: '2026-10-25T03:00:00+08:00',
    });
    renderPage();

    await userEvent.click(await screen.findByTestId('content-discovery-refresh'));
    await userEvent.click(screen.getByTestId('content-discovery-network-confirm-ok'));

    await waitFor(() => expect(refresh).toHaveBeenCalledWith(created.id, true));
    expect(screen.getByText(/正式笔记没有被自动覆盖/)).toBeInTheDocument();
  });

  it('明确显示未入库状态并允许查看抓取正文', async () => {
    const created = capture();
    vi.spyOn(contentDiscoveryService, 'list').mockResolvedValue([created]);
    vi.spyOn(contentDiscoveryService, 'get').mockResolvedValue(created);
    renderPage();

    expect(await screen.findByTestId('content-discovery-save-state')).toHaveTextContent(
      '尚未写入知识库',
    );
    expect(screen.getByTestId('content-discovery-preview-content')).toHaveTextContent('正文内容');
    await userEvent.click(screen.getByTestId('content-discovery-preview-toggle'));
    expect(screen.queryByTestId('content-discovery-preview-content')).not.toBeInTheDocument();
  });

  it('未开启联网时禁用创建并给出设置入口', async () => {
    vi.spyOn(aiService, 'policy').mockResolvedValue({
      ...policy,
      network: 'localOnly',
      networkConfirmedAt: null,
    });
    renderPage();

    expect(await screen.findByTestId('content-discovery-create')).toBeDisabled();
    expect(screen.getByRole('link', { name: '设置 → AI → 联网与增强' })).toHaveAttribute(
      'href',
      '/settings',
    );
  });

  it('空地址不会打开联网确认', async () => {
    renderPage();

    await userEvent.click(await screen.findByTestId('content-discovery-create'));
    expect(toast.info).toHaveBeenCalledWith('请先输入网页 URL 或分享文本。');
    expect(screen.queryByTestId('content-discovery-network-confirm')).not.toBeInTheDocument();
  });

  it('收件箱读取失败时保留页面并显示明确错误', async () => {
    vi.spyOn(aiService, 'policy').mockRejectedValue(new Error('策略服务不可用'));
    renderPage();

    expect(await screen.findByText(/读取内容收件箱失败：策略服务不可用/)).toBeInTheDocument();
    expect(screen.getByTestId('content-discovery-create')).toBeDisabled();
  });

  it('已入库、来源更新与失败页面都有明确状态', async () => {
    const saved = {
      ...capture(),
      status: 'saved' as const,
      noteId: 'note_saved',
      savedAt: '2026-10-25T03:00:00+08:00',
      sourceChanged: true,
      suggestedCategoryId: 'cat_1',
      userNote: null,
      pages: [
        {
          url: 'https://example.com/article',
          canonicalUrl: 'https://example.com/article',
          parentUrl: null,
          depth: 0,
          title: 'Rust 异步调度指南',
          site: 'example.com',
          contentHash: 'root',
          wordCount: 420,
          bytes: 1000,
          cachePath: 'root.md',
          status: 'ready',
          fetchedAt: '2026-10-25T03:00:00+08:00',
          error: null,
        },
        {
          url: 'https://example.com/broken',
          canonicalUrl: 'https://example.com/broken',
          parentUrl: 'https://example.com/article',
          depth: 1,
          title: '损坏页面',
          site: 'example.com',
          contentHash: '',
          wordCount: 0,
          bytes: 0,
          cachePath: '',
          status: 'failed',
          fetchedAt: '2026-10-25T03:00:00+08:00',
          error: 'HTTP 500',
        },
      ],
    };
    vi.spyOn(contentDiscoveryService, 'list').mockResolvedValue([saved]);
    vi.spyOn(contentDiscoveryService, 'get').mockResolvedValue(saved);
    renderPage();

    expect(await screen.findByText('已写入知识库。')).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '打开对应笔记' })).toHaveAttribute(
      'href',
      '/notes/note_saved',
    );
    expect(screen.getByText(/来源正文已变化/)).toHaveTextContent('正式笔记没有被自动覆盖');
    expect(screen.getByText('已抓取页面')).toBeInTheDocument();
    expect(screen.getByText('失败')).toBeInTheDocument();
    expect(screen.getByText(/已匹配分类/)).toBeInTheDocument();
  });

  it('忽略内容后保持收件箱状态且不再提供写入按钮', async () => {
    const created = capture();
    vi.spyOn(contentDiscoveryService, 'list').mockResolvedValue([created]);
    vi.spyOn(contentDiscoveryService, 'get').mockResolvedValue(created);
    vi.spyOn(contentDiscoveryService, 'ignore').mockResolvedValue({
      ...created,
      status: 'ignored',
    });
    renderPage();

    await userEvent.click(await screen.findByTestId('content-discovery-ignore'));
    expect(await screen.findByText('已忽略，未写入知识库。')).toBeInTheDocument();
    expect(screen.getByTestId('content-discovery-save-open')).toBeDisabled();
    expect(toast.info).toHaveBeenCalledWith('已忽略这条内容');
  });

  it('隐藏候选需显式开启，阻断原因可解释', async () => {
    const created = capture();
    created.candidates.push({
      ...created.candidates[0]!,
      url: 'https://example.com/outside',
      canonicalUrl: 'https://example.com/outside',
      title: '路径外资料',
      hidden: false,
      eligible: false,
      blockedReason: 'outsidePath',
    });
    vi.spyOn(contentDiscoveryService, 'list').mockResolvedValue([created]);
    vi.spyOn(contentDiscoveryService, 'get').mockResolvedValue(created);
    renderPage();

    const candidates = await screen.findAllByTestId('content-candidate');
    expect(candidates[1]).toBeDisabled();
    expect(screen.getByText('隐藏/折叠链接，需要开启选项')).toBeInTheDocument();
    expect(screen.getByText('不在当前路径子树')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('switch', { name: '包含隐藏/折叠链接' }));
    expect(candidates[1]).not.toBeDisabled();
  });

  it('抓取进度到达后显示当前页并在完成后刷新汇总', async () => {
    const created = capture();
    let progressHandler:
      | ((payload: {
          captureId: string;
          status: 'fetching' | 'ready';
          completed: number;
          total: number;
          currentUrl?: string | null;
        }) => void)
      | undefined;
    vi.spyOn(api, 'subscribe').mockImplementation(async (_event, handler) => {
      progressHandler = handler as typeof progressHandler;
      return () => {};
    });
    const list = vi
      .spyOn(contentDiscoveryService, 'list')
      .mockResolvedValueOnce([created])
      .mockResolvedValue([{ ...created, status: 'ready' }]);
    vi.spyOn(contentDiscoveryService, 'get').mockResolvedValue(created);
    renderPage();

    await screen.findByTestId('content-discovery-detail');
    progressHandler?.({
      captureId: created.id,
      status: 'fetching',
      completed: 1,
      total: 3,
      currentUrl: 'https://example.com/guide',
    });
    expect(await screen.findByText(/正在抓取 1\/3/)).toHaveTextContent('https://example.com/guide');
    progressHandler?.({
      captureId: created.id,
      status: 'ready',
      completed: 3,
      total: 3,
    });
    await waitFor(() => expect(list).toHaveBeenCalledTimes(2));
  });
});
