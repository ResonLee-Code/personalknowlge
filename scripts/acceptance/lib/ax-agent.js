ObjC.import('ApplicationServices');
ObjC.import('AppKit');
ObjC.import('CoreGraphics');
ObjC.import('Foundation');

/**
 * macOS 侧的执行代理（JXA / `osascript -l JavaScript`）。
 *
 * 只做四件在 AppleScript 里做不好的事：**找元素拿坐标**、
 * **用 CoreGraphics 投递真实鼠标事件**（点击 / 拖拽 / 滚轮）、按键与输入。
 * 参数是一个 JSON 字符串（`argv[0]`），结果打印成一行 JSON。
 *
 * ⚠️ 这一层需要「辅助功能（Accessibility）」权限，且只能在装了应用的 macOS 上跑；
 * 仓库的自动化测试**不会**执行它——首次使用请先跑 `run.sh --preflight`。
 *
 * ## 为什么用 System Events 找元素，而不是直接调 AX API（2026-10-14 实测教训）
 *
 * 最初的实现是 `AXUIElementCopyAttributeValue` + `Ref()` 取输出参数，**在 macOS 27 上全部失败**：
 * 属性名传 JS 字符串会返回 `-25201`（illegal argument），必须桥接成 `CFString`（`$('AXRole')`）；
 * 桥接之后错误码变 0，但 `Ref()` 拿到的仍是一个**无法解引用的不透明对象**（`[object Ref]`），
 * 于是遍历结果是"一个节点"、任何元素都找不到——现象就是**每条用例的第一次点击都报"没找到元素"**。
 *
 * 改走 `Application('System Events')` 之后，整棵 UI 树（含 WKWebView 里的 React 界面）都能读到，
 * 元素自带 `position` / `size`，正好拿来算点击坐标。
 * **点击仍然用 CoreGraphics 投递真实鼠标事件**：System Events 的 `click` 走 AXPress，
 * 对没有该动作的 React 元素无效，而真实鼠标事件和用户手点完全一样。
 */

/**
 * 输出一行 JSON 到 **stdout**。
 *
 * 注意：JXA 里 `console.log` 走的是 **stderr**，Node 侧用 execFileSync 读 stdout 会拿到空串——
 * 所以这里必须显式写 `NSFileHandle.fileHandleWithStandardOutput`。
 */
function out(value) {
  const text = JSON.stringify(value === undefined ? null : value) + '\n';
  const data = $.NSString.alloc.initWithUTF8String(text).dataUsingEncoding($.NSUTF8StringEncoding);
  $.NSFileHandle.fileHandleWithStandardOutput.writeData(data);
}

function unwrap(value) {
  if (value === undefined || value === null) return null;
  try {
    return ObjC.unwrap(value);
  } catch (error) {
    return value;
  }
}

function pidOf(appName) {
  const apps = $.NSWorkspace.sharedWorkspace.runningApplications;
  for (let index = 0; index < apps.count; index += 1) {
    const app = apps.objectAtIndex(index);
    if (unwrap(app.localizedName) === appName) return app.processIdentifier;
  }
  return -1;
}

/** 读一个 UI 属性，读不到就返回 null（System Events 对不存在的属性会抛错）。 */
function read(element, property) {
  try {
    const value = element[property]();
    if (value === undefined || value === null) return null;
    const text = String(value);
    return text === 'null' || text === 'missing value' ? null : text;
  } catch (error) {
    return null;
  }
}

/** 取元素的位置与尺寸（System Events 直接给 [x, y] 与 [w, h]）。 */
function frameOfElement(element) {
  try {
    const position = element.position();
    const size = element.size();
    if (!position || !size) return null;
    return { x: position[0], y: position[1], width: size[0], height: size[1] };
  } catch (error) {
    return null;
  }
}

/**
 * 找一个元素：`role` 必给；`title` / `description` / `value` 任一**包含**给定子串即命中
 * （按钮的可读文字在 WebView 里经常落在 title 或 value 上，实测两者都要看）。
 */
function findElement(process, query) {
  const window = process.windows[0];
  if (!window) return null;
  let elements;
  try {
    elements = window.entireContents();
  } catch (error) {
    return null;
  }
  const wanted = query.title || query.description || query.value;
  for (let index = 0; index < elements.length; index += 1) {
    const element = elements[index];
    const role = read(element, 'role');
    if (query.role && role !== query.role) continue;
    if (!wanted) return element;
    for (const property of ['title', 'description', 'value']) {
      const current = read(element, property);
      if (current && current.indexOf(wanted) >= 0) return element;
    }
  }
  return null;
}

