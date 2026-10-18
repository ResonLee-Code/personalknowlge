#!/usr/bin/env node
'use strict';

/**
 * E 节真机验收的**一条命令**入口（Track E1）。
 *
 *   node scripts/acceptance/run.js --list            # 看用例
 *   node scripts/acceptance/run.js --dry-run         # 只出结论表骨架（不碰界面）
 *   node scripts/acceptance/run.js --preflight       # 自检：工具 / 辅助功能权限 / 应用是否在跑
 *   node scripts/acceptance/run.js --only 3,4        # 只跑指定用例（调某一条时用）
 *   node scripts/acceptance/run.js                   # 真跑（需要应用在运行 + 辅助功能权限）
 *
 * 纪律：
 * - **没验的不写成通过**：dry-run 一律标「未执行」；主观手感标「待人工」；
 * - 截图与结论表都落在 `--out`（默认 `docs/acceptance/<时间戳>/`）；
 * - 脚本里不写任何私密信息（不读凭据、不进工作区正文）。
 */

const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const { spawn } = require('node:child_process');

const mac = require('./lib/mac');
const report = require('./lib/report');
const { APP_NAME, CASES, MANUAL_ONLY } = require('./cases');

const repoRoot = path.resolve(__dirname, '..', '..');

/** 与 `lib/ax-keepalive.js` 约定的忙碌锁：脚本查元素时让保活轮询停下（避免互相拖垮）。 */
const AX_LOCK = process.env.PKOS_AX_LOCK || path.join(os.tmpdir(), 'pkos-ax-busy.lock');
function withAxLock(run) {
  try {
    fs.writeFileSync(AX_LOCK, String(process.pid));
  } catch {
    // 锁写不进去也要继续跑：最坏情况只是退化成"两边抢 System Events"
  }
  try {
    return run();
  } finally {
    try {
      fs.unlinkSync(AX_LOCK);
    } catch {
      // 已经没了就算了
    }
  }
}

function parseArgs(argv) {
  const options = {
    list: false,
    dryRun: false,
    preflight: false,
    app: APP_NAME,
    workspace: process.env.PKOS_ACCEPTANCE_WORKSPACE || '/private/tmp/pkos-acceptance',
    out: null,
    only: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--list') options.list = true;
    else if (arg === '--dry-run') options.dryRun = true;
    else if (arg === '--preflight') options.preflight = true;
    else if (arg === '--app') options.app = argv[++index];
    else if (arg === '--workspace') options.workspace = argv[++index];
    else if (arg === '--out') options.out = argv[++index];
    // 只跑某几条用例（迭代某一条时不用把 9 条全跑一遍）：`--only 3,4`
    else if (arg === '--only') {
      options.only = String(argv[++index] || '')
        .split(',')
        .map((value) => Number(value.trim()))
        .filter((value) => Number.isFinite(value));
    }
    else if (arg === '--help' || arg === '-h') options.help = true;
  }
  return options;
}

