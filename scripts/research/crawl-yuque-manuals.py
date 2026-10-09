#!/usr/bin/env python3
"""抓取「语雀」官方公开手册，归档到 docs/yuque-manuals/（仅供本地查阅与功能对标）。

语雀站点是 SPA，直接抓 HTML 只能拿到标题。这里用三条可复现的公开路径：

1. 知识库列表：GET /api/groups/{group_id}/books?offset=0&limit=100
2. 目录树：    抓 https://www.yuque.com/yuque/{book_slug} 页面，
              解析服务端注入的 window.appData（URL-encoded JSON）里的 book.toc
3. 文档正文：  GET /api/docs/{doc_slug}?book_id={book_id}&mode=markdown&plain=true
              → data.sourcecode（Markdown 原文）

用法：
    python3 scripts/research/crawl-yuque-manuals.py            # 全量抓取（约 1400 篇，2~5 分钟）
    python3 scripts/research/crawl-yuque-manuals.py --books thyzgp,gpvawt   # 只抓指定手册

产出：
    docs/yuque-manuals/README.md          索引（手册清单 + 抓取方法 + 版权说明）
    docs/yuque-manuals/index.json         结构化清单（含每篇的原始 URL）
    docs/yuque-manuals/books/<slug>/*.md  每本手册的目录索引与全部文档正文

注意：正文是语雀的公开内容，版权归语雀所有，仅用于个人学习与功能对标，
不要对外分发；归档目录默认不纳入 Git（见 docs/yuque-manuals/.gitignore）。
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys
import urllib.parse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone

GROUP_ID = 84140  # 语雀官方组（https://www.yuque.com/yuque）
ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT_DIR = ROOT / "docs" / "yuque-manuals"
BOOKS_DIR = OUT_DIR / "books"
WORKERS = 5
UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) pkos-research/1.0"


def curl(url: str, accept: str) -> str:
    """单次 HTTP 抓取；失败抛异常，由调用方重试。"""
    proc = subprocess.run(
        [
            "curl", "-L", "--fail", "--silent", "--show-error", "--max-time", "40",
            "-H", f"Accept: {accept}", "-H", f"User-Agent: {UA}", url,
        ],
        capture_output=True,
    )
    if proc.returncode != 0:
        raise RuntimeError(f"curl {proc.returncode}: {proc.stderr.decode('utf-8', 'ignore')[:200]}")
    return proc.stdout.decode("utf-8", "ignore")


def curl_json(url: str, attempts: int = 3) -> dict:
    last: Exception | None = None
    for attempt in range(attempts):
        try:
            return json.loads(curl(url, "application/json"))
        except Exception as exc:  # noqa: BLE001 - 网络抖动重试
            last = exc
            if attempt < attempts - 1:
                import time

                time.sleep(1.5 * (attempt + 1))
    raise RuntimeError(f"GET {url} 失败：{last}")


def book_appdata(book_slug: str) -> dict:
    raw = curl(f"https://www.yuque.com/yuque/{book_slug}", "text/html")
    match = re.search(r'window\.appData = JSON\.parse\(decodeURIComponent\("([^"]+)"\)\)', raw)
    if not match:
        raise RuntimeError(f"{book_slug}: 页面未包含 window.appData")
    return json.loads(urllib.parse.unquote(match.group(1)))


UNSAFE = re.compile(r'[\\/:*?"<>|\n\r\t]+')


def safe_name(text: str, limit: int = 70) -> str:
    cleaned = UNSAFE.sub("、", text or "").strip().strip(".")
    return (cleaned[:limit] or "untitled").strip()


def book_list() -> list[dict]:
    payload = curl_json(f"https://www.yuque.com/api/groups/{GROUP_ID}/books?offset=0&limit=100")
    books = payload.get("data") or []
    if not books:
        raise RuntimeError("知识库列表为空，接口结构可能已变化")
    return books


def flatten_toc(nodes: list[dict], depth: int = 0) -> list[dict]:
    """把 `book.toc` 拍平成带层级的列表。

    语雀的目录节点形如 `{type, title, url, level, child_uuid, ...}`：
    - `type = DOC` 时 `url` 是**文档 slug**（如 `repository`）；
    - `type = TITLE` 是分组标题，`type = LINK` 是外链（`url` 为完整地址）；
    - 层级由 `level` 给出（0 起），嵌套也可能出现在 `children` 里，两者都兼容。
    """
    rows: list[dict] = []
    for node in nodes:
        kind = node.get("type")
        raw_url = (node.get("url") or "").strip()
        slug = raw_url if kind == "DOC" and raw_url and not raw_url.startswith("http") else None
        rows.append(
            {
                "type": kind,
                "title": node.get("title") or "",
                "slug": slug,
                "url": raw_url,
                "level": node.get("level", depth),
                "depth": int(node.get("level", depth) or 0),
            }
        )
        if node.get("children"):
            rows.extend(flatten_toc(node["children"], depth + 1))
    return rows


def crawl_book(book: dict) -> dict:
    slug = book["slug"]
    name = book.get("name") or slug
    data = book_appdata(slug)
    meta = data.get("book") or {}
    book_id = meta.get("id") or book.get("id")
    toc = flatten_toc(meta.get("toc") or [])
    docs = [row for row in toc if row["type"] == "DOC" and row.get("slug")]

    book_dir = BOOKS_DIR / slug
    book_dir.mkdir(parents=True, exist_ok=True)

    def fetch(index_and_row):
        index, row = index_and_row
        url = (
            f"https://www.yuque.com/api/docs/{row['slug']}"
            f"?book_id={book_id}&mode=markdown&plain=true"
        )
        try:
            payload = curl_json(url)
        except Exception as exc:  # noqa: BLE001 - 单篇失败（已删除 / 无权限）不拖垮整本手册
            return {"error": str(exc), **row}
        body = (payload.get("data") or {}).get("sourcecode") or ""
        title = (payload.get("data") or {}).get("title") or row["title"]
        file_name = f"{index:03d}-{safe_name(title)}.md"
        source_url = f"https://www.yuque.com/yuque/{slug}/{row['slug']}"
        header = (
            "---\n"
            f"title: {json.dumps(title, ensure_ascii=False)}\n"
            f"book: {json.dumps(name, ensure_ascii=False)}\n"
            f"book_slug: {slug}\n"
            f"doc_slug: {row['slug']}\n"
            f"source: {source_url}\n"
            f"depth: {row['depth']}\n"
            f"fetched_at: {datetime.now(timezone.utc).astimezone().isoformat(timespec='seconds')}\n"
            "---\n\n"
        )
        (book_dir / file_name).write_text(header + body.rstrip() + "\n", encoding="utf-8")
        return {**row, "title": title, "url": source_url, "file": f"books/{slug}/{file_name}", "chars": len(body)}

    with ThreadPoolExecutor(max_workers=WORKERS) as pool:
        results = list(pool.map(fetch, list(enumerate(docs, start=1))))
    skipped = [item for item in results if item.get("error")]
    results = [item for item in results if not item.get("error")]
    if skipped:
        print(f"    （{len(skipped)} 篇跳过：{skipped[0]['title']} 等）", flush=True)

    # 目录索引：保留 TITLE / LINK 节点，方便按层级查阅
    index_lines = [
        f"# {name}",
        "",
        f"- 语雀知识库：https://www.yuque.com/yuque/{slug}",
        f"- 文档数：{len(results)}（目录节点含分组标题）",
        f"- 抓取时间：{datetime.now(timezone.utc).astimezone().isoformat(timespec='seconds')}",
        "",
        "## 目录",
        "",
    ]
    by_slug = {row["slug"]: row for row in results}
    for row in toc:
        indent = "  " * row["depth"]
        if row["type"] == "DOC" and row["slug"] in by_slug:
            doc = by_slug[row["slug"]]
            # 手册索引与文档同目录，用文件名即可（doc['file'] 形如 books/<slug>/NNN-xxx.md）
            index_lines.append(
                f"{indent}- [{row['title']}]({pathlib.PurePosixPath(doc['file']).name})"
            )
        elif row["type"] == "LINK":
            index_lines.append(f"{indent}- [{row['title']}]({row['url']})（外链）")
        elif row["title"]:
            index_lines.append(f"{indent}- **{row['title']}**")
    (book_dir / "README.md").write_text("\n".join(index_lines) + "\n", encoding="utf-8")

    print(f"  ✓ {name:<28} {len(results):>4} 篇", flush=True)
    return {
        "slug": slug,
        "name": name,
        "book_id": book_id,
        "url": f"https://www.yuque.com/yuque/{slug}",
        "declared_items": book.get("items_count"),
        "docs": results,
        "skipped": [
            {"title": item["title"], "slug": item["slug"], "reason": item["error"][:120]}
            for item in skipped
        ],
        "dir": f"books/{slug}",
    }


def write_readme(books: list[dict], fetched_at: str) -> None:
    total = sum(len(book["docs"]) for book in books)
    lines = [
        "# 语雀官方公开手册归档（功能对标用）",
        "",
        f"> 抓取时间：{fetched_at} · 手册 {len(books)} 本 · 文档 {total} 篇",
        "",
        "## 用途",
        "",
        "把语雀官方帮助手册落到本地，用于 **查阅原件** 与 **功能对标**"
        "（对标结论见 [../FEATURE_PLANNING.md](../FEATURE_PLANNING.md) 第 7 节，"
        "原文摘录见 [../research/YUQUE_FEATURE_NOTES.md](../research/YUQUE_FEATURE_NOTES.md)）。",
        "",
        "## 抓取方法（可复现）",
        "",
        "```bash",
        "python3 scripts/research/crawl-yuque-manuals.py            # 全量",
        "python3 scripts/research/crawl-yuque-manuals.py --books thyzgp,gpvawt",
        "```",
        "",
        "| 数据 | 接口 / 位置 |",
        "| --- | --- |",
        "| 知识库列表 | `GET /api/groups/84140/books` |",
        "| 目录树 | 页面内注入的 `window.appData`（URL-encoded JSON）→ `book.toc` |",
        "| 文档正文 | `GET /api/docs/{slug}?book_id={id}&mode=markdown` → `data.sourcecode` |",
        "",
        "## 手册清单",
        "",
        "| # | 手册 | 文档数 | 本地目录 | 原文 |",
        "| --- | --- | --- | --- | --- |",
    ]
    for index, book in enumerate(sorted(books, key=lambda item: -len(item["docs"])), start=1):
        lines.append(
            f"| {index} | {book['name']} | {len(book['docs'])} | "
            f"[{book['dir']}](./{book['dir']}/README.md) | [语雀]({book['url']}) |"
        )
    lines += [
        "",
        "## 版权与使用边界",
        "",
        "- 正文版权归 **语雀（蚂蚁集团）** 所有，这里只做**本地存档**用于个人学习与功能对标，**不要对外分发**；",
        "- 内容为 2026-10-10 的**公开手册**快照，不含登录态下的私有空间内容；语雀迭代较快，需要时重跑脚本刷新；",
        "- 归档正文默认**不纳入 Git**（见本目录 `.gitignore`），索引与抓取脚本会纳入版本管理；",
        "  若需要把正文一起提交，删除 `.gitignore` 中的 `books/` 一行后重新 `git add` 即可。",
        "",
    ]
    (OUT_DIR / "README.md").write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description="抓取语雀官方公开手册")
    parser.add_argument("--books", help="只抓指定手册（逗号分隔的 book slug）")
    args = parser.parse_args()

    fetched_at = datetime.now(timezone.utc).astimezone().isoformat(timespec="seconds")
    print(f"→ 获取知识库列表（group {GROUP_ID}）", flush=True)
    books = book_list()
    if args.books:
        wanted = {item.strip() for item in args.books.split(",") if item.strip()}
        books = [book for book in books if book["slug"] in wanted]
    print(f"→ 命中 {len(books)} 本手册，开始抓取正文（并发 {WORKERS}）", flush=True)

    BOOKS_DIR.mkdir(parents=True, exist_ok=True)
    results: list[dict] = []
    for book in sorted(books, key=lambda item: -(item.get("items_count") or 0)):
        try:
            results.append(crawl_book(book))
        except Exception as exc:  # noqa: BLE001 - 单本失败不影响整体
            print(f"  ✗ {book.get('name')}: {exc}", file=sys.stderr, flush=True)

    write_readme(results, fetched_at)
    (OUT_DIR / "index.json").write_text(
        json.dumps(
            {
                "source": "https://www.yuque.com/yuque",
                "group_id": GROUP_ID,
                "fetched_at": fetched_at,
                "method": {
                    "books": f"/api/groups/{GROUP_ID}/books",
                    "toc": "window.appData → book.toc",
                    "doc": "/api/docs/{slug}?book_id={id}&mode=markdown → data.sourcecode",
                },
                "books": results,
            },
            ensure_ascii=False,
            indent=1,
        ),
        encoding="utf-8",
    )
    total = sum(len(book["docs"]) for book in results)
    print(f"✓ 完成：{len(results)} 本手册 / {total} 篇 → {OUT_DIR.relative_to(ROOT)}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
