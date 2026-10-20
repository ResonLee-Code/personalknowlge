-- Organize P0：持久化建议与后台任务。
--
-- 建议不会直接改 Markdown。只有用户接受后才进入常规笔记写入路径；
-- `before_raw` 保存接受前的完整文件文本，用于字节级撤销。

CREATE TABLE IF NOT EXISTS ai_suggestions (
    id             TEXT PRIMARY KEY,
    workspace_id   TEXT NOT NULL,
    note_id        TEXT NOT NULL,
    kind           TEXT NOT NULL,
    status         TEXT NOT NULL DEFAULT 'pending',
    model          TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    evidence_json  TEXT NOT NULL,
    payload_json   TEXT NOT NULL,
    source_hash    TEXT NOT NULL,
    before_raw     TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    applied_at     TEXT,
    undone_at      TEXT,
    error          TEXT
);

-- 同一篇笔记同一类只保留一条待确认建议。生成新建议时旧 pending 先标 superseded。
CREATE UNIQUE INDEX IF NOT EXISTS idx_ai_suggestions_pending_unique
    ON ai_suggestions(workspace_id, note_id, kind)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_ai_suggestions_workspace_status
    ON ai_suggestions(workspace_id, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_ai_suggestions_note
    ON ai_suggestions(workspace_id, note_id, updated_at DESC);

CREATE TABLE IF NOT EXISTS ai_jobs (
    id           TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    kind         TEXT NOT NULL,
    status       TEXT NOT NULL,
    total        INTEGER NOT NULL,
    completed    INTEGER NOT NULL DEFAULT 0,
    failed       INTEGER NOT NULL DEFAULT 0,
    payload_json TEXT NOT NULL,
    error        TEXT,
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    started_at   TEXT,
    finished_at  TEXT
);

CREATE INDEX IF NOT EXISTS idx_ai_jobs_workspace_status
    ON ai_jobs(workspace_id, status, updated_at DESC);

CREATE TABLE IF NOT EXISTS ai_job_items (
    job_id      TEXT NOT NULL,
    item_index  INTEGER NOT NULL,
    note_id     TEXT NOT NULL,
    status      TEXT NOT NULL,
    attempts    INTEGER NOT NULL DEFAULT 0,
    error       TEXT,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (job_id, item_index),
    UNIQUE (job_id, note_id),
    FOREIGN KEY (job_id) REFERENCES ai_jobs(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_ai_job_items_status
    ON ai_job_items(job_id, status, item_index ASC);
