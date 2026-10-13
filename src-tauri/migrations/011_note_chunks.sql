-- 分块索引（N5：chunk 索引 + 引用定位）。
--
-- 为什么需要：检索命中要能**跳回原文的精确区间**，而不是只给一整篇笔记。
-- 这张表只存「分块在哪里」（标题路径 + 正文字符区间），**不存正文**——
-- 正文是 Markdown 文件的投影，随文件走；删库重扫时要能从文件完整重建。
--
-- 区间口径：正文（不含 frontmatter）里的 **Unicode 标量值（Rust `char`）下标**，
-- 与 `read_note` 返回的 `body`、编辑器里的正文是同一个坐标系。

CREATE TABLE IF NOT EXISTS note_chunks (
    note_id     TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    -- 标题路径（`A > B`）；空串 = 位于任何标题之前
    heading     TEXT NOT NULL,
    -- 正文字符区间 [char_start, char_end)
    char_start  INTEGER NOT NULL,
    char_end    INTEGER NOT NULL,
    PRIMARY KEY (note_id, chunk_index)
);

CREATE INDEX IF NOT EXISTS idx_note_chunks_note ON note_chunks(note_id);
