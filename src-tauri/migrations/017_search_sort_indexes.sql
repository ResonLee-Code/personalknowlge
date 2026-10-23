-- Scale 2.0：高频检索的显式排序索引。
--
-- 检索命中超过阈值时，先把命中 note_id 收进临时表，再沿这些覆盖索引按用户
-- 选择顺序扫描并提前截断。索引包含 status 作为首列，保证只遍历活跃笔记；
-- id 作为末列，和查询的稳定次级键完全一致。

CREATE INDEX IF NOT EXISTS idx_notes_search_updated
    ON notes(status, updated_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_notes_search_created
    ON notes(status, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_notes_search_title
    ON notes(status, title COLLATE NOCASE ASC, id ASC);
