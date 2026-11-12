import { describe, expect, it, vi } from 'vitest';

import { contentDiscoveryService } from '@/services/contentDiscoveryService';

const { call } = vi.hoisted(() => ({ call: vi.fn() }));

vi.mock('./api', () => ({ call }));

describe('contentDiscoveryService', () => {
  it('把内容发现操作完整转发到 Tauri 命令', async () => {
    call.mockResolvedValue({});
    const input = {
      url: 'https://example.com/article',
      sharedText: null,
      userNote: '稍后整理',
      confirmed: true,
    };
    const runInput = {
      captureId: 'capture_1',
      selectedUrls: ['https://example.com/guide'],
      options: {
        maxDepth: 1,
        maxPages: 20,
        samePath: true,
        includeSubdomains: false,
        includeExternal: false,
        includeHidden: false,
        keepFullText: false,
      },
      confirmed: true,
    };
    const saveInput = {
      captureId: 'capture_1',
      title: '整理后的标题',
      categoryId: null,
      tags: ['Rust'],
      keepFullText: true,
    };

    await contentDiscoveryService.create(input);
    await contentDiscoveryService.list('ready', 10);
    await contentDiscoveryService.list();
    await contentDiscoveryService.get('capture_1');
    await contentDiscoveryService.crawlSelected(runInput);
    await contentDiscoveryService.refresh('capture_1', true);
    await contentDiscoveryService.save(saveInput);
    await contentDiscoveryService.ignore('capture_1');

    expect(call).toHaveBeenNthCalledWith(1, 'content_capture_create', { input });
    expect(call).toHaveBeenNthCalledWith(2, 'content_capture_list', {
      status: 'ready',
      limit: 10,
    });
    expect(call).toHaveBeenNthCalledWith(3, 'content_capture_list', {
      status: null,
      limit: 50,
    });
    expect(call).toHaveBeenNthCalledWith(4, 'content_capture_get', {
      captureId: 'capture_1',
    });
    expect(call).toHaveBeenNthCalledWith(5, 'content_discovery_run', { input: runInput });
    expect(call).toHaveBeenNthCalledWith(6, 'content_capture_refresh', {
      captureId: 'capture_1',
      confirmed: true,
    });
    expect(call).toHaveBeenNthCalledWith(7, 'content_capture_save', { input: saveInput });
    expect(call).toHaveBeenNthCalledWith(8, 'content_capture_ignore', {
      captureId: 'capture_1',
    });
  });
});
