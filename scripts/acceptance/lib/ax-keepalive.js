#!/usr/bin/env node
'use strict';

/**
 * AX 保活轮询（2026-10-14 实测发现）。
 *
 * **现象**：应用跑在 WKWebView 里，WebKit 只在"有 AX 客户端在看"的时候才维护无障碍树。
 * 脚本隔几秒查一次的话，树会**塌掉**——`window.entireContents()` 只剩窗口边框三个按钮，
 * 界面上的输入框/按钮一个都读不到（现象就是"没找到元素"，而屏幕上明明有）。
 *
 * **办法**：真跑期间起一个后台进程，**每隔一段时间**轻量查一次（`frame` 空查询），
 * WebKit 就会保持无障碍树可用；跑完由 `run.js` 杀掉。
 *
 * ⚠️ 间隔不能太密：System Events 处理查询是**串行**的，而一次 `entireContents()`
 * 要 ~1 秒；400ms 轮询会把 System Events 占满，反而让**真正要用的查询**查不到元素
 * （实测：真跑里 15 次点击全部退化成"按坐标点"，只有少数恰好落在按钮上）。
 * 默认 1500ms，可用 `PKOS_KEEPALIVE_MS` 调；**脚本执行步骤时会通过忙碌锁让它完全停下**
 * （`PKOS_AX_LOCK`）。
 *
 * 这不是"为了让测试变绿"的 hack：没有它，脚本读到的是一棵**不存在的树**，
 * 结论表会把"读不到界面"写成"功能有问题"——那是谎报。
 */

const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const mac = require('./mac');

const appName = process.argv[2];
if (!appName) {
  process.stderr.write('用法：ax-keepalive.js <应用名>\n');
  process.exit(2);
}

let stopped = false;
const stop = () => {
  stopped = true;
  process.exit(0);
};
process.on('SIGTERM', stop);
process.on('SIGINT', stop);

const intervalMs = Number(process.env.PKOS_KEEPALIVE_MS || 1500);
/** 与 run.js 约定的"忙碌锁"路径：脚本正在查元素时，保活让路（见下）。 */
const lockPath = process.env.PKOS_AX_LOCK || path.join(os.tmpdir(), 'pkos-ax-busy.lock');

function tick() {
  if (stopped) return;
  try {
    // **让路**：脚本正在执行一个步骤（要自己查元素）时不要插队——
    // System Events 是串行的，两边同时查会互相拖垮，实测会让步骤查不到元素
    // 从而退化成"按坐标点"（这个 WebView 的坐标点击并不总是有效）。
    if (fs.existsSync(lockPath)) {
      setTimeout(tick, 300);
      return;
    }
    // 空查询 = 拿到树里的第一个元素，只为"poke"一次，不关心结果
    mac.frameOf(appName, {});
  } catch {
    // 应用退出 / 树暂时不可用都不影响保活本身
  }
  setTimeout(tick, intervalMs);
}

tick();
