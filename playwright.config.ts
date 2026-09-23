import { defineConfig } from '@playwright/test';

/**
 * 端到端验证（设计文档 22.3 UI 测试）。
 *
 * 运行前需先构建前端：`pnpm build`。
 * 测试通过内置的内存后端（浏览器预览模式）驱动真实界面，
 * 因此无需启动桌面外壳即可验证界面行为。
 */
export default defineConfig({
  testDir: './e2e',
  timeout: 60_000,
  fullyParallel: false,
  workers: 1,
  reporter: [['list']],
  use: {
    baseURL: 'http://127.0.0.1:4173',
    viewport: { width: 1440, height: 900 },
    locale: 'zh-CN',
    screenshot: 'off',
    trace: 'off',
  },
  webServer: {
    command: 'pnpm exec vite preview --port 4173 --strictPort --host 127.0.0.1',
    url: 'http://127.0.0.1:4173',
    reuseExistingServer: true,
    timeout: 60_000,
  },
});
