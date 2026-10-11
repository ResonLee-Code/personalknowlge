import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync, statSync } from 'node:fs';
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

/** 去掉围栏代码块（` ``` ` / `~~~`），行内代码保留（链接检查时要删、行内路径检查时要看）。 */
function stripFencedBlocks(markdown: string): string {
  return markdown.replace(/```[\s\S]*?```/g, '').replace(/~~~[\s\S]*?~~~/g, '');
}

/** 去掉围栏代码块**与**行内代码：代码里的 `[文字](路径)` 是**示例**，不是链接。 */
function stripCode(markdown: string): string {
  return stripFencedBlocks(markdown).replace(/`[^`\n]*`/g, '');
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

/**
 * 行内代码里的**仓库内路径**也要存在。
 *
 * 起因：`docs/BUILD_COMMANDS.md` 里写着「crates 源配置在 `src-tauri/.cargo/config.toml`」，
 * 而真实文件在仓库根 `.cargo/config.toml`——文档让读者去核对一个不存在的文件。
 * 只查带已知扩展名的路径，避免把 `pnpm test`、`note_create` 这类命令/标识符误判成路径。
 */
const PATH_EXTENSIONS = [
  'md', 'toml', 'json', 'yml', 'yaml', 'rs', 'ts', 'tsx', 'css', 'sql',
  'py', 'sh', 'ps1', 'nsi', 'lock', 'txt', 'html', 'conf',
];

function inlineCodePaths(markdown: string): string[] {
  const paths: string[] = [];
  // 只看行内代码：围栏块里的内容是示例/日志，不必逐条核对
  for (const match of stripFencedBlocks(markdown).matchAll(/`([^`\n]+)`/g)) {
    const token = (match[1] ?? '').trim();
    if (!token.includes('/') || /\s/.test(token)) continue;
    if (token.includes('*')) continue; // 通配符是模式，不是具体路径
    const extension = token.split('.').pop()?.toLowerCase() ?? '';
    if (!PATH_EXTENSIONS.includes(extension)) continue;
    if (/^(https?:|mailto:)/i.test(token)) continue;
    paths.push(token);
  }
  return paths;
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

  it('行内代码里提到的仓库内路径真实存在', () => {
    // 只查「首段是仓库真实顶层目录」的路径：这样 `src-tauri/.cargo/config.toml`
    // 这类「前缀写错」会被抓到，而文档里惯用的模块相对简写（`core/text.rs`、
    // `application/ai/policy.rs`、`tests/x.rs`）与占位符（`.knowledge/templates/<名字>.md`）
    // 不会被误判成烂路径。
    const topLevel = new Set([...readdirSync(repoRoot)]);
    const missing: string[] = [];
    for (const file of markdownFiles()) {
      const content = readFileSync(file, 'utf8');
      for (const token of inlineCodePaths(content)) {
        if (token.includes('<') || token.includes('>')) continue;
        const [head] = token.split('/');
        if (!head || !topLevel.has(head)) continue;
        const candidate = resolve(repoRoot, token);
        let exists = false;
        try {
          exists = statSync(candidate).isFile() || statSync(candidate).isDirectory();
        } catch {
          exists = false;
        }
        if (!exists) missing.push(`${file.replace(`${repoRoot}/`, '')} → \`${token}\``);
      }
    }
    expect(
      missing,
      `以下文档提到的仓库内路径不存在（读者会照着去找）：\n${missing.join('\n')}`,
    ).toEqual([]);
  });
});
