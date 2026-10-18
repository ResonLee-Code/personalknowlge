'use strict';

const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const AGENT = path.join(__dirname, 'ax-agent.js');

const isMac = process.platform === 'darwin';

/** 调一次 macOS 代理（JXA）：参数是 JSON，返回值也是 JSON。 */
function agent(request) {
  if (!isMac) return { ok: false, error: '只支持 macOS' };
  let raw;
  try {
    raw = execFileSync('osascript', ['-l', 'JavaScript', AGENT, JSON.stringify(request)], {
      encoding: 'utf8',
      /**
       * 30 秒 → 90 秒（2026-10-17 实测）：验收库里笔记攒到 50+ 篇之后，
       * WebKit 的整棵树 `entireContents()` 一次要 17 秒（空库只要 1~6 秒），
       * 30 秒的超时会把"环境变慢"报成"功能失败"。这里给足余量，
       * 真正的卡死仍会因为超时而暴露。
       */
      timeout: 90_000,
    });
  } catch (error) {
    return { ok: false, error: `调 osascript 失败：${error.message}` };
  }
  const line = raw.trim().split('\n').filter(Boolean).pop();
  try {
    return JSON.parse(line);
  } catch {
    return { ok: false, error: `代理输出无法解析：${raw.trim()}` };
  }
}

/** 运行前自检：工具在不在、辅助功能权限有没有、应用跑没跑。 */
function preflight(appName) {
  const checks = [];
  checks.push({ name: '平台是 macOS', ok: isMac, detail: process.platform });
  if (!isMac) return checks;
  checks.push({ name: '有 osascript', ok: fs.existsSync('/usr/bin/osascript'), detail: '/usr/bin/osascript' });
  checks.push({
    name: '有 screencapture',
    ok: fs.existsSync('/usr/sbin/screencapture'),
    detail: '/usr/sbin/screencapture',
  });
  const trusted = agent({ op: 'trusted' });
  checks.push({
    name: '辅助功能权限',
    ok: trusted.ok && trusted.trusted === true,
    detail: trusted.ok
      ? trusted.trusted
        ? '已授权'
        : '未授权：系统设置 → 隐私与安全性 → 辅助功能里勾上「终端 / Codex」'
      : trusted.error,
  });
  const pid = agent({ op: 'pid', app: appName });
  checks.push({
    name: `应用在运行（${appName}）`,
    ok: pid.ok === true,
    detail: pid.ok ? `pid=${pid.pid}` : pid.error,
  });
  return checks;
}

function trusted() {
  const result = agent({ op: 'trusted' });
  return result.ok === true && result.trusted === true;
}

function pidOf(appName) {
  return agent({ op: 'pid', app: appName });
}

/** 把应用切到前台（CGEvent 点击必须落在最前面的窗口上）。 */
function activate(appName) {
  return agent({ op: 'activate', app: appName });
}

function frameOf(appName, query) {
  return agent({ op: 'frame', app: appName, query });
}

/**
 * 点击：`query` 给得出元素时优先 AXPress，否则用 `point` 投递真实鼠标事件。
 *
 * ⚠️ `app` 必须传：代理侧要按应用名找 System Events 的进程对象。
 * （2026-10-14 踩过：这里漏传，于是"查元素"永远失败 → 每次都退化成按坐标点，
 * 报告里看着是"✅ 通过"，其实什么都没点到。）
 */
function click(appName, point, query) {
  return agent({ op: 'click', app: appName, point, query });
}

/** 把元素设为焦点（给 Radix 菜单用：它们只认 pointerdown，AXPress 打不开）。 */
function focus(appName, query) {
  return agent({ op: 'focus', app: appName, query });
}

function drag(from, to) {
  return agent({ op: 'drag', from, to });
}

/** 悬停（只移动鼠标，不按键）：给"鼠标移上去才展开"的控件用（见 ax-agent 的说明）。 */
function hover(point, jitter) {
  return agent({ op: 'hover', point, jitter });
}

function scroll(point, ticks) {
  return agent({ op: 'scroll', point, ticks });
}

function keystroke(text, modifiers) {
  return agent({ op: 'keystroke', text, modifiers });
}

/** 粘贴文本（走剪贴板 + ⌘V）：中文输入不经过系统输入法，比 keystroke 稳。 */
function paste(text) {
  return agent({ op: 'paste', text });
}

/** 按虚拟键码（Escape=53 / Return=36 / Tab=48）：`keystroke` 送的是字符，不走这条路。 */
function keyCode(code, modifiers) {
  return agent({ op: 'keyCode', code, modifiers });
}

/** 截图（`-x` 不播放快门声）。目录不存在会先建。 */
function screenshot(file) {
  if (!isMac) return { ok: false, error: '只支持 macOS' };
  fs.mkdirSync(path.dirname(file), { recursive: true });
  try {
    execFileSync('/usr/sbin/screencapture', ['-x', file], { timeout: 20_000 });
    return { ok: true };
  } catch (error) {
    return { ok: false, error: error.message };
  }
}

module.exports = {
  isMac,
  agent,
  preflight,
  trusted,
  pidOf,
  activate,
  frameOf,
  click,
  focus,
  drag,
  hover,
  scroll,
  keystroke,
  paste,
  keyCode,
  screenshot,
};
