-- 007_note_parent.sql
-- 子文档（父子层级）：父子关系由来源笔记 frontmatter 的 `parent:` 声明，
-- 这里把它投影成 parent_id 列（可重建的派生数据），用于列表按层级展示与「创建子文档」。
--
-- 取值优先级：优先写父笔记 id（稳定，不随标题变化）；手写标题时按标题解析后落库。
-- 父笔记被删除时子文档仍保留（parent_id 指向已删除笔记，列表按根级展示）。

ALTER TABLE notes ADD COLUMN parent_id TEXT;

CREATE INDEX IF NOT EXISTS idx_notes_parent ON notes(parent_id);
