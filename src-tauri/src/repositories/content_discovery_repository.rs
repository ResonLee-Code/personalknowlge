//! 内容发现持久化：收件箱记录、候选链接和已抓取页面元数据。

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::time::now_iso;
use crate::domain::content_discovery::{
    ContentCapture, ContentCaptureStatus, ContentDiscoveryCandidate, ContentDiscoveryPage,
    ContentSourceKind,
};

pub trait ContentDiscoveryRepository {
    fn insert_capture(&self, conn: &Connection, capture: &ContentCapture) -> Result<()>;
    fn update_capture(&self, conn: &Connection, capture: &ContentCapture) -> Result<()>;
    fn get_capture(
        &self,
        conn: &Connection,
        workspace_id: &str,
        id: &str,
    ) -> Result<Option<ContentCapture>>;
    fn list_captures(
        &self,
        conn: &Connection,
        workspace_id: &str,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ContentCapture>>;
    fn find_capture_by_canonical(
        &self,
        conn: &Connection,
        workspace_id: &str,
        canonical_url: &str,
    ) -> Result<Option<String>>;
    fn replace_candidates(
        &self,
        conn: &Connection,
        capture_id: &str,
        candidates: &[ContentDiscoveryCandidate],
    ) -> Result<()>;
    fn list_candidates(
        &self,
        conn: &Connection,
        capture_id: &str,
    ) -> Result<Vec<ContentDiscoveryCandidate>>;
    fn upsert_page(
        &self,
        conn: &Connection,
        capture_id: &str,
        page: &ContentDiscoveryPage,
    ) -> Result<()>;
    fn list_pages(&self, conn: &Connection, capture_id: &str) -> Result<Vec<ContentDiscoveryPage>>;
}

pub struct SqliteContentDiscoveryRepository;

fn parse_json_array(raw: &str, field: &str) -> Result<Vec<String>> {
    serde_json::from_str(raw).map_err(|error| {
        AppError::new(
            ErrorCode::DatabaseError,
            format!("内容发现字段 {field} 不是合法 JSON 数组：{error}"),
        )
    })
}

fn map_capture(row: &Row<'_>) -> rusqlite::Result<ContentCapture> {
    let status: String = row.get("status")?;
    let source_kind: String = row.get("source_kind")?;
    let key_points: String = row.get("key_points_json")?;
    let tags: String = row.get("tags_json")?;
    let related: String = row.get("related_note_ids_json")?;
    Ok(ContentCapture {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        requested_url: row.get("requested_url")?,
        final_url: row.get("final_url")?,
        canonical_url: row.get("canonical_url")?,
        source_kind: ContentSourceKind::parse(&source_kind),
        title: row.get("title")?,
        author: row.get("author")?,
        site: row.get("site")?,
        published_at: row.get("published_at")?,
        captured_at: row.get("captured_at")?,
        user_note: row.get("user_note")?,
        status: ContentCaptureStatus::parse(&status),
        summary: row.get("summary")?,
        key_points: parse_json_array(&key_points, "key_points_json").unwrap_or_default(),
        tags: parse_json_array(&tags, "tags_json").unwrap_or_default(),
        related_note_ids: parse_json_array(&related, "related_note_ids_json").unwrap_or_default(),
        related_notes: Vec::new(),
        suggested_category_id: row.get("suggested_category_id")?,
        decision_provider: row.get("decision_provider")?,
        keep_full_text: row.get::<_, i64>("keep_full_text")? != 0,
        page_count: row.get::<_, i64>("page_count")?.max(0) as usize,
        word_count: row.get("word_count")?,
        cache_dir: row.get("cache_dir")?,
        error: row.get("error")?,
        note_id: row.get("note_id")?,
        duplicate_of: row.get("duplicate_of")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        saved_at: row.get("saved_at")?,
        source_changed: row.get::<_, i64>("source_changed")? != 0,
        last_refreshed_at: row.get("last_refreshed_at")?,
        candidates: Vec::new(),
        pages: Vec::new(),
        markdown: None,
    })
}

fn map_candidate(row: &Row<'_>) -> rusqlite::Result<ContentDiscoveryCandidate> {
    Ok(ContentDiscoveryCandidate {
        url: row.get("url")?,
        canonical_url: row.get("canonical_url")?,
        parent_url: row.get("parent_url")?,
        depth: row.get::<_, i64>("depth")?.max(0) as u32,
        title: row.get("title")?,
        anchor_text: row.get("anchor_text")?,
        context: row.get("context")?,
        hidden: row.get::<_, i64>("hidden")? != 0,
        same_origin: row.get::<_, i64>("same_origin")? != 0,
        same_site: row.get::<_, i64>("same_site")? != 0,
        eligible: row.get::<_, i64>("eligible")? != 0,
        blocked_reason: row.get("blocked_reason")?,
        relevance_score: row.get("relevance_score")?,
        relevance_reason: row.get("relevance_reason")?,
        selected: row.get::<_, i64>("selected")? != 0,
    })
}

fn map_page(row: &Row<'_>) -> rusqlite::Result<ContentDiscoveryPage> {
    Ok(ContentDiscoveryPage {
        url: row.get("url")?,
        canonical_url: row.get("canonical_url")?,
        parent_url: row.get("parent_url")?,
        depth: row.get::<_, i64>("depth")?.max(0) as u32,
        title: row.get("title")?,
        site: row.get("site")?,
        content_hash: row.get("content_hash")?,
        word_count: row.get("word_count")?,
        bytes: row.get::<_, i64>("bytes")?.max(0) as usize,
        cache_path: row.get("cache_path")?,
        status: row.get("status")?,
        fetched_at: row.get("fetched_at")?,
        error: row.get("error")?,
    })
}

impl ContentDiscoveryRepository for SqliteContentDiscoveryRepository {
    fn insert_capture(&self, conn: &Connection, capture: &ContentCapture) -> Result<()> {
        conn.execute(
            "INSERT INTO content_captures
             (id, workspace_id, requested_url, final_url, canonical_url, source_kind,
              title, author, site, published_at, captured_at, user_note, status,
              summary, key_points_json, tags_json, related_note_ids_json,
              suggested_category_id, decision_provider, keep_full_text, page_count,
              word_count, cache_dir, error, note_id, duplicate_of, created_at,
              updated_at, saved_at, source_changed, last_refreshed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                     ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31)",
            params![
                capture.id,
                capture.workspace_id,
                capture.requested_url,
                capture.final_url,
                capture.canonical_url,
                capture.source_kind.as_str(),
                capture.title,
                capture.author,
                capture.site,
                capture.published_at,
                capture.captured_at,
                capture.user_note,
                capture.status.as_str(),
                capture.summary,
                serde_json::to_string(&capture.key_points).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&capture.tags).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&capture.related_note_ids)
                    .unwrap_or_else(|_| "[]".to_string()),
                capture.suggested_category_id,
                capture.decision_provider,
                capture.keep_full_text as i64,
                capture.page_count as i64,
                capture.word_count,
                capture.cache_dir,
                capture.error,
                capture.note_id,
                capture.duplicate_of,
                capture.created_at,
                capture.updated_at,
                capture.saved_at,
                capture.source_changed as i64,
                capture.last_refreshed_at,
            ],
        )?;
        Ok(())
    }

    fn update_capture(&self, conn: &Connection, capture: &ContentCapture) -> Result<()> {
        conn.execute(
            "UPDATE content_captures SET
               requested_url = ?2, final_url = ?3, canonical_url = ?4, source_kind = ?5,
               title = ?6, author = ?7, site = ?8, published_at = ?9, captured_at = ?10,
               user_note = ?11, status = ?12, summary = ?13, key_points_json = ?14,
               tags_json = ?15, related_note_ids_json = ?16, decision_provider = ?17,
               suggested_category_id = ?18, keep_full_text = ?19, page_count = ?20,
               word_count = ?21, cache_dir = ?22, error = ?23, note_id = ?24,
               duplicate_of = ?25, updated_at = ?26, saved_at = ?27,
               source_changed = ?28, last_refreshed_at = ?29
             WHERE id = ?1 AND workspace_id = ?30",
            params![
                capture.id,
                capture.requested_url,
                capture.final_url,
                capture.canonical_url,
                capture.source_kind.as_str(),
                capture.title,
                capture.author,
                capture.site,
                capture.published_at,
                capture.captured_at,
                capture.user_note,
                capture.status.as_str(),
                capture.summary,
                serde_json::to_string(&capture.key_points).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&capture.tags).unwrap_or_else(|_| "[]".to_string()),
                serde_json::to_string(&capture.related_note_ids)
                    .unwrap_or_else(|_| "[]".to_string()),
                capture.decision_provider,
                capture.suggested_category_id,
                capture.keep_full_text as i64,
                capture.page_count as i64,
                capture.word_count,
                capture.cache_dir,
                capture.error,
                capture.note_id,
                capture.duplicate_of,
                capture.updated_at,
                capture.saved_at,
                capture.source_changed as i64,
                capture.last_refreshed_at,
                capture.workspace_id,
            ],
        )?;
        Ok(())
    }

    fn get_capture(
        &self,
        conn: &Connection,
        workspace_id: &str,
        id: &str,
    ) -> Result<Option<ContentCapture>> {
        conn.query_row(
            "SELECT * FROM content_captures WHERE workspace_id = ?1 AND id = ?2",
            params![workspace_id, id],
            map_capture,
        )
        .optional()
        .map_err(Into::into)
    }

    fn list_captures(
        &self,
        conn: &Connection,
        workspace_id: &str,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ContentCapture>> {
        let limit = limit.clamp(1, 200) as i64;
        let captures = match status.filter(|value| !value.trim().is_empty()) {
            Some(status) => {
                let mut statement = conn.prepare(
                    "SELECT * FROM content_captures
                     WHERE workspace_id = ?1 AND status = ?2
                     ORDER BY updated_at DESC LIMIT ?3",
                )?;
                let rows = statement
                    .query_map(params![workspace_id, status, limit], map_capture)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            }
            None => {
                let mut statement = conn.prepare(
                    "SELECT * FROM content_captures
                     WHERE workspace_id = ?1 ORDER BY updated_at DESC LIMIT ?2",
                )?;
                let rows = statement
                    .query_map(params![workspace_id, limit], map_capture)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                rows
            }
        };
        Ok(captures)
    }

    fn find_capture_by_canonical(
        &self,
        conn: &Connection,
        workspace_id: &str,
        canonical_url: &str,
    ) -> Result<Option<String>> {
        conn.query_row(
            "SELECT id FROM content_captures
             WHERE workspace_id = ?1 AND canonical_url = ?2 AND status <> 'ignored'
             ORDER BY updated_at DESC LIMIT 1",
            params![workspace_id, canonical_url],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    fn replace_candidates(
        &self,
        conn: &Connection,
        capture_id: &str,
        candidates: &[ContentDiscoveryCandidate],
    ) -> Result<()> {
        let now = now_iso();
        conn.execute(
            "DELETE FROM content_discovery_candidates WHERE capture_id = ?1",
            params![capture_id],
        )?;
        let mut statement = conn.prepare(
            "INSERT OR REPLACE INTO content_discovery_candidates
             (capture_id, url, canonical_url, parent_url, depth, title, anchor_text,
              context, hidden, same_origin, same_site, eligible, blocked_reason, relevance_score,
              relevance_reason, selected, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        )?;
        for candidate in candidates {
            statement.execute(params![
                capture_id,
                candidate.url,
                candidate.canonical_url,
                candidate.parent_url,
                candidate.depth as i64,
                candidate.title,
                candidate.anchor_text,
                candidate.context,
                candidate.hidden as i64,
                candidate.same_origin as i64,
                candidate.same_site as i64,
                candidate.eligible as i64,
                candidate.blocked_reason,
                candidate.relevance_score,
                candidate.relevance_reason,
                candidate.selected as i64,
                now,
            ])?;
        }
        Ok(())
    }

    fn list_candidates(
        &self,
        conn: &Connection,
        capture_id: &str,
    ) -> Result<Vec<ContentDiscoveryCandidate>> {
        let mut statement = conn.prepare(
            "SELECT * FROM content_discovery_candidates
             WHERE capture_id = ?1 ORDER BY eligible DESC, depth ASC, relevance_score DESC, url ASC",
        )?;
        let rows = statement.query_map(params![capture_id], map_candidate)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    fn upsert_page(
        &self,
        conn: &Connection,
        capture_id: &str,
        page: &ContentDiscoveryPage,
    ) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO content_discovery_pages
             (capture_id, url, canonical_url, parent_url, depth, title, site,
              content_hash, word_count, bytes, cache_path, status, fetched_at, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                capture_id,
                page.url,
                page.canonical_url,
                page.parent_url,
                page.depth as i64,
                page.title,
                page.site,
                page.content_hash,
                page.word_count,
                page.bytes as i64,
                page.cache_path,
                page.status,
                page.fetched_at,
                page.error,
            ],
        )?;
        Ok(())
    }

    fn list_pages(&self, conn: &Connection, capture_id: &str) -> Result<Vec<ContentDiscoveryPage>> {
        let mut statement = conn.prepare(
            "SELECT * FROM content_discovery_pages
             WHERE capture_id = ?1 ORDER BY depth ASC, fetched_at ASC, url ASC",
        )?;
        let rows = statement.query_map(params![capture_id], map_page)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}
