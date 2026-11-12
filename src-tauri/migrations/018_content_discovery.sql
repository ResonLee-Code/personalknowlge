CREATE TABLE IF NOT EXISTS content_captures (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    requested_url TEXT NOT NULL,
    final_url TEXT NOT NULL,
    canonical_url TEXT NOT NULL,
    source_kind TEXT NOT NULL,
    title TEXT NOT NULL,
    author TEXT,
    site TEXT,
    published_at TEXT,
    captured_at TEXT NOT NULL,
    user_note TEXT,
    status TEXT NOT NULL,
    summary TEXT,
    key_points_json TEXT NOT NULL DEFAULT '[]',
    tags_json TEXT NOT NULL DEFAULT '[]',
    related_note_ids_json TEXT NOT NULL DEFAULT '[]',
    suggested_category_id TEXT,
    decision_provider TEXT,
    keep_full_text INTEGER NOT NULL DEFAULT 0,
    page_count INTEGER NOT NULL DEFAULT 0,
    word_count INTEGER NOT NULL DEFAULT 0,
    cache_dir TEXT NOT NULL,
    error TEXT,
    note_id TEXT,
    duplicate_of TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    saved_at TEXT,
    source_changed INTEGER NOT NULL DEFAULT 0,
    last_refreshed_at TEXT,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_content_captures_workspace_updated
    ON content_captures(workspace_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_content_captures_canonical
    ON content_captures(workspace_id, canonical_url);
CREATE INDEX IF NOT EXISTS idx_content_captures_status
    ON content_captures(workspace_id, status);

CREATE TABLE IF NOT EXISTS content_discovery_candidates (
    capture_id TEXT NOT NULL,
    url TEXT NOT NULL,
    canonical_url TEXT NOT NULL,
    parent_url TEXT,
    depth INTEGER NOT NULL DEFAULT 1,
    title TEXT NOT NULL,
    anchor_text TEXT,
    context TEXT,
    hidden INTEGER NOT NULL DEFAULT 0,
    same_origin INTEGER NOT NULL DEFAULT 1,
    same_site INTEGER NOT NULL DEFAULT 1,
    eligible INTEGER NOT NULL DEFAULT 1,
    blocked_reason TEXT,
    relevance_score REAL,
    relevance_reason TEXT,
    selected INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    PRIMARY KEY (capture_id, canonical_url),
    FOREIGN KEY (capture_id) REFERENCES content_captures(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_content_candidates_capture
    ON content_discovery_candidates(capture_id, depth, url);

CREATE TABLE IF NOT EXISTS content_discovery_pages (
    capture_id TEXT NOT NULL,
    url TEXT NOT NULL,
    canonical_url TEXT NOT NULL,
    parent_url TEXT,
    depth INTEGER NOT NULL DEFAULT 0,
    title TEXT NOT NULL,
    site TEXT,
    content_hash TEXT NOT NULL,
    word_count INTEGER NOT NULL DEFAULT 0,
    bytes INTEGER NOT NULL DEFAULT 0,
    cache_path TEXT NOT NULL,
    status TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    error TEXT,
    PRIMARY KEY (capture_id, canonical_url),
    FOREIGN KEY (capture_id) REFERENCES content_captures(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_content_pages_capture
    ON content_discovery_pages(capture_id, depth, url);
