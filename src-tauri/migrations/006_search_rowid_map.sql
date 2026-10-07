-- 006_search_rowid_map.sql
-- 问题：FTS5 表的 note_id 是 UNINDEXED 列，`DELETE ... WHERE note_id = ?` 必须全表扫描，
-- 导致「保存 / 采纳」每篇都要扫一遍索引 → 1 万篇首次导入变成 O(N²)（实测 6.5 分钟）。
-- 方案：用映射表保存 note_id → rowid，删除改按 rowid（主键），退化为 O(log N)。
--
-- 既有索引里没有映射记录，因此这里清空 FTS 表；应用下次打开工作区时会检测
-- 「已索引 < 总数」并自动重建一次（WorkspaceService::open）。

CREATE TABLE IF NOT EXISTS search_documents_map (
    note_id TEXT PRIMARY KEY,
    row_id  INTEGER NOT NULL UNIQUE
);

DELETE FROM search_documents_fts;
