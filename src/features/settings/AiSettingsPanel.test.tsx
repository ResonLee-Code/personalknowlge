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

  it('本地模型默认不启用：地址/模型名为空、两个开关都是关的（D5）', async () => {
    render(<AiSettingsPanel />);

    expect(await screen.findByTestId('ai-local-endpoint')).toHaveValue('');
    expect(screen.getByTestId('ai-local-model')).toHaveValue('');
    expect(screen.getByTestId('ai-local-compose')).not.toBeChecked();
    expect(screen.getByTestId('ai-local-score')).not.toBeChecked();
    // 提示里说明「只连本机」「不会真的调用」
    expect(screen.getByText(/只连本机地址/)).toBeInTheDocument();
  });

  it('本地模型开关与模型名分别提交（失败降级由后端负责）', async () => {
    // 回显输入：开关是受控组件，测试要看到「点一下就提交真值」而不是固定返回值
    const setPolicy = vi
      .spyOn(aiService, 'setPolicy')
      .mockImplementation(async (input) => ({ ...conservative, enabled: true, ...input }));
    render(<AiSettingsPanel />);

    await userEvent.type(
      await screen.findByTestId('ai-local-endpoint'),
      'http://127.0.0.1:11434',
    );
    await userEvent.tab();
    await waitFor(() =>
      expect(setPolicy).toHaveBeenCalledWith({ localModelEndpoint: 'http://127.0.0.1:11434' }),
    );

    await userEvent.click(screen.getByTestId('ai-local-compose'));
    await waitFor(() => expect(setPolicy).toHaveBeenCalledWith({ localModelCompose: true }));
    await userEvent.click(screen.getByTestId('ai-local-score'));
    await waitFor(() => expect(setPolicy).toHaveBeenCalledWith({ localModelScore: true }));
  });

  it('在线模型默认不启用：地址/模型名为空、两个开关关着、Key 未配置', async () => {
    render(<AiSettingsPanel />);

    expect(await screen.findByTestId('ai-remote-endpoint')).toHaveValue('');
    expect(screen.getByTestId('ai-remote-model')).toHaveValue('');
    expect(screen.getByTestId('ai-remote-compose')).not.toBeChecked();
    expect(screen.getByTestId('ai-remote-score')).not.toBeChecked();
    expect(screen.getByTestId('ai-remote-key-badge')).toHaveTextContent('未配置 Key');
    expect(screen.getByTestId('ai-remote-authorized')).toHaveTextContent('尚未授权');
    // 预览/桌面同一口径：说明里讲清「本机可写 localhost」「裸 IP 会被拒绝」
    expect(screen.getByText(/裸 IP 与非加密地址会被拒绝/)).toBeInTheDocument();
  });

  it('在线模型地址与模型名分别提交（丢失规范化由后端负责）', async () => {
    const setPolicy = vi
      .spyOn(aiService, 'setPolicy')
      .mockImplementation(async (input) => ({ ...conservative, enabled: true, ...input }));
    render(<AiSettingsPanel />);

    await userEvent.type(
      await screen.findByTestId('ai-remote-endpoint'),
      'https://api.example.com/v1',
    );
    await userEvent.tab();
    await waitFor(() =>
      expect(setPolicy).toHaveBeenCalledWith({ remoteModelBaseUrl: 'https://api.example.com/v1' }),
    );

    await userEvent.type(screen.getByTestId('ai-remote-model'), 'deepseek-chat');
    await userEvent.tab();
    await waitFor(() =>
      expect(setPolicy).toHaveBeenCalledWith({ remoteModelName: 'deepseek-chat' }),
    );
  });

  /**
   * API Key 是「只写不读」：输入框保存后必须立刻清空，界面只回显「已配置 / 未配置」。
   * 如果这里把 Key 留在输入框里，截图、录屏、共享屏幕都会把它带出去。
   */
  it('API Key 只写不读：保存后清空输入框、只回显「已配置」', async () => {
    const setPolicy = vi.spyOn(aiService, 'setPolicy').mockImplementation(async (input) => ({
      ...conservative,
      enabled: true,
      ...input,
      // 后端不会回显 Key，只会把 keySet 置为真
      remoteModelApiKey: undefined,
      remoteModelKeySet: (input.remoteModelApiKey ?? '').trim().length > 0,
    }));
    render(<AiSettingsPanel />);

    const keyInput = await screen.findByTestId('ai-remote-key');
    await userEvent.type(keyInput, 'sk-secret-value');
    await userEvent.click(screen.getByTestId('ai-remote-key-save'));

    await waitFor(() =>
      expect(setPolicy).toHaveBeenCalledWith({ remoteModelApiKey: 'sk-secret-value' }),
    );
    expect(keyInput).toHaveValue('');
    expect(await screen.findByTestId('ai-remote-key-badge')).toHaveTextContent('已配置 Key');
    expect(screen.queryByDisplayValue('sk-secret-value')).not.toBeInTheDocument();

    await userEvent.click(screen.getByTestId('ai-remote-key-clear'));
    await waitFor(() => expect(setPolicy).toHaveBeenCalledWith({ remoteModelApiKey: '' }));
    expect(await screen.findByTestId('ai-remote-key-badge')).toHaveTextContent('未配置 Key');
  });

  it('打开「用在线模型写报告」先弹授权说明，确认后才提交开关', async () => {
    const setPolicy = vi
      .spyOn(aiService, 'setPolicy')
      .mockImplementation(async (input) => ({
        ...conservative,
        enabled: true,
        ...input,
        // 后端在收到确认后写入授权时间戳（界面据此显示「已授权」）
        remoteModelAuthorizedAt: input.confirmRemoteModel
          ? '2026-10-11T10:00:00+08:00'
          : conservative.remoteModelAuthorizedAt,
      }));
    render(<AiSettingsPanel />);

    await userEvent.click(await screen.findByTestId('ai-remote-compose'));
    // 未授权：只弹说明，不提交
    expect(await screen.findByTestId('ai-remote-confirm')).toBeInTheDocument();
    expect(setPolicy).not.toHaveBeenCalled();

    await userEvent.click(screen.getByTestId('ai-remote-confirm-ok'));
    await waitFor(() =>
      expect(setPolicy).toHaveBeenCalledWith({
        confirmRemoteModel: true,
        remoteModelCompose: true,
      }),
    );
    expect(await screen.findByTestId('ai-remote-compose')).toBeChecked();
    expect(screen.getByTestId('ai-remote-authorized')).toHaveTextContent('已授权外发');
  });

  it('取消授权对话框时保持关闭，一个字段都不提交', async () => {
    const setPolicy = vi.spyOn(aiService, 'setPolicy');
    render(<AiSettingsPanel />);

    await userEvent.click(await screen.findByTestId('ai-remote-score'));
    await userEvent.click(await screen.findByRole('button', { name: '暂不启用' }));

    expect(setPolicy).not.toHaveBeenCalled();
    expect(screen.getByTestId('ai-remote-score')).not.toBeChecked();
  });

  it('已授权后切换开关直接提交（不再重复弹窗）', async () => {
    vi.spyOn(aiService, 'policy').mockResolvedValue({
      ...conservative,
      enabled: true,
      remoteModelBaseUrl: 'https://api.example.com/v1',
      remoteModelName: 'deepseek-chat',
      remoteModelKeySet: true,
      remoteModelAuthorizedAt: '2026-10-11T10:00:00+08:00',
    });
    const setPolicy = vi
      .spyOn(aiService, 'setPolicy')
      .mockImplementation(async (input) => ({
        ...conservative,
        enabled: true,
        remoteModelBaseUrl: 'https://api.example.com/v1',
        remoteModelName: 'deepseek-chat',
        remoteModelKeySet: true,
        remoteModelAuthorizedAt: '2026-10-11T10:00:00+08:00',
        ...input,
      }));
    render(<AiSettingsPanel />);

    expect(await screen.findByTestId('ai-remote-authorized')).toHaveTextContent('已授权外发');
    await userEvent.click(screen.getByTestId('ai-remote-score'));

    await waitFor(() => expect(setPolicy).toHaveBeenCalledWith({ remoteModelScore: true }));
    expect(screen.queryByTestId('ai-remote-confirm')).not.toBeInTheDocument();
    expect(await screen.findByTestId('ai-remote-score')).toBeChecked();
  });

  it('读取策略失败时给出明确提示与「重试」，而不是一直显示正在读取', async () => {
    const policy = vi
      .spyOn(aiService, 'policy')
      .mockRejectedValueOnce({ code: 'NOT_INITIALIZED', message: '尚未打开任何工作区。' })
      .mockResolvedValueOnce(conservative);
    render(<AiSettingsPanel />);

    expect(await screen.findByText(/读取 AI 策略失败/)).toBeInTheDocument();
    expect(screen.getByText(/尚未打开任何工作区/)).toBeInTheDocument();
    expect(screen.queryByText('正在读取 AI 策略…')).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: '重试' }));
    await waitFor(() => expect(policy).toHaveBeenCalledTimes(2));
    expect(await screen.findByTestId('ai-enabled')).not.toBeChecked();
  });

  /**
   * 回归：审计读取失败以前被静默吞掉，界面显示成「还没有 AI 行为记录」——
   * 而这里正是用来看「谁在什么时候动过 AI」的地方，说成「没有记录」是在误导。
   */
  it('审计读取失败时明确报错，而不是显示成「还没有 AI 行为记录」', async () => {
    vi.spyOn(aiService, 'auditTail').mockRejectedValue({
      code: 'IO_ERROR',
      message: '审计文件不可读',
    });
    render(<AiSettingsPanel />);

    const error = await screen.findByTestId('ai-audit-error');
    expect(error).toHaveTextContent('审计文件不可读');
    expect(screen.queryByText('还没有 AI 行为记录。')).not.toBeInTheDocument();
  });
});
