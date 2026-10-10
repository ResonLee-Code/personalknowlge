import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AiSettingsPanel } from '@/features/settings/AiSettingsPanel';
import { aiService } from '@/services/aiService';
import type { AiPolicy } from '@/types';

vi.mock('sonner', () => ({
  toast: { success: vi.fn(), warning: vi.fn(), error: vi.fn(), info: vi.fn() },
}));

const conservative: AiPolicy = {
  enabled: false,
  network: 'localOnly',
  networkConfirmedAt: null,
  allowFetchBody: false,
  domainAllowlist: [],
  domainDenylist: [],
  budget: { maxSubquestions: 5, maxPages: 12, maxDurationSecs: 480 },
  auditMaxEntries: 500,
};

describe('AiSettingsPanel（V1.2.1 D1）', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(aiService, 'policy').mockResolvedValue(conservative);
    vi.spyOn(aiService, 'auditTail').mockResolvedValue([]);
  });

  it('默认展示最保守口径：总开关关闭、联网仅本地、抓取正文关闭', async () => {
    render(<AiSettingsPanel />);

    const enabled = await screen.findByTestId('ai-enabled');
    expect(enabled).not.toBeChecked();
    expect(screen.getByTestId('ai-network')).toHaveValue('localOnly');
    expect(screen.getByTestId('ai-fetch-body')).not.toBeChecked();
    expect(screen.getByTestId('ai-allowlist')).toHaveValue('');
    expect(screen.getByTestId('ai-budget-pages')).toHaveValue(12);
  });

  it('打开总开关会调用后端策略更新', async () => {
    const setPolicy = vi
      .spyOn(aiService, 'setPolicy')
      .mockResolvedValue({ ...conservative, enabled: true });
    render(<AiSettingsPanel />);

    await userEvent.click(await screen.findByTestId('ai-enabled'));
    await waitFor(() => expect(setPolicy).toHaveBeenCalledWith({ enabled: true }));
    expect(await screen.findByTestId('ai-enabled')).toBeChecked();
  });

  it('切到「允许联网检索」先弹说明，确认后才提交联网档位', async () => {
    const setPolicy = vi.spyOn(aiService, 'setPolicy').mockResolvedValue({
      ...conservative,
      enabled: true,
      network: 'allowNetwork',
      networkConfirmedAt: '2026-10-11T10:00:00+08:00',
    });
    render(<AiSettingsPanel />);

    await userEvent.selectOptions(await screen.findByTestId('ai-network'), 'allowNetwork');
    // 说明对话框出现，且此时**还没有**提交策略
    expect(await screen.findByTestId('ai-network-confirm')).toBeInTheDocument();
    expect(screen.getByText(/不会上传整篇知识库/)).toBeInTheDocument();
    expect(setPolicy).not.toHaveBeenCalled();

    await userEvent.click(screen.getByTestId('ai-network-confirm-ok'));
    await waitFor(() =>
      expect(setPolicy).toHaveBeenCalledWith({ network: 'allowNetwork', confirmNetwork: true }),
    );
  });

  it('取消说明对话框时保持「仅本地」', async () => {
    const setPolicy = vi.spyOn(aiService, 'setPolicy');
    render(<AiSettingsPanel />);

    await userEvent.selectOptions(await screen.findByTestId('ai-network'), 'allowNetwork');
    await userEvent.click(screen.getByRole('button', { name: '暂不启用' }));
    await waitFor(() => expect(screen.queryByTestId('ai-network-confirm')).not.toBeInTheDocument());
    expect(setPolicy).not.toHaveBeenCalled();
    expect(screen.getByTestId('ai-network')).toHaveValue('localOnly');
  });

  it('保存域名黑名单（一行一个）', async () => {
    const setPolicy = vi
      .spyOn(aiService, 'setPolicy')
      .mockResolvedValue({ ...conservative, domainDenylist: ['ads.example.com'] });
    render(<AiSettingsPanel />);

    const denylist = await screen.findByTestId('ai-denylist');
    await userEvent.type(denylist, 'ads.example.com');
    await userEvent.tab();
    await waitFor(() =>
      expect(setPolicy).toHaveBeenCalledWith({ domainDenylist: ['ads.example.com'] }),
    );
  });

  it('展示最近行为（含「被拒绝」与是否外发）', async () => {
    vi.spyOn(aiService, 'auditTail').mockResolvedValue([
      {
        at: '2026-10-11T10:00:00+08:00',
        capability: 'web_fetch',
        outbound: false,
        outcome: 'denied',
        durationMs: 0,
        note: 'NETWORK_DISABLED',
      },
    ]);
    render(<AiSettingsPanel />);

    const list = await screen.findByTestId('ai-audit-list');
    expect(list).toHaveTextContent('web_fetch');
    expect(list).toHaveTextContent('被拒绝');
    expect(list).toHaveTextContent('NETWORK_DISABLED');
  });

  it('检索源地址在失焦时保存（D3：应用不预置第三方检索服务）', async () => {
    const setPolicy = vi
      .spyOn(aiService, 'setPolicy')
      .mockResolvedValue({
        ...conservative,
        searchEndpoint: 'https://searx.example/search?q={query}&format=json',
      });
    render(<AiSettingsPanel />);

    const field = await screen.findByTestId('ai-search-endpoint');
    expect(field).toHaveValue('');
    // userEvent.type 会把 {query} 当成按键描述符，这里用 paste 传字面量
    await userEvent.click(field);
    await userEvent.paste('https://searx.example/search?q={query}&format=json');
    expect(field).toHaveValue('https://searx.example/search?q={query}&format=json');
    await userEvent.tab();

    await waitFor(() =>
      expect(setPolicy).toHaveBeenCalledWith({
        searchEndpoint: 'https://searx.example/search?q={query}&format=json',
      }),
    );
  });
});
