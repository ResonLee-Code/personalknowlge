'use strict';

const fs = require('node:fs');
const path = require('node:path');

/**
 * 结论表写出（Markdown + JSON）。
 *
 * 口径（与计划 §1.5 的落地纪律一致）：
 * - **没跑的不能写成「通过」**：dry-run / 被跳过的步骤一律标「未执行」；
 * - 只能人眼判断的步骤标「待人工」——脚本不替人下结论；
 * - 每条用例都留证据（截图文件名 / 文件断言结果），报告里可点回去核对。
 */

const STATUS_LABEL = {
  passed: '✅ 通过',
  failed: '❌ 失败',
  manual: '🖐 待人工',
  skipped: '⏭ 未执行',
};

/** 由逐条执行结果汇总出用例状态。 */
function caseStatus(steps) {
  if (steps.some((step) => step.status === 'failed')) return 'failed';
  if (steps.some((step) => step.status === 'manual')) return 'manual';
  if (steps.some((step) => step.status === 'skipped')) return 'skipped';
  return 'passed';
}

function renderMarkdown(run) {
  const lines = [];
  lines.push('# E 节真机验收（自动跑）');
  lines.push('');
  lines.push(`- 开始时间：${run.startedAt}`);
  lines.push(`- 结论：${run.summary}`);
  if (run.autoSteps) {
    lines.push(
      `- 自动步骤：**${run.autoSteps.passed} 步通过 / ${run.autoSteps.failed} 步失败**` +
        '（用例含人工确认时整体标「待人工」，机器证据看这里与每条的明细）',
    );
  }
  lines.push(`- 工作区：\`${run.workspace}\``);
  lines.push(`- 应用：${run.appName}`);
  lines.push(`- 模式：${run.mode}`);
  lines.push(
    '- 截图：同名 PNG 就在本目录（**默认不进仓库**，见 `scripts/acceptance/README.md`；要留证请 `git add -f`）',
  );
  lines.push('');
  if (run.scene && run.scene.steps && run.scene.steps.length > 0) {
    lines.push('## 现场准备（自动）');
    lines.push('');
    lines.push('> 每轮真跑都会先做这一步：把应用切到前台，并确认**目标工作区**已打开。');
    lines.push('> 现场不对就不出结论表——不能把「现场没准备好」写成「功能有问题」。');
    lines.push('');
    for (const step of run.scene.steps) {
      lines.push(`- [${STATUS_LABEL[step.status] ?? step.status}] \`${step.op}\` ${step.detail ?? ''}`);
    }
    lines.push('');
  }
  lines.push('## 结论表');
  lines.push('');
  lines.push('| # | 项目 | 结果 | 证据 |');
  lines.push('| --- | --- | --- | --- |');
  for (const item of run.cases) {
    const evidence = item.steps
      .filter((step) => step.evidence)
      .map((step) => step.evidence)
      .join('；');
    lines.push(
      `| ${item.id} | ${item.title} | ${STATUS_LABEL[item.status] ?? item.status} | ${evidence || '—'} |`,
    );
  }
  for (const item of run.manualOnly) {
    lines.push(`| ${item.id} | ${item.title} | 🖐 人工 | ${item.reason} |`);
  }
  lines.push('');
  lines.push('## 逐条明细');
  for (const item of run.cases) {
    lines.push('');
    lines.push(`### ${item.id}. ${item.title}（${STATUS_LABEL[item.status] ?? item.status}）`);
    lines.push('');
    lines.push(`- 期望：${item.expect}`);
    for (const step of item.steps) {
      lines.push(`- [${STATUS_LABEL[step.status] ?? step.status}] \`${step.op}\` ${step.detail ?? ''}`);
    }
  }
  lines.push('');
  return lines.join('\n');
}

function write(outDir, run) {
  fs.mkdirSync(outDir, { recursive: true });
  const markdownPath = path.join(outDir, 'report.md');
  const jsonPath = path.join(outDir, 'report.json');
  fs.writeFileSync(markdownPath, renderMarkdown(run), 'utf8');
  fs.writeFileSync(jsonPath, JSON.stringify(run, null, 2), 'utf8');
  return { markdownPath, jsonPath };
}

module.exports = { STATUS_LABEL, caseStatus, renderMarkdown, write };
