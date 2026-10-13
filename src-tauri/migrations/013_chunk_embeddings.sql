-- 向量索引（D3 / N7 第二步）。
--
-- 只存**向量与指纹**，不存正文：正文是 Markdown 文件的投影，
-- 分块由 `note_chunks` 描述；删掉这张表只是让下次重建一遍。
--
-- `content_hash` 是分块内容的指纹：**内容没变就不重嵌**（增量构建），
-- `model` 进主键：换了嵌入模型要各存一份（不能混用不同模型的向量）。
-- `vector` 是 f32 小端数组（`dim` 个元素），本地暴力检索（见 vector_index.rs）。

CREATE TABLE IF NOT EXISTS chunk_embeddings (
    note_id      TEXT NOT NULL,
    chunk_index  INTEGER NOT NULL,
    model        TEXT NOT NULL,
    dim          INTEGER NOT NULL,
    vector       BLOB NOT NULL,
    content_hash TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    PRIMARY KEY (note_id, chunk_index, model)
);

CREATE INDEX IF NOT EXISTS idx_chunk_embeddings_model ON chunk_embeddings(model);
