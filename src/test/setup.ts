import '@testing-library/jest-dom/vitest';
import { configure } from '@testing-library/react';

/**
 * `waitFor` / `findBy*` 的默认超时是 **1 秒**，而 `pnpm test` 会同时起 60+ 个 jsdom 环境
 * （每个文件一套），机器一忙，渲染 + 两次 state 更新就可能超过 1 秒 →
 * **偶发失败**（2026-10-14 发布前跑全套时碰到过一次：`HistoryDialog` 的 diff 断言）。
 *
 * 发布前的回归是"放行/不放行"的判据，宁可多等几秒，也不要让它靠运气。
 * 这里把异步工具的超时放宽到 5 秒；**测试自身的总超时**仍由 vitest 管（默认 5 秒，见 vite.config.ts）。
 */
configure({ asyncUtilTimeout: 5000 });

// jsdom 未实现 ResizeObserver：Radix 的 Popper（下拉 / 右键菜单 / tooltip）需要它
if (!('ResizeObserver' in window)) {
  class ResizeObserverStub {
    observe() {}
    unobserve() {}
    disconnect() {}
  }
  (window as unknown as { ResizeObserver: unknown }).ResizeObserver = ResizeObserverStub;
}

// jsdom 未实现 matchMedia，主题切换相关代码需要它
if (!window.matchMedia) {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}
