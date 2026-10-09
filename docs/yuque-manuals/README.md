# 语雀官方公开手册归档（功能对标用）

> 抓取时间：2026-10-10T02:32:13+08:00 · 手册 25 本 · 文档 1222 篇

## 用途

把语雀官方帮助手册落到本地，用于 **查阅原件** 与 **功能对标**（对标结论见 [../FEATURE_PLANNING.md](../FEATURE_PLANNING.md) 第 7 节，原文摘录见 [../research/YUQUE_FEATURE_NOTES.md](../research/YUQUE_FEATURE_NOTES.md)）。

## 抓取方法（可复现）

```bash
python3 scripts/research/crawl-yuque-manuals.py            # 全量
python3 scripts/research/crawl-yuque-manuals.py --books thyzgp,gpvawt
```

| 数据 | 接口 / 位置 |
| --- | --- |
| 知识库列表 | `GET /api/groups/84140/books` |
| 目录树 | 页面内注入的 `window.appData`（URL-encoded JSON）→ `book.toc` |
| 文档正文 | `GET /api/docs/{slug}?book_id={id}&mode=markdown` → `data.sourcecode` |

## 手册清单

| # | 手册 | 文档数 | 本地目录 | 原文 |
| --- | --- | --- | --- | --- |
| 1 | 语雀的天空 | 399 | [books/blog](./books/blog/README.md) | [语雀](https://www.yuque.com/yuque/blog) |
| 2 | 语雀更新日志 | 298 | [books/changelog](./books/changelog/README.md) | [语雀](https://www.yuque.com/yuque/changelog) |
| 3 | 开发者文档 | 75 | [books/developer](./books/developer/README.md) | [语雀](https://www.yuque.com/yuque/developer) |
| 4 | 语雀空间：在线文档协同 & 企业知识管理 | 66 | [books/organizations](./books/organizations/README.md) | [语雀](https://www.yuque.com/yuque/organizations) |
| 5 | 常见问题 | 57 | [books/qa](./books/qa/README.md) | [语雀](https://www.yuque.com/yuque/qa) |
| 6 | 文档编辑器 | 54 | [books/gpvawt](./books/gpvawt/README.md) | [语雀](https://www.yuque.com/yuque/gpvawt) |
| 7 | 移动客户端 | 48 | [books/mobile-app](./books/mobile-app/README.md) | [语雀](https://www.yuque.com/yuque/mobile-app) |
| 8 | 语雀桌面端 | 34 | [books/yuque-desktop](./books/yuque-desktop/README.md) | [语雀](https://www.yuque.com/yuque/yuque-desktop) |
| 9 | 知识库 | 33 | [books/thyzgp](./books/thyzgp/README.md) | [语雀](https://www.yuque.com/yuque/thyzgp) |
| 10 | 语雀：知识构建与分享 | 24 | [books/ng1qth](./books/ng1qth/README.md) | [语雀](https://www.yuque.com/yuque/ng1qth) |
| 11 | 数据表 | 21 | [books/cqb86p](./books/cqb86p/README.md) | [语雀](https://www.yuque.com/yuque/cqb86p) |
| 12 | 画板 | 19 | [books/wknab3](./books/wknab3/README.md) | [语雀](https://www.yuque.com/yuque/wknab3) |
| 13 | 小记 | 18 | [books/ekrzd1](./books/ekrzd1/README.md) | [语雀](https://www.yuque.com/yuque/ekrzd1) |
| 14 | 电子表格 | 17 | [books/vecyyc](./books/vecyyc/README.md) | [语雀](https://www.yuque.com/yuque/vecyyc) |
| 15 | 媒体报道 | 12 | [books/lpa7n6](./books/lpa7n6/README.md) | [语雀](https://www.yuque.com/yuque/lpa7n6) |
| 16 | 企业客户案例 | 11 | [books/ykl39r](./books/ykl39r/README.md) | [语雀](https://www.yuque.com/yuque/ykl39r) |
| 17 | 我的花园 | 9 | [books/mgcsc6](./books/mgcsc6/README.md) | [语雀](https://www.yuque.com/yuque/mgcsc6) |
| 18 | 新手视频手册 | 7 | [books/xnx62x](./books/xnx62x/README.md) | [语雀](https://www.yuque.com/yuque/xnx62x) |
| 19 | 语雀服务协议 | 7 | [books/qeyyk7](./books/qeyyk7/README.md) | [语雀](https://www.yuque.com/yuque/qeyyk7) |
| 20 | 浏览器插件 🔥 | 4 | [books/yuque-browser-extension](./books/yuque-browser-extension/README.md) | [语雀](https://www.yuque.com/yuque/yuque-browser-extension) |
| 21 | 个人使用案例 | 4 | [books/obbrpg](./books/obbrpg/README.md) | [语雀](https://www.yuque.com/yuque/obbrpg) |
| 22 | 加入语雀 | 2 | [books/careers](./books/careers/README.md) | [语雀](https://www.yuque.com/yuque/careers) |
| 23 | 如何从其他工具迁入语雀 | 2 | [books/eaghk3](./books/eaghk3/README.md) | [语雀](https://www.yuque.com/yuque/eaghk3) |
| 24 | 行业解决方案 | 1 | [books/rdglqp](./books/rdglqp/README.md) | [语雀](https://www.yuque.com/yuque/rdglqp) |
| 25 | 语雀功能讨论 | 0 | [books/ztnf11](./books/ztnf11/README.md) | [语雀](https://www.yuque.com/yuque/ztnf11) |

## 版权与使用边界

- 正文版权归 **语雀（蚂蚁集团）** 所有，这里只做**本地存档**用于个人学习与功能对标，**不要对外分发**；
- 内容为 2026-10-10 的**公开手册**快照，不含登录态下的私有空间内容；语雀迭代较快，需要时重跑脚本刷新；
- 归档正文默认**不纳入 Git**（见本目录 `.gitignore`），索引与抓取脚本会纳入版本管理；
  若需要把正文一起提交，删除 `.gitignore` 中的 `books/` 一行后重新 `git add` 即可。
