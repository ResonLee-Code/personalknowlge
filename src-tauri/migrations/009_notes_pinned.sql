-- 009_notes_pinned.sql
-- 置顶文档（对标语雀「置顶文档」）。
--
-- 与 parent_id / sort_order 一样是**派生列**：来源是 frontmatter 的 `pinned: true`，
-- 删除数据库后重新扫描 Markdown 即可重建。
-- 置顶在所有排序方式下都排在最前面（ORDER BY pinned DESC, <原有排序键>）。

ALTER TABLE notes ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
