-- 全文搜索索引（设计文档 11.4）
--
-- title / content 保存原文，keywords 保存中文单字与双字扩展，
-- 使 unicode61 分词器下的中文查询可以按「双字短语」精确匹配。

CREATE VIRTUAL TABLE IF NOT EXISTS search_documents_fts
USING fts5(
    note_id UNINDEXED,
    title,
    content,
    keywords,
    tokenize = 'unicode61 remove_diacritics 2'
);
