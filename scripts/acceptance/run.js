#!/usr/bin/env node
'use strict';

/**
 * E 节真机验收的**一条命令**入口（Track E1）。
 *
 *   node scripts/acceptance/run.js --list            # 看用例
 *   node scripts/acceptance/run.js --dry-run         # 只出结论表骨架（不碰界面）
 *   node scripts/acceptance/run.js --preflight       # 自检：工具 / 辅助功能权限 / 应用是否在跑
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
  };
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--list') options.list = true;
    else if (arg === '--dry-run') options.dryRun = true;
    else if (arg === '--preflight') options.preflight = true;
    else if (arg === '--app') options.app = argv[++index];
    else if (arg === '--workspace') options.workspace = argv[++index];
    else if (arg === '--out') options.out = argv[++index];
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
    const entries = fs.readdirSync(target).filter((name) => name.includes(step.containsFile));
    if (entries.length === 0) return { ok: false, error: `${step.path} 下没有匹配「${step.containsFile}」的文件` };
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

  const cases = CASES.map((item) =>
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