/**
 * 带重试的查找。
 *
 * 为什么需要（实测）：界面切页/开弹层之后，无障碍树要过一会儿才更新；
 * 只查一次会在"刚点完就查"的瞬间落空，现象是**明明点对了却报"没找到元素"**。
 * `entireContents()` 每次约 1 秒，所以重试次数压到 3 次（最坏约 3 秒）。
 */
function findElementWithRetry(process, query, attempts) {
  // 5 → 3（2026-10-17）：单次 entireContents() 在大库上要十几秒，重试 5 次就是一分多钟
  const tries = attempts || 3;
  for (let index = 0; index < tries; index += 1) {
    const element = findElement(process, query);
    if (element) return element;
    if (index + 1 < tries) delay(0.4);
  }
  return null;
}

/**
 * 投递一个鼠标事件。
 *
 * ⚠️ `clickState` 必须显式设成 1（按下/抬起都要）：`CGEventCreateMouseEvent` 造的原始事件
 * 这个字段是 0，**WebKit / Chromium 会把它当成无效点击直接忽略**——现象是坐标明明对
 * （控件会显示焦点环），但界面的 onClick 永远不触发。这是实测踩到的坑。
 */
function postMouse(type, point, clickState) {
  const event = $.CGEventCreateMouseEvent($(), type, { x: point.x, y: point.y }, $.kCGMouseButtonLeft);
  if (clickState) {
    $.CGEventSetIntegerValueField(event, $.kCGMouseEventClickState, clickState);
  }
  $.CGEventPost($.kCGHIDEventTap, event);
}

function clickAt(point) {
  postMouse($.kCGEventMouseMoved, point);
  postMouse($.kCGEventLeftMouseDown, point, 1);
  // 按下与抬起之间要有真实间隔：零延迟的"瞬时点击"会被 WebKit 合并/丢弃（实测）
  delay(0.08);
  postMouse($.kCGEventLeftMouseUp, point, 1);
}

