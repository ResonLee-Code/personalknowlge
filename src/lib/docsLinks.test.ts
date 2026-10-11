import { execFileSync } from 'node:child_process';
import { readFileSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

/**
 * 文档链接守卫（验收清单里那条「文档内引用的相对路径全部存在」的可执行版本）。
 *
 * 以前这条是**临时手工核对**的，改文档时没人再跑一遍就会烂掉。这里把 `docs/**.md`
 * 与根 `README.md` 里所有相对链接的目标逐个查一遍：
 * 目标文件不存在 → 直接失败并一次列出全部问题（便于一次改完）。
 *
 * 外链（http/https/mailto）与纯锚点（`#section`）不做检查。
 *
 * 只查**被 Git 跟踪**的 Markdown：`docs/yuque-manuals/` 下的语雀手册正文是第三方抓取内容
 * （默认不入库），里面全是相对他们站点根目录的伪链接，不属于本项目要维护的引用。
 */
const repoRoot = resolve(__dirname, '../..');

function markdownFiles(): string[] {
  const listed = execFileSync('git', ['ls-files'], {
    cwd: repoRoot,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  return listed
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.endsWith('.md'))
    .map((line) => join(repoRoot, line))
    .filter((path) => !path.includes('/docs/yuque-manuals/books/'));
}

/** 去掉围栏代码块与行内代码：代码里的 `[文字](路径)` 是**示例**，不是链接。 */
function stripCode(markdown: string): string {
  return markdown
    .replace(/```[\s\S]*?```/g, '')
    .replace(/~~~[\s\S]*?~~~/g, '')
    .replace(/`[^`\n]*`/g, '');
}

function relativeLinks(markdown: string): string[] {
  const links: string[] = [];
  const pattern = /\]\(([^)\s]+)\)/g;
  const source = stripCode(markdown);
  let match: RegExpExecArray | null;
  while ((match = pattern.exec(source)) !== null) {
    const target = match[1] ?? '';
    if (!target || /^(https?:|mailto:|#|data:)/i.test(target)) continue;
    links.push(target);
  }
  return links;
}

describe('文档相对链接', () => {
  it('docs/**.md 与 README.md 里的相对链接都指向真实存在的文件', () => {
    const missing: string[] = [];
    for (const file of markdownFiles()) {
      const content = readFileSync(file, 'utf8');
      for (const link of relativeLinks(content)) {
        const withoutAnchor = link.split('#')[0];
        if (!withoutAnchor) continue;
        const target = resolve(dirname(file), withoutAnchor);
        let exists = false;
        try {
          // 目录链接（例如 `[截图](./docs/screenshots)`）也算有效引用
          exists = statSync(target).isFile() || statSync(target).isDirectory();
        } catch {
          exists = false;
        }
        if (!exists) {
          missing.push(`${file.replace(`${repoRoot}/`, '')} → ${link}`);
        }
      }
    }
    expect(missing, `以下文档链接指向不存在的文件：\n${missing.join('\n')}`).toEqual([]);
  });
});
