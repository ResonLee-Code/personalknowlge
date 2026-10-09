-- 008_notes_sort_order.sql
-- 手动排序（自定义顺序）：支持把笔记拖拽到同层的新位置。
--
-- 与 parent_id 一样是**派生列**：来源是 frontmatter 的 `order:`（1 起，越小越靠前），
-- 删除数据库后重新扫描 Markdown 即可完整重建。
-- 0 表示「未显式排序」，此时按 updated_at 兜底，保证旧库升级后顺序不变。

ALTER TABLE notes ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_notes_sort_order ON notes(sort_order);
