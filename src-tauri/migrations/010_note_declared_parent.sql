-- 010_note_declared_parent.sql
-- 子文档声明的**原文**：frontmatter 的 `parent:` 既可能写父笔记 id，也可能写标题。
--
-- `parent_id` 只保存「解析结果」，解析不到时（父文档还没出现）就丢了线索，
-- 只能等这篇子文档下次被外部编辑才重新解析——已知限制：「父文档晚于子文档出现时，
-- 层级要等下次编辑才补上」。这里额外投影一份声明原文，扫描每轮都能对
-- 「声明了父文档但尚未解析成功」的子文档重试一次，无需为找它们去读全库文件。
--
-- 与 parent_id / sort_order / pinned 一样是**可重建的派生数据**：
-- 删掉数据库重新扫描 Markdown 即可从 frontmatter 复原。

ALTER TABLE notes ADD COLUMN declared_parent TEXT;

CREATE INDEX IF NOT EXISTS idx_notes_declared_parent
    ON notes(workspace_id, declared_parent);