function run(argv) {
  let request = {};
  try {
    request = JSON.parse(argv[0] || '{}');
  } catch (error) {
    return out({ ok: false, error: '参数不是合法 JSON' });
  }

  try {
    const pid = request.app ? pidOf(request.app) : -1;
    const needsApp = ['pid', 'activate', 'frame'];
    if (needsApp.indexOf(request.op) >= 0 && pid < 0) {
      return out({ ok: false, error: `「${request.app}」没在运行` });
    }

    switch (request.op) {
      case 'trusted':
        return out({ ok: true, trusted: Boolean($.AXIsProcessTrusted()) });
      case 'pid':
        return out({ ok: true, pid: pid });
      // 把应用切到前台：CoreGraphics 投递的点击落在**最前面的窗口**上，
      // 不激活的话脚本会把点击送到别的应用里（看到的现象是"所有步骤都点不到元素"）。
      case 'activate': {
        const running = $.NSRunningApplication.runningApplicationWithProcessIdentifier(pid);
        if (!running) return out({ ok: false, error: '拿不到应用实例' });
        const activated = running.activateWithOptions($.NSApplicationActivateIgnoringOtherApps);
        delay(0.4);
        return out({ ok: true, activated: Boolean(activated) });
      }
      case 'frame': {
        const process = Application('System Events').processes.byName(request.app);
        const element = findElementWithRetry(process, request.query || {});
        if (!element) return out({ ok: false, error: '没找到元素' });
        const frame = frameOfElement(element);
        return out(frame ? { ok: true, frame: frame } : { ok: false, error: '拿不到坐标' });
      }
      /**
       * 点击：**优先 AXPress**（`element.click()`），拿不到动作才退到真实鼠标事件。
       *
       * 为什么（2026-10-14 实测）：这个应用的界面跑在 WKWebView 里，
       * 用 `CGEventPost` 投递的鼠标点击**不生效**——坐标是对的（控件会亮起焦点环），
       * 但 WebKit 不把它当成一次真正的点击，React 的 onClick 永远不触发；
       * 补上 `kCGMouseEventClickState = 1` 也一样。
       * 而 System Events 的 `click()` 走 AXPress，对这个界面的按钮有效。
       */
      case 'click': {
        let elementFound = false;
        let pressError = null;
        if (request.query) {
          const process = Application('System Events').processes.byName(request.app);
          const element = findElementWithRetry(process, request.query);
          elementFound = Boolean(element);
          if (element) {
            try {
              element.click();
              delay(0.2);
              return out({ ok: true, method: 'AXPress' });
            } catch (error) {
              // 没有 AXPress 的控件（自绘的 div 等）：退到真实鼠标事件
              pressError = String(error);
            }
          }
        }
        if (!request.point) return out({ ok: false, error: '缺少 point' });
        clickAt(request.point);
        // 把"为什么没用 AXPress"带回去：查不到元素 vs 查到了但按不动，是两回事
        return out({ ok: true, method: 'CGEvent', elementFound, pressError });
      }
      case 'drag': {
        const from = request.from;
        const to = request.to;
        if (!from || !to) return out({ ok: false, error: '缺少 from/to' });
        postMouse($.kCGEventMouseMoved, from);
        postMouse($.kCGEventLeftMouseDown, from, 1);
        const steps = 12;
        for (let index = 1; index <= steps; index += 1) {
          postMouse($.kCGEventLeftMouseDragged, {
            x: from.x + ((to.x - from.x) * index) / steps,
            y: from.y + ((to.y - from.y) * index) / steps,
          });
          delay(0.01);
        }
        postMouse($.kCGEventLeftMouseUp, to, 1);
        return out({ ok: true });
      }
      /**
       * 悬停：只投递鼠标移动，不按任何键。
       *
       * 为什么需要（2026-10-15）：大纲「贴边 mini 态」是**悬停展开**的，
       * 而悬停不是 AX 动作——只靠 AXPress 覆盖不到它，用例里只能标「待人工」。
       * 实测鼠标移动事件能让 WebKit 认账（`drag` 的中间事件就是靠这个生效的），
       * 所以这里补一个"纯移动"原语，把这条也变成机器证据。
       */
      case 'hover': {
        if (!request.point) return out({ ok: false, error: '缺少 point' });
        /**
         * 先把点夹进窗口内（2026-10-15 实测）：贴边 mini 态的「大纲」条贴在窗口最右侧，
         * 取它的**中心**正好落在窗口边界上——鼠标移过去（甚至出界）WebKit 不会派发 mouseover，
         * 悬浮大纲就不展开（现象："悬停步骤 ✅ 通过，但面板没出来"）。
         * 这里按窗口框内缩 2px 夹一下，保证光标真的落在那个控件身上。
         */
        let point = { x: request.point.x, y: request.point.y };
        try {
          const window = Application('System Events').processes.byName(request.app).windows[0];
          const position = window.position();
          const size = window.size();
          if (position && size) {
            const left = position[0] + 2;
            const top = position[1] + 2;
            const right = position[0] + size[0] - 2;
            const bottom = position[1] + size[1] - 2;
            point = {
              x: Math.min(Math.max(point.x, left), right),
              y: Math.min(Math.max(point.y, top), bottom),
            };
          }
        } catch (error) {
          // 拿不到窗口框就按原点走：最坏情况跟以前一样（可能不展开）
        }
        postMouse($.kCGEventMouseMoved, point);
        // 再补一次：WebKit 对"从没动过就停在原地"的移动不一定会派发 mouseover（实测）
        delay(0.15);
        postMouse($.kCGEventMouseMoved, {
          x: point.x - (request.jitter || 2),
          y: point.y - (request.jitter || 2),
        });
        return out({ ok: true });
      }
      case 'scroll': {
        if (!request.point) return out({ ok: false, error: '缺少 point' });
        const event = $.CGEventCreateScrollWheelEvent(
          $(),
          $.kCGScrollEventUnitLine,
          1,
          request.ticks || 1,
        );
        $.CGEventPost($.kCGHIDEventTap, event);
        return out({ ok: true });
      }
      case 'keystroke': {
        Application('System Events').keystroke(
          request.text || '',
          request.modifiers ? { using: request.modifiers } : {},
        );
        return out({ ok: true });
      }
      /**
       * 按一个**虚拟键码**（例如 Tab = 48、Return = 36、Escape = 53）。
       *
       * 为什么不用 `keystroke('\t')`（2026-10-14 实测）：那送的是"制表符字符"，
       * 不是一次 Tab 按键，界面不会因此移动焦点；要用 `key code`。
       */
      case 'keyCode': {
        Application('System Events').keyCode(request.code, request.modifiers ? { using: request.modifiers } : {});
        delay(0.2);
        return out({ ok: true });
      }
      /**
       * 粘贴一段文本（先写系统剪贴板，再 ⌘V）。
       *
       * 为什么需要它（2026-10-14 实测）：`keystroke` 送**中文**字符会走系统输入法，
       * 文字进的是候选窗（屏幕上冒出一排「啊啊啊…」），不是目标输入框；
       * 走剪贴板则与输入法无关，中英文都稳。
       */
      case 'paste': {
        const board = $.NSPasteboard.generalPasteboard;
        board.clearContents;
        board.setStringForType(
          $.NSString.alloc.initWithUTF8String(request.text || ''),
          $.NSPasteboardTypeString,
        );
        Application('System Events').keystroke('v', { using: ['command down'] });
        delay(0.2);
        return out({ ok: true });
      }
      default:
        return out({ ok: false, error: '不认识的 op：' + request.op });
    }
  } catch (error) {
    return out({ ok: false, error: String(error) });
  }
}
