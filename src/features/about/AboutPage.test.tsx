import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { AboutPage } from '@/features/about/AboutPage';
import { appService } from '@/services/appService';

/**
 * 回归：`app_info` 读取失败以前被 `.catch(() => setInfo(null))` 吞掉，
 * 页面把所有字段显示成「—」。用户来这一页就是为了看版本号（提 issue 要用），
 * 只显示「—」不说原因，等于什么都没说。
 */
describe('AboutPage · 运行信息', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(appService, 'logPath').mockResolvedValue('/tmp/logs');
  });

  it('读取运行信息失败时说明原因，而不是只显示「—」', async () => {
    vi.spyOn(appService, 'info').mockRejectedValue({
      code: 'NOT_INITIALIZED',
      message: '尚未打开任何工作区。',
    });

    render(
      <MemoryRouter>
        <AboutPage />
      </MemoryRouter>,
    );

    expect(await screen.findByTestId('about-info-error')).toHaveTextContent('尚未打开任何工作区');
  });

  it('读取成功时不显示错误行', async () => {
    vi.spyOn(appService, 'info').mockResolvedValue({
      name: 'Personal Knowledge OS',
      version: '1.2.0',
      schemaVersion: 10,
      dataDir: '/tmp/data',
      platform: 'macos',
      sqliteVersion: '3.45',
    });

    render(
      <MemoryRouter>
        <AboutPage />
      </MemoryRouter>,
    );

    expect(await screen.findByText('1.2.0')).toBeInTheDocument();
    expect(screen.queryByTestId('about-info-error')).not.toBeInTheDocument();
  });
});
