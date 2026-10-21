# V2.0.x FTS 4 分片 10 万检索复测

## 环境

- 日期：2026-10-22
- 平台：macOS 27 / arm64 / 24 GB / 15 核
- schema：16
- 命令：`pnpm bench --only search --search-notes 100000`

## 结果

| 指标 | 实测 | 红线 |
| --- | ---: | ---: |
| 检索平均 | 102.44 ms | — |
| 检索 p95 | **190.69 ms** | < 200 ms |
| recall@5 | **100%** | 100% |
| 10 万后端重开 | **1246.09 ms** | < 2000 ms |
| RSS | **344.64 MB** | < 1536 MB |
| database.sqlite | 307.04 MB | — |
| 扫描采纳 | 43.68 s | < 600 s |
| FTS 重建 | 54.67 s | — |

1 万篇发布规格复测：平均 9.84 ms、p95 **18.09 ms**、recall@5 **100%**。

机器可读摘要：

```text
PKOS_BENCH search notes=100000 avg_ms=102.44 p95_ms=190.69 recall_at1_pct=65.0 recall_at5_pct=100.0 db_mb=307.04 db_kb_per_note=3.1 scan_s=43.68 fts_s=54.67 writes_s=5.59 reopen_ms=1246.09 rss_mb=344.64
```

## 说明

4 分片跨表查询会增加每轮固定 SQL 开销。当前 p95 仍过线，但只比红线低约 9 ms；
后续若增加更多分片、扩大摘要工作或提高每次返回条数，必须重新跑本门禁。
