-- 双向链接（设计文档 27 章 V2.0「知识关联」的数据基础）
--
-- 该表是**可重建的派生数据**：任何时刻都能从 Markdown 正文重新解析得到，
-- 删除它不会影响用户内容。外链（http/https）也会记录，但不参与图谱。

CREATE TABLE IF NOT EXISTS note_links (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    workspace_id   TEXT NOT NULL,
    source_note_id TEXT NOT NULL,
    target_note_id TEXT,
    target_title   TEXT NOT NULL,
    target_anchor  TEXT,
    link_type      TEXT NOT NULL DEFAULT 'wikilink',
    context        TEXT,
    position       INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL,

    UNIQUE (source_note_id, target_title, position),

    FOREIGN KEY (workspace_id)
        REFERENCES workspaces(id)
        ON DELETE CASCADE,

    FOREIGN KEY (source_note_id)
        REFERENCES notes(id)
        ON DELETE CASCADE,

    FOREIGN KEY (target_note_id)
        REFERENCES notes(id)
        ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_note_links_source ON note_links(source_note_id);
CREATE INDEX IF NOT EXISTS idx_note_links_target ON note_links(target_note_id);
CREATE INDEX IF NOT EXISTS idx_note_links_title
    ON note_links(workspace_id, lower(target_title));
CREATE INDEX IF NOT EXISTS idx_note_links_type ON note_links(link_type);
