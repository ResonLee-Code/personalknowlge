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
  /**
   * 单条用例 90 秒：主流程有 30+ 步，机器忙碌时需要余量。
   *
   * 断言超时放到 10 秒（默认 5 秒）：测试驱动的是 `vite preview` 的**冷启动**产物，
   * 首次访问要拉 ~500KB 的 bundle + 初始化内存后端，5 秒偶发不够（表现为
   * 「第一个断言就超时」，与功能无关）。放宽的是**等待**，不是断言本身——
   * 界面真的坏了仍然会失败。
   */
  timeout: 90_000,
  expect: { timeout: 10_000 },
  /**
   * 失败重试 1 次：这条用例驱动的是**冷启动的预览构建**，包含力导向布局收敛、
   * 拖拽与跨页面跳转，对机器负载敏感（同一份代码在空载与满载下耗时差数倍）。
   * 重试只用来吸收这类时序抖动；真正的功能回归由 Rust 集成测试（29 个目标）
   * 与前端 217 个用例把关，重试后仍失败就是真问题。
   */
  retries: 1,
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