function timestamp() {
  const now = new Date();
  const pad = (value) => String(value).padStart(2, '0');
  return `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
}

const MODIFIERS = {
  cmd: 'command down',
  command: 'command down',
  shift: 'shift down',
  alt: 'option down',
  opt: 'option down',
  ctrl: 'control down',
};

function resolvePoint(appName, target) {
  if (!target) return { ok: false, error: '缺少目标' };
  if (typeof target.x === 'number' && typeof target.y === 'number') {
    return { ok: true, point: { x: target.x, y: target.y } };
  }
  const found = mac.frameOf(appName, target);
  if (!found.ok) return { ok: false, error: found.error };
  const frame = found.frame;
  return { ok: true, point: { x: frame.x + frame.width / 2, y: frame.y + frame.height / 2 }, frame };
}

/**
 * 在目录里**递归**找文件名包含 `needle` 的文件（最多 3 层，返回相对名字 + 绝对路径）。
 *
 * 为什么要递归（2026-10-18）：附件按 `attachments/YYYY-MM/` 分月存放，
 * 只读顶层目录会得到"没有匹配的 .png"——那是**断言写错了**，不是没落盘。
 */
function findFilesRecursive(root, needle, depth = 3) {
  const found = [];
  const walk = (dir, prefix, level) => {
    let entries;
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const relative = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) {
        if (level < depth) walk(path.join(dir, entry.name), relative, level + 1);
      } else if (entry.name.includes(needle)) {
        found.push({ name: relative, absolute: path.join(dir, entry.name) });
      }
    }
  };
  walk(root, '', 0);
  return found;
}

/** 文件断言：`{path, exists, containsFile, grep}`（相对于工作区）。 */
function runCheck(workspace, step) {
  const target = path.join(workspace, step.path);
  if (step.exists) {
    if (!fs.existsSync(target)) return { ok: false, error: `不存在：${step.path}` };
    return { ok: true, evidence: `✓ ${step.path} 存在` };
  }
  if (step.notExists) {
    if (fs.existsSync(target)) return { ok: false, error: `本不该存在，却在：${step.path}` };
    return { ok: true, evidence: `✓ ${step.path} 不存在（符合预期）` };
  }
  if (step.containsFile) {
    if (!fs.existsSync(target)) return { ok: false, error: `不存在：${step.path}` };
    // 递归找（附件在 attachments/YYYY-MM/ 子目录里）
    const entries = findFilesRecursive(target, step.containsFile).map((entry) => entry.name);
    if (entries.length === 0) return { ok: false, error: `${step.path}（含子目录）下没有匹配「${step.containsFile}」的文件` };
    // 「必须有**刚刚生成**的文件」：只检查"目录里有匹配文件"太弱——
    // 上次验收留下的旧文件也能让断言通过，等于没验（备份那条就踩过）。
    if (step.modifiedWithinMinutes) {
      // 取**最新**的那个：目录里可能同时躺着上一轮几分钟内的产物，
      // 随便挑一个会让证据指向旧文件（看起来"验了"，其实验的是别人）。
      const newest = entries
        .map((name) => ({ name, mtime: fs.statSync(path.join(target, name)).mtimeMs }))
        .sort((left, right) => right.mtime - left.mtime)[0];
      if (!newest || Date.now() - newest.mtime > step.modifiedWithinMinutes * 60_000) {
        return {
          ok: false,
          error: `${step.path} 下的「${step.containsFile}」都不是最近 ${step.modifiedWithinMinutes} 分钟内生成的（可能是旧文件）`,
        };
      }
      // 证据里报**这一轮刚生成的那个文件**，而不是目录里第一个匹配项
      // （旧文件也能匹配子串，报旧文件等于让人以为验的是新的）
      return { ok: true, evidence: `✓ ${step.path}/${newest.name}（刚生成）` };
    }
    if (step.grep) {
      const hit = entries.some((name) => {
        const body = fs.readFileSync(path.join(target, name), 'utf8');
        return body.includes(step.grep);
      });
      if (!hit) return { ok: false, error: `匹配到的文件里没有「${step.grep}」` };
    }
    return { ok: true, evidence: `✓ ${step.path}/${entries[0]}` };
  }
  return { ok: false, error: '未知的断言类型' };
}

/** 睡一会儿（不 spin，也不起子进程）。 */
function sleepMs(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

/** 轮询等待某个元素出现（界面渲染需要时间，一次查不到不代表失败）。 */
function waitForFrame(appName, query, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let last = { ok: false, error: '没找到元素' };
  for (;;) {
    last = mac.frameOf(appName, query);
    if (last.ok) return last;
    if (Date.now() >= deadline) return last;
    sleepMs(250);
  }
}

function centerOf(frame) {
  return { x: frame.x + frame.width / 2, y: frame.y + frame.height / 2 };
}

/**
 * 现场准备：把应用切到前台，并确保**目标工作区已打开**。
 *
 * 为什么需要这一步（2026-10-14 首次真跑暴露的问题）：脚本原先假设"界面已经停在
 * 打开着工作区的主界面"。实际上一份刚启动的应用停在**欢迎页**，于是后面每条用例的
 * 第一次 `click` 全部失败——报告会写成"6 条失败"，把**现场没准备好**说成**功能有问题**。
 * 这正是本项目最忌讳的那种谎报，所以这里把它前置成一步，并如实记进报告。
 */
function ensureScene(appName, workspace) {
  const steps = [];
  const record = (op, status, detail) => steps.push({ op, status, detail, evidence: '' });
  const finish = (ok, detail) => {
    if (detail) record('scene', ok ? 'passed' : 'failed', detail);
    return { ok, steps };
  };

  const activated = mac.activate(appName);
  record(
    'activate',
    activated.ok ? 'passed' : 'failed',
    activated.ok ? '已把应用切到前台' : activated.error,
  );
  if (!activated.ok) return finish(false);

  // 清场：上一轮跑剩的弹层会把侧边栏挤出无障碍树（实测：弹层打开时树里只有弹层本身），
  // 于是"主界面 / 欢迎页"都认不出来。先按两次 Escape 把可能的弹层关掉。
  mac.keyCode(53);
  sleepMs(300);
  mac.keyCode(53);
  sleepMs(300);
  record('cleanup', 'passed', '按 Esc 清掉可能残留的弹层');

  // 主界面 = 有「新建笔记」；欢迎页 = 有「创建知识库」
  if (waitForFrame(appName, { role: 'AXButton', title: '新建笔记' }, 5000).ok) {
    return finish(true, '应用已在主界面：直接开始用例（若当前打开的不是目标工作区，文件断言会如实报错）');
  }
  if (!waitForFrame(appName, { role: 'AXButton', title: '创建知识库' }, 5000).ok) {
    return finish(false, '界面既不是主界面（找不到「新建笔记」）也不是欢迎页（找不到「创建知识库」）');
  }

  // 欢迎页：先在「最近打开」里**按路径**找目标工作区（按名字找会认错库）。
  // 实测（macOS 27）：「最近打开」的条目是 AXButton，标题形如
  // 「验收临时库 /private/tmp/pkos-acceptance 最近打开 2 天前」——**整条包含路径**，
  // 所以按路径子串匹配；不限定 role，避免不同版本换了控件类型就找不到。
  const recentQuery = { role: 'AXButton', title: workspace };
  const recent = mac.frameOf(appName, recentQuery).ok
    ? mac.frameOf(appName, recentQuery)
    : mac.frameOf(appName, { title: workspace });
  if (recent.ok) {
    mac.click(appName, centerOf(recent.frame), recentQuery);
    if (waitForFrame(appName, { role: 'AXButton', title: '新建笔记' }, 15000).ok) {
      return finish(true, `已从「最近打开」按路径打开工作区：${workspace}`);
    }
    return finish(false, `点了「最近打开」里的 ${workspace}，但主界面没有出现`);
  }

  if (fs.existsSync(workspace)) {
    return finish(
      false,
      `磁盘上有 ${workspace}，但它不在「最近打开」列表里：请在应用里手动打开一次该知识库再重跑` +
        '（脚本不走原生目录选择器，避免猜路径）',
    );
  }

  // 全新机器：直接建（欢迎页有「保存目录（绝对路径）」输入框，不必碰原生面板）
  const field = mac.frameOf(appName, { role: 'AXTextField', title: '例如' });
  if (!field.ok) return finish(false, '找不到「保存目录」输入框，无法自动创建知识库');
  mac.click(appName, centerOf(field.frame));
  mac.keystroke('a', ['command down']);
  mac.keystroke(workspace);
  const createQuery = { role: 'AXButton', title: '创建知识库' };
  const create = mac.frameOf(appName, createQuery);
  if (!create.ok) return finish(false, '找不到「创建知识库」按钮');
  mac.click(appName, centerOf(create.frame), createQuery);
  if (waitForFrame(appName, { role: 'AXButton', title: '新建笔记' }, 20000).ok) {
    return finish(true, `已创建并打开工作区：${workspace}`);
  }
  return finish(false, `创建 ${workspace} 之后主界面没有出现（可能路径已存在或创建失败）`);
}

function runCase(appName, workspace, outDir, item, dryRun) {
  const steps = [];
  /**
   * 每条用例开始时先把应用切前台（2026-10-15 实测教训）。
   *
   * 为什么：跑验收期间**别人的手会碰电脑**（这一次是我自己在旁边的窗口里打字/看图），
   * 前台被抢走之后，界面上的元素查不到、点击也可能落空——报告会写成"功能失败"，
   * 而其实只是**现场没保持住**。用例级切一次前台是最便宜的兜底（步骤级切太拖时间）。
   */
  if (!dryRun) mac.activate(appName);
  for (const step of item.steps) {
    const record = { op: step.op, status: 'skipped', detail: '', evidence: '' };
    if (dryRun) {
      record.detail = '（dry-run：未执行）';
      steps.push(record);
      continue;
    }
    const execute = () => {
      /**
       * 键盘类步骤（粘贴 / 快捷键 / 按键码）必须先确认应用在前台。
       *
       * 为什么：`AXPress` 点击与前台无关，但**键盘事件只送给最前面的应用**。
       * 2026-10-14 实测踩过：跑验收途中 Chrome 抢到前台，于是 `paste` 打进了 Chrome，
       * 应用里的标题是空的、后续断言全崩（18 步失败）——而报告里每个步骤都写着 ✅。
       * 这里的教训与「步骤通过 ≠ 动作生效」是同一条。
       */
      if (step.op === 'paste' || step.op === 'keys' || step.op === 'keyCode' || step.op === 'type') {
        mac.activate(appName);
      }
      switch (step.op) {
        case 'app': {
          const pid = mac.pidOf(appName);
          record.status = pid.ok ? 'passed' : 'failed';
          record.detail = pid.ok ? `app=${appName} pid=${pid.pid}` : pid.error;
          break;
        }
        case 'click': {
          const point = resolvePoint(appName, step.frame);
          if (!point.ok) {
            record.status = 'failed';
            record.detail = point.error;
            break;
          }
          const clicked = mac.click(appName, point.point, step.frame);
          record.status = clicked.ok ? 'passed' : 'failed';
          const why = clicked.pressError
            ? `（AXPress 失败：${clicked.pressError}）`
            : clicked.elementFound === false
              ? '（没找到元素，按坐标点）'
              : '';
          record.detail = clicked.ok
            ? `click(${Math.round(point.point.x)},${Math.round(point.point.y)}) · ${clicked.method ?? 'CGEvent'}${why}`
            : clicked.error;
          break;
        }
        /**
         * 聚焦某个元素（2026-10-18）。
         * Radix 的下拉菜单触发按钮只认 `pointerdown`，AXPress 的 `click` 打不开它；
         * 但触发按钮可聚焦，聚焦后回车/空格就按它的键盘逻辑展开（键盘用户的真实路径）。
         */
        case 'focus': {
          const focused = mac.focus(appName, step.query);
          record.status = focused.ok ? 'passed' : 'failed';
          record.detail = focused.ok
            ? `已聚焦：${step.label || JSON.stringify(step.query)}`
            : focused.error;
          break;
        }
        case 'drag': {
          const from = resolvePoint(appName, step.from);
          if (!from.ok) {
            record.status = 'failed';
            record.detail = from.error;
            break;
          }
          const to = step.to.dx
            ? { x: from.point.x + step.to.dx, y: from.point.y + step.to.dy }
            : resolvePoint(appName, step.to).point;
          const dragged = mac.drag(from.point, to);
          record.status = dragged.ok ? 'passed' : 'failed';
          record.detail = `drag(${Math.round(from.point.x)},${Math.round(from.point.y)}) → (${Math.round(to.x)},${Math.round(to.y)})`;
          break;
        }
        case 'scroll': {
          const point = resolvePoint(appName, step.target);
          if (!point.ok) {
            record.status = 'failed';
            record.detail = point.error;
            break;
          }
          const scrolled = mac.scroll(point.point, step.ticks || 3);
          record.status = scrolled.ok ? 'passed' : 'failed';
          record.detail = `scroll(${step.ticks || 3})`;
          break;
        }
        /**
         * 悬停到某个元素上（鼠标移上去才展开的控件）：见 lib/ax-agent.js 的 hover 说明。
         * 悬停前先把应用切前台：不属于"键盘步骤"的那套自动前台处理，但后台窗口上的悬停
         * 在实测里也不一定被 WebKit 当回事（2026-10-15）。
         */
        case 'hover': {
          mac.activate(appName);
          const point = resolvePoint(appName, step.target);
          if (!point.ok) {
            record.status = 'failed';
            record.detail = point.error;
            break;
          }
          const hovered = mac.hover(point.point, step.jitter);
          record.status = hovered.ok ? 'passed' : 'failed';
          record.detail = hovered.ok
            ? `hover(${Math.round(point.point.x)},${Math.round(point.point.y)})`
            : hovered.error;
          break;
        }
        case 'keys': {
          const parts = String(step.combo).split('+');
          const key = parts.pop();
          const modifiers = parts.map((part) => MODIFIERS[part.trim()]).filter(Boolean);
          const sent = mac.keystroke(key, modifiers);
          record.status = sent.ok ? 'passed' : 'failed';
          record.detail = step.combo;
          break;
        }
        case 'type': {
          const sent = mac.keystroke(step.text);
          record.status = sent.ok ? 'passed' : 'failed';
          record.detail = '输入了一小段文本（不落进报告）';
          break;
        }
        case 'paste': {
          const sent = mac.paste(step.text);
          record.status = sent.ok ? 'passed' : 'failed';
          record.detail = '粘贴了一小段文本（不落进报告）';
          break;
        }
        /**
         * 按一个虚拟键码（Escape=53 / Return=36 / Tab=48）。
         *
         * 为什么不用 `keys` 送 'esc' / 'return'：那走的是 `keystroke`（送"字符"），
         * 不产生真正的按键事件，界面不会当回车/退出处理（实测：快速切换面板里按
         * "return 字符"不会选中条目）。
         */
        case 'keyCode': {
          const modifiers = (step.modifiers || []).map((part) => MODIFIERS[part]).filter(Boolean);
          const sent = mac.keyCode(step.code, modifiers);
          record.status = sent.ok ? 'passed' : 'failed';
          record.detail = sent.ok ? `keyCode(${step.code})` : sent.error;
          break;
        }
        /**
         * 明确地等一会儿（毫秒）。
         *
         * 用途：等**界面自己完成一件事**——例如剪藏的「抓取」要联网取回网页，
         * 抓完之前「剪藏为新笔记」是禁用的，点上去不会有任何反应。
         * 这类等待必须在用例里写明（不能靠"点两次试试"），失败时才好定位。
         */
        case 'pause': {
          sleepMs(Math.min(60_000, Math.max(0, Number(step.ms) || 0)));
          record.status = 'passed';
          record.detail = `等待 ${step.ms} ms（等界面自己完成）`;
          break;
        }
        case 'shot': {
          const file = path.join(outDir, step.name);
          const shot = mac.screenshot(file);
          record.status = shot.ok ? 'passed' : 'failed';
          record.detail = shot.ok ? '截图已存档' : shot.error;
          record.evidence = shot.ok ? `截图 \`${step.name}\`` : '';
          break;
        }
        case 'check': {
          const checked = runCheck(workspace, step);
          record.status = checked.ok ? 'passed' : 'failed';
          record.detail = checked.ok ? '文件断言通过' : checked.error;
          record.evidence = checked.evidence || '';
          break;
        }
        /**
         * 元素断言：界面上**必须**能找到某个元素（例如首页的「待复核」卡片）。
         * 这是"界面上真的出现了这个东西"的机器证据，比只看截图强。
         */
        case 'expectElement': {
          const found = mac.frameOf(appName, step.query);
          record.status = found.ok ? 'passed' : 'failed';
          record.detail = found.ok
            ? `界面上找到了：${step.label || JSON.stringify(step.query)}`
            : `界面上没有找到：${step.label || JSON.stringify(step.query)}`;
          record.evidence = found.ok ? `✓ 界面存在「${step.label || ''}」` : '';
          break;
        }
        /**
         * 元素**不该**存在（例如"默认状态下大纲不占列表栏"）。
         * 与 expectElement 成对：一处说"在"，一处说"不在"，才叫真的有断言。
         *
         * 复核两次（间隔 800ms）才判失败：无障碍树**总是慢一拍**，刚切完状态时
         * 树里可能还留着已经不存在的元素；不复核就会把"树没刷新"写成"功能有问题"。
         */
        case 'expectAbsent': {
          let found = mac.frameOf(appName, step.query);
          for (let index = 0; found.ok && index < 2; index += 1) {
            sleepMs(800);
            found = mac.frameOf(appName, step.query);
          }
          record.status = found.ok ? 'failed' : 'passed';
          record.detail = found.ok
            ? `本来不该有，却在：${step.label || JSON.stringify(step.query)}`
            : `界面上确实没有：${step.label || JSON.stringify(step.query)}`;
          record.evidence = found.ok ? '' : `✓ 界面不存在「${step.label || ''}」`;
          break;
        }
        /**
         * 把界面**推到**目标状态（2026-10-15 实测教训）。
         *
         * 导航刚发生时，无障碍树里可能还留着**上一页**的同名元素（树的更新比界面慢一拍）。
         * 这时候 AXPress 会返回成功，但 React 那边的节点已经卸载，点击被**静默丢弃**——
         * 现象是"步骤 ✅ 通过，界面却没动"，而屏幕上什么都看不出来。
         *
         * 所以这里的语义是**以界面状态为准**：目标在就什么都不做；不在就按一次给定的控件再查。
         * 按了几次**如实写进报告**（"按了 2 次"本身就是证据，不是被抹掉的抖动）。
         * 要求 `press` 是"每次按都朝目标推进"的控件（开关类：只在目标缺失时才按）。
         */
        case 'ensure': {
          if (!step.until || !step.press) {
            record.status = 'failed';
            record.detail = 'ensure 需要 until（目标元素）与 press（要按的控件）';
            break;
          }
          const attempts = Math.max(1, Number(step.attempts) || 3);
          let presses = 0;
          let failedPress = '';
          let satisfied = false;
          for (let index = 0; index < attempts; index += 1) {
            if (mac.frameOf(appName, step.until).ok) {
              satisfied = true;
              break;
            }
            const point = resolvePoint(appName, step.press);
            if (!point.ok) {
              failedPress = point.error;
              break;
            }
            mac.click(appName, point.point, step.press);
            presses += 1;
            sleepMs(1200);
          }
          if (satisfied) {
            record.status = 'passed';
            record.detail =
              presses === 0
                ? `界面已在目标状态（${step.label || ''}），未按控件`
                : `按了 ${presses} 次「${step.pressLabel || '控件'}」达到目标状态（${step.label || ''}）`;
            record.evidence = `✓ 界面存在「${step.label || ''}」`;
          } else {
            record.status = 'failed';
            record.detail = failedPress
              ? `按不动「${step.pressLabel || '控件'}」：${failedPress}`
              : `按了 ${presses} 次「${step.pressLabel || '控件'}」仍未达到：界面没有「${step.label || ''}」`;
          }
          break;
        }
        /**
         * 改一个**原生下拉**的值（设置页那些 `<select>`）。
         *
         * 为什么需要（2026-10-15 实测）：像「大纲位置」这种**工作区级偏好**会被上一轮跑残留、
         * 也会被排查脚本改掉——用例如果假定"它一定是默认值"，就会在别的机器/别的轮次上假失败。
         * 与其假定前提，不如**用界面把前提摆好**（顺带也真机验了设置页这一行）。
         *
         * ⚠️ **不能用"按菜单项"的方式改**（2026-10-15 踩了两小时的坑）：
         * `AXPress` 点 `AXMenuItem` 确实会让 `<select>` 的 DOM `value` 变掉，**但 React 收不到 change 事件**——
         * app 的状态没动，下一次渲染还会把值弹回去。更糟的是：当时那版验证只读了 DOM 的 `value`，
         * 于是报告写着"已选 ✅"，而工作区里其实还是旧值（**假通过**，比失败更坏）。
         *
         * 现在的做法是**走键盘**（真人操作原生菜单的方式，WebKit 自己会派发 change）：
         * 按下拉打开菜单 → 连按 ↑ 回到第一项 → 按 ↓ 走到目标项 → 回车；
         * 然后**隔 2.5 秒再复核一次**（弹回去就是没生效，如实判失败）。
         *
         * 步骤参数：`{ popup: {role,description}, option, options: [按菜单顺序列出全部选项] }`。
         */
        case 'selectOption': {
          const popupQuery = step.popup || {};
          const options = Array.isArray(step.options) ? step.options : [];
          const index = options.indexOf(step.option);
          // 只看 value 匹配（findElement 里 title/description/value 三选一按子串匹配，
          // 所以这里刻意**不给** description，免得按 description 命中"名字对但值不同"的那个）
          const probe = { role: popupQuery.role || 'AXPopUpButton', value: step.option };
          if (mac.frameOf(appName, probe).ok) {
            record.status = 'passed';
            record.detail = `本来就是「${step.option}」，未改动`;
            record.evidence = `✓ 「${step.label || step.option}」= ${step.option}`;
            break;
          }
          if (index < 0) {
            record.status = 'failed';
            record.detail = 'selectOption 需要 options（按菜单顺序列出全部选项），且 option 必须在其内';
            break;
          }
          const opened = resolvePoint(appName, popupQuery);
          if (!opened.ok) {
            record.status = 'failed';
            record.detail = `找不到下拉：${opened.error}`;
            break;
          }
          mac.click(appName, opened.point, popupQuery);
          sleepMs(900);
          // ↑ 先退回第一项（到顶后再按也不会越过），再 ↓ 走到目标项 —— 不依赖"当前选的是哪一项"
          for (let stepUp = 0; stepUp < options.length; stepUp += 1) mac.keyCode(126);
          for (let stepDown = 0; stepDown < index; stepDown += 1) mac.keyCode(125);
          sleepMs(300);
          mac.keyCode(36);
          sleepMs(1200);
          // 复核两次：**改完要真的留住**（弹回去说明 React 没收到，等于没改）
          const appliedNow = mac.frameOf(appName, probe).ok;
          sleepMs(2500);
          const appliedLater = mac.frameOf(appName, probe).ok;
          record.status = appliedNow && appliedLater ? 'passed' : 'failed';
          record.detail = appliedNow && appliedLater
            ? `已选「${step.option}」，2.5 秒后复核仍是它`
            : appliedNow
              ? `选完之后被弹回去了（多半是 React 没收到 change 事件）：现在不是「${step.option}」`
              : `选完之后下拉的值不是「${step.option}」`;
          record.evidence =
            appliedNow && appliedLater ? `✓ 「${step.label || step.option}」= ${step.option}` : '';
          break;
        }
        /**
         * 元素必须**落在**另一个元素的水平范围内（框套框）。
         *
         * 用途（2026-10-15）：确认条目行没有溢出列表栏。Radix `ScrollArea` 的内容层是
         * `display: table`，会按内容撑宽——实测面板 320px 时行有 520px，行右侧的按钮因此
         * 被面板裁掉。AX 读不到透明度（"按钮是不是常显"它看不见），但**读得到坐标**，
         * 所以用几何断言把"行有没有溢出"这条兜住；两侧各留 `tolerance` px（默认 8）。
         */
        case 'expectInside': {
          const inner = mac.frameOf(appName, step.inner);
          const outer = mac.frameOf(appName, step.outer);
          if (!inner.ok || !outer.ok) {
            record.status = 'failed';
            /**
             * 顺手把"这一栏现在是什么态"带上（2026-10-16）：
             * 上一次失败只写"没找到元素"，看不出是因为**这一栏当时显示的是大纲**（列表条目根本没渲染）。
             * 三个探针查的都是轻量元素，代价可忽略，但省掉一整轮排查。
             */
            const paneState = mac.frameOf(appName, { role: 'AXButton', description: '返回笔记列表' }).ok
              ? '列表栏=大纲'
              : mac.frameOf(appName, { role: 'AXButton', description: '收起大纲' }).ok
                ? '正文右侧=大纲'
                : '列表栏=列表';
            record.detail =
              (!inner.ok
                ? `找不到内框元素：${inner.error}（${JSON.stringify(step.inner)}）`
                : `找不到外框元素：${outer.error}（${JSON.stringify(step.outer)}）`) +
              `；当时${paneState}`;
            break;
          }
          const tolerance = Number(step.tolerance) || 8;
          const innerRight = inner.frame.x + inner.frame.width;
          const outerRight = outer.frame.x + outer.frame.width;
          const ok = inner.frame.x >= outer.frame.x - tolerance && innerRight <= outerRight + tolerance;
          record.status = ok ? 'passed' : 'failed';
          record.detail =
            `内框 x=${Math.round(inner.frame.x)}..${Math.round(innerRight)}，` +
            `外框 x=${Math.round(outer.frame.x)}..${Math.round(outerRight)}` +
            (ok ? '（在范围内）' : `（超出 ${Math.round(Math.max(0, innerRight - outerRight))}px）`);
          record.evidence = ok ? `✓ ${step.label || '元素在范围内'}` : '';
          break;
        }
        /**
         * 元素的尺寸要在给定范围内（`maxHeight` / `minWidth` / `maxWidth`，都是可选）。
         *
         * 用途（2026-10-16）：钉住"大纲栏头不许变成两行"——标题一长，栏头被挤成
         * 「大 / 纲」两行时高度会翻倍，这条断言当场就能抓住；报告里带上实测尺寸。
         */
        case 'expectSize': {
          const found = mac.frameOf(appName, step.query);
          if (!found.ok) {
            record.status = 'failed';
            record.detail = `找不到元素：${found.error}（${JSON.stringify(step.query)}）`;
            break;
          }
          const { width, height } = found.frame;
          const problems = [];
          if (step.maxHeight !== undefined && height > step.maxHeight) {
            problems.push(`高度 ${Math.round(height)} > ${step.maxHeight}`);
          }
          if (step.minWidth !== undefined && width < step.minWidth) {
            problems.push(`宽度 ${Math.round(width)} < ${step.minWidth}`);
          }
          if (step.maxWidth !== undefined && width > step.maxWidth) {
            problems.push(`宽度 ${Math.round(width)} > ${step.maxWidth}`);
          }
          record.status = problems.length === 0 ? 'passed' : 'failed';
          record.detail =
            `实测 ${Math.round(width)}×${Math.round(height)}` +
            (problems.length === 0 ? '（在范围内）' : `：${problems.join('；')}`);
          record.evidence = problems.length === 0 ? `✓ ${step.label || '尺寸符合预期'}` : '';
          break;
        }
        case 'manual':
          record.status = 'manual';
          record.detail = step.note;
          break;
        default:
          record.status = 'failed';
          record.detail = `不认识的步骤：${step.op}`;
      }
    };
    // 步骤执行期间持有 AX 忙碌锁：保活轮询让路，避免两边抢 System Events
    // （实测抢起来会让步骤查不到元素，退化成"按坐标点"）
    try {
      withAxLock(execute);
    } catch (error) {
      record.status = 'failed';
      record.detail = String(error && error.message ? error.message : error);
    }
    // 界面动完要给它一点时间重画（无障碍树跟着更新）再进下一步；
    // 否则下一步的元素查找会落空 → 退化成"按坐标点"，而这个 WebView 不认坐标点击。
    if (!dryRun && (step.op === 'click' || step.op === 'keys' || step.op === 'keyCode')) {
      sleepMs(500);
    }
    steps.push(record);
  }
  return { ...item, status: report.caseStatus(steps), steps };
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.help) {
    console.log(fs.readFileSync(__filename, 'utf8').split('*/')[0].replace(/^\/\*\*?/, '').trim());
    return 0;
  }
  if (options.list) {
    for (const item of CASES) console.log(`${item.id}. ${item.title}`);
    for (const item of MANUAL_ONLY) console.log(`${item.id}. ${item.title}（人工：${item.reason}）`);
    return 0;
  }
  if (options.preflight) {
    const checks = mac.preflight(options.app);
    for (const check of checks) console.log(`${check.ok ? '✓' : '✗'} ${check.name}：${check.detail}`);
    return checks.every((check) => check.ok) ? 0 : 1;
  }

  const outDir = options.out
    ? path.resolve(options.out)
    : path.join(repoRoot, 'docs', 'acceptance', timestamp());
  const mode = options.dryRun ? 'dry-run（未执行）' : 'real（真机执行）';

  if (!options.dryRun && !mac.isMac) {
    console.error('真跑只支持 macOS；想要结论表骨架请用 --dry-run');
    return 1;
  }

  // 真跑之前先自检：**应用没在跑 / 没权限时不出报告**——
  // 否则会生成一份"9 条失败"的结论表，把"环境没准备好"写成"功能有问题"。
  if (!options.dryRun) {
    const checks = mac.preflight(options.app);
    const blocking = checks.filter((check) => !check.ok);
    if (blocking.length > 0) {
      for (const check of checks) console.log(`${check.ok ? '✓' : '✗'} ${check.name}：${check.detail}`);
      console.error('环境没准备好，已中止（没有生成结论表）。请先处理上面 ✗ 的项目，或先跑 --dry-run 看用例。');
      return 1;
    }
  }

  // 现场准备：切前台 + 确认目标工作区已打开。
  // 这一步失败也**不出结论表**——同样是不把"现场没准备好"写成"功能有问题"。
  let scene = { ok: true, steps: [] };
  if (!options.dryRun) {
    // AX 保活：不轮询的话 WebKit 会把无障碍树收起来，脚本读到的是"空树"
    // （详见 lib/ax-keepalive.js），结论表就会把"读不到界面"写成"功能有问题"。
    keepAlive = spawn(process.execPath, [path.join(__dirname, 'lib', 'ax-keepalive.js'), options.app], {
      stdio: 'ignore',
      detached: false,
    });
    scene = ensureScene(options.app, options.workspace);
    for (const step of scene.steps) {
      console.log(`${step.status === 'passed' ? '✓' : '✗'} 现场准备 · ${step.op}：${step.detail}`);
    }
    if (!scene.ok) {
      console.error('现场没准备好，已中止（没有生成结论表）。按上面最后一条 ✗ 处理后再重跑。');
      return 1;
    }
  }

  // `--only 3,4` 只跑指定用例（调某一条时用；结论表里也只列这几条，避免把"没跑"看成"没测过"）
  const selected = options.only
    ? CASES.filter((item) => options.only.includes(item.id))
    : CASES;
  if (options.only && selected.length === 0) {
    console.error(`--only 没匹配到任何用例：${options.only.join(',')}`);
    return 1;
  }
  if (options.only) {
    console.log(`只跑用例：${selected.map((item) => item.id).join(', ')}（其余未执行）`);
  }
  const cases = selected.map((item) =>
    runCase(options.app, options.workspace, outDir, item, options.dryRun),
  );
  // 自动步骤的通过/失败数：用例级状态会被"含人工步骤"拉成 🖐，
  // 所以机器证据必须单独报一次，否则读者看不出自动化那部分到底过没过。
  const steps = cases.flatMap((item) => item.steps);
  const autoPassed = steps.filter((step) => step.status === 'passed').length;
  const autoFailed = steps.filter((step) => step.status === 'failed').length;
  const summary = options.dryRun
    ? `${cases.length} 条已登记，未执行（dry-run）`
    : `${cases.filter((item) => item.status === 'passed').length} 条通过、` +
      `${cases.filter((item) => item.status === 'failed').length} 条失败、` +
      `${cases.filter((item) => item.status === 'manual').length} 条待人工` +
      `（自动步骤 ${autoPassed} 步通过 / ${autoFailed} 步失败）`;

  const run = {
    startedAt: new Date().toISOString(),
    mode,
    appName: options.app,
    workspace: options.workspace,
    scene,
    summary,
    autoSteps: { passed: autoPassed, failed: autoFailed },
    cases,
    manualOnly: MANUAL_ONLY,
  };
  const written = report.write(outDir, run);
  console.log(`结论表：${path.relative(repoRoot, written.markdownPath)}`);
  console.log(`明细：${path.relative(repoRoot, written.jsonPath)}`);
  console.log(summary);
  return cases.some((item) => item.status === 'failed') ? 1 : 0;
}

let keepAlive = null;
function stopKeepAlive() {
  if (keepAlive && !keepAlive.killed) {
    try {
      keepAlive.kill('SIGTERM');
    } catch {
      // 已经退了就算了
    }
  }
  keepAlive = null;
}
process.on('exit', stopKeepAlive);

process.exitCode = (() => {
  try {
    return main();
  } finally {
    stopKeepAlive();
  }
})();
