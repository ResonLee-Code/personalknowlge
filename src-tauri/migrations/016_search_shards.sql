-- Scale 2.0：4 个稳定 FTS 分片。
--
-- note_id 的稳定哈希决定唯一分片；既有记录保留在 shard 0，直至保存或重建时重新路由。
-- 多个 FTS 表各自维护 rowid，映射表显式记录 shard，避免跨表 rowid 混淆。

CREATE VIRTUAL TABLE IF NOT EXISTS search_documents_fts_1
USING fts5(
    note_id UNINDEXED,
    title,
    content,
    keywords,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE VIRTUAL TABLE IF NOT EXISTS search_documents_fts_2
USING fts5(
    note_id UNINDEXED,
    title,
    content,
    keywords,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE VIRTUAL TABLE IF NOT EXISTS search_documents_fts_3
USING fts5(
    note_id UNINDEXED,
    title,
    content,
    keywords,
    tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TABLE search_documents_map_v2 (
    note_id TEXT PRIMARY KEY,
    row_id  INTEGER NOT NULL,
    shard   INTEGER NOT NULL DEFAULT 0
);

INSERT INTO search_documents_map_v2 (note_id, row_id, shard)
SELECT note_id, row_id, 0 FROM search_documents_map;

DROP TABLE search_documents_map;
ALTER TABLE search_documents_map_v2 RENAME TO search_documents_map;

CREATE INDEX IF NOT EXISTS idx_search_documents_map_shard
    ON search_documents_map(shard);
