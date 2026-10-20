//! Organize P0 的持久化建议与任务队列。

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use crate::core::error::{AppError, ErrorCode, Result};
use crate::core::time::now_iso;
use crate::domain::organize::{AiJob, AiJobItem, AiJobItemStatus, AiJobStatus};

#[derive(Debug, Clone)]
pub struct SuggestionRow {
    pub id: String,
    pub workspace_id: String,
    pub note_id: String,
    pub note_title: String,
    pub kind: String,
    pub status: String,
    pub model: String,
    pub prompt_version: String,
    pub evidence: Value,
    pub payload: Value,
    pub source_hash: String,
    pub before_raw: Option<String>,
    pub applied_hash: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub applied_at: Option<String>,
    pub undone_at: Option<String>,
    pub error: Option<String>,
}

pub trait AiOrganizeRepository {
    fn replace_pending(&self, conn: &Connection, row: &SuggestionRow) -> Result<()>;
    fn get_suggestion(&self, conn: &Connection, id: &str) -> Result<Option<SuggestionRow>>;
    fn list_suggestions(
        &self,
        conn: &Connection,
        workspace_id: &str,
        status: Option<&str>,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SuggestionRow>>;
    fn reject_suggestion(&self, conn: &Connection, id: &str) -> Result<bool>;
    fn accept_suggestion(
        &self,
        conn: &Connection,
        id: &str,
        before_raw: &str,
        applied_hash: &str,
    ) -> Result<bool>;
    fn undo_suggestion(
        &self,
        conn: &Connection,
        id: &str,
    ) -> Result<Option<(String, Option<String>)>>;

    fn create_job(&self, conn: &Connection, job: &AiJob, note_ids: &[String]) -> Result<()>;
    fn get_job(&self, conn: &Connection, id: &str) -> Result<Option<AiJob>>;
    fn list_jobs(&self, conn: &Connection, workspace_id: &str, limit: usize) -> Result<Vec<AiJob>>;
    fn set_job_status(
        &self,
        conn: &Connection,
        id: &str,
        status: AiJobStatus,
        error: Option<&str>,
    ) -> Result<()>;
    fn next_pending_item(&self, conn: &Connection, job_id: &str) -> Result<Option<AiJobItem>>;
    fn list_job_items(&self, conn: &Connection, job_id: &str) -> Result<Vec<AiJobItem>>;
    fn requeue_failed_items(&self, conn: &Connection, job_id: &str) -> Result<usize>;
    fn finish_job_item(
        &self,
        conn: &Connection,
        job_id: &str,
        item_index: usize,
        status: AiJobItemStatus,
        error: Option<&str>,
    ) -> Result<()>;
    fn refresh_job_counts(&self, conn: &Connection, job_id: &str) -> Result<()>;
    fn recover_interrupted(&self, conn: &Connection, workspace_id: &str) -> Result<usize>;
}

pub struct SqliteAiOrganizeRepository;

const SUGGESTION_COLUMNS: &str = "
    s.id, s.workspace_id, s.note_id, n.title AS note_title,
    s.kind, s.status, s.model, s.prompt_version, s.evidence_json,
    s.payload_json, s.source_hash, s.before_raw, s.applied_hash, s.created_at,
    s.updated_at, s.applied_at, s.undone_at, s.error";

fn parse_json(raw: &str, field: &str) -> Result<Value> {
    serde_json::from_str(raw).map_err(|error| {
        AppError::new(
            ErrorCode::DatabaseError,
            format!("Organize 建议字段 {field} 不是合法 JSON：{error}"),
        )
    })
}

struct RawSuggestionRow {
    id: String,
    workspace_id: String,
    note_id: String,
    note_title: String,
    kind: String,
    status: String,
    model: String,
    prompt_version: String,
    evidence_json: String,
    payload_json: String,
    source_hash: String,
    before_raw: Option<String>,
    applied_hash: Option<String>,
    created_at: String,
    updated_at: String,
    applied_at: Option<String>,
    undone_at: Option<String>,
    error: Option<String>,
}

fn map_suggestion(row: &Row<'_>) -> rusqlite::Result<RawSuggestionRow> {
    Ok(RawSuggestionRow {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        note_id: row.get("note_id")?,
        note_title: row.get("note_title")?,
        kind: row.get("kind")?,
        status: row.get("status")?,
        model: row.get("model")?,
        prompt_version: row.get("prompt_version")?,
        evidence_json: row.get("evidence_json")?,
        payload_json: row.get("payload_json")?,
        source_hash: row.get("source_hash")?,
        before_raw: row.get("before_raw")?,
        applied_hash: row.get("applied_hash")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        applied_at: row.get("applied_at")?,
        undone_at: row.get("undone_at")?,
        error: row.get("error")?,
    })
}

fn finish_suggestion(row: RawSuggestionRow) -> Result<SuggestionRow> {
    Ok(SuggestionRow {
        id: row.id,
        workspace_id: row.workspace_id,
        note_id: row.note_id,
        note_title: row.note_title,
        kind: row.kind,
        status: row.status,
        model: row.model,
        prompt_version: row.prompt_version,
        evidence: parse_json(&row.evidence_json, "evidence_json")?,
        payload: parse_json(&row.payload_json, "payload_json")?,
        source_hash: row.source_hash,
        before_raw: row.before_raw,
        applied_hash: row.applied_hash,
        created_at: row.created_at,
        updated_at: row.updated_at,
        applied_at: row.applied_at,
        undone_at: row.undone_at,
        error: row.error,
    })
}

fn map_job(row: &Row<'_>) -> rusqlite::Result<AiJob> {
    let payload_raw: String = row.get("payload_json")?;
    let payload = serde_json::from_str(&payload_raw).unwrap_or(Value::Null);
    let status_raw: String = row.get("status")?;
    Ok(AiJob {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        kind: row.get("kind")?,
        status: AiJobStatus::parse(&status_raw).unwrap_or(AiJobStatus::Failed),
        total: row.get::<_, i64>("total")?.max(0) as usize,
        completed: row.get::<_, i64>("completed")?.max(0) as usize,
        failed: row.get::<_, i64>("failed")?.max(0) as usize,
        payload,
        error: row.get("error")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        started_at: row.get("started_at")?,
        finished_at: row.get("finished_at")?,
    })
}

fn map_item(row: &Row<'_>) -> rusqlite::Result<AiJobItem> {
    let status_raw: String = row.get("status")?;
    Ok(AiJobItem {
        job_id: row.get("job_id")?,
        item_index: row.get::<_, i64>("item_index")?.max(0) as usize,
        note_id: row.get("note_id")?,
        status: AiJobItemStatus::parse(&status_raw).unwrap_or(AiJobItemStatus::Failed),
        attempts: row.get::<_, i64>("attempts")?.max(0) as u32,
        error: row.get("error")?,
    })
}

impl AiOrganizeRepository for SqliteAiOrganizeRepository {
    fn replace_pending(&self, conn: &Connection, row: &SuggestionRow) -> Result<()> {
        let now = now_iso();
        conn.execute(
            "UPDATE ai_suggestions SET status = 'superseded', updated_at = ?1
             WHERE workspace_id = ?2 AND note_id = ?3 AND kind = ?4 AND status = 'pending'",
            params![now, row.workspace_id, row.note_id, row.kind],
        )?;
        conn.execute(
            "INSERT INTO ai_suggestions
                (id, workspace_id, note_id, kind, status, model, prompt_version,
                 evidence_json, payload_json, source_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                row.id,
                row.workspace_id,
                row.note_id,
                row.kind,
                row.model,
                row.prompt_version,
                row.evidence.to_string(),
                row.payload.to_string(),
                row.source_hash,
                now,
            ],
        )?;
        Ok(())
    }

    fn get_suggestion(&self, conn: &Connection, id: &str) -> Result<Option<SuggestionRow>> {
        let sql = format!(
            "SELECT {SUGGESTION_COLUMNS}
             FROM ai_suggestions s JOIN notes n ON n.id = s.note_id
             WHERE s.id = ?1"
        );
        let mut statement = conn.prepare(&sql)?;
        let mut rows = statement.query(params![id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(finish_suggestion(map_suggestion(row)?)?))
    }

    fn list_suggestions(
        &self,
        conn: &Connection,
        workspace_id: &str,
        status: Option<&str>,
        kind: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SuggestionRow>> {
        let mut sql = format!(
            "SELECT {SUGGESTION_COLUMNS}
             FROM ai_suggestions s JOIN notes n ON n.id = s.note_id
             WHERE s.workspace_id = ?1"
        );
        if status.is_some() {
            sql.push_str(" AND s.status = ?2");
        }
        if kind.is_some() {
            sql.push_str(if status.is_some() {
                " AND s.kind = ?3"
            } else {
                " AND s.kind = ?2"
            });
        }
        let limit_index = 2 + usize::from(status.is_some()) + usize::from(kind.is_some());
        sql.push_str(&format!(
            " ORDER BY s.updated_at DESC, s.id DESC LIMIT ?{limit_index}"
        ));
        let mut statement = conn.prepare(&sql)?;
        let rows =
            match (status, kind) {
                (Some(status), Some(kind)) => statement.query_map(
                    params![workspace_id, status, kind, limit as i64],
                    map_suggestion,
                )?,
                (Some(status), None) => statement
                    .query_map(params![workspace_id, status, limit as i64], map_suggestion)?,
                (None, Some(kind)) => statement
                    .query_map(params![workspace_id, kind, limit as i64], map_suggestion)?,
                (None, None) => {
                    statement.query_map(params![workspace_id, limit as i64], map_suggestion)?
                }
            };
        let mut result = Vec::new();
        for row in rows {
            result.push(finish_suggestion(row?)?);
        }
        Ok(result)
    }

    fn reject_suggestion(&self, conn: &Connection, id: &str) -> Result<bool> {
        let changed = conn.execute(
            "UPDATE ai_suggestions SET status = 'rejected', updated_at = ?1
             WHERE id = ?2 AND status = 'pending'",
            params![now_iso(), id],
        )?;
        Ok(changed > 0)
    }

    fn accept_suggestion(
        &self,
        conn: &Connection,
        id: &str,
        before_raw: &str,
        applied_hash: &str,
    ) -> Result<bool> {
        let now = now_iso();
        let changed = conn.execute(
            "UPDATE ai_suggestions
             SET status = 'accepted', before_raw = ?1, applied_hash = ?2,
                 applied_at = ?3, undone_at = NULL, updated_at = ?3, error = NULL
             WHERE id = ?4 AND status = 'pending'",
            params![before_raw, applied_hash, now, id],
        )?;
        Ok(changed > 0)
    }

    fn undo_suggestion(
        &self,
        conn: &Connection,
        id: &str,
    ) -> Result<Option<(String, Option<String>)>> {
        let before: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT before_raw, applied_hash FROM ai_suggestions
                 WHERE id = ?1 AND status = 'accepted' AND undone_at IS NULL",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some(before) = before else {
            return Ok(None);
        };
        conn.execute(
            "UPDATE ai_suggestions SET undone_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now_iso(), id],
        )?;
        Ok(Some(before))
    }

    fn create_job(&self, conn: &Connection, job: &AiJob, note_ids: &[String]) -> Result<()> {
        let now = now_iso();
        conn.execute(
            "INSERT INTO ai_jobs
                (id, workspace_id, kind, status, total, completed, failed, payload_json,
                 error, created_at, updated_at, started_at, finished_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6, NULL, ?7, ?7, NULL, NULL)",
            params![
                job.id,
                job.workspace_id,
                job.kind,
                job.status.as_str(),
                note_ids.len() as i64,
                job.payload.to_string(),
                now,
            ],
        )?;
        let mut statement = conn.prepare(
            "INSERT INTO ai_job_items
                (job_id, item_index, note_id, status, attempts, error, updated_at)
             VALUES (?1, ?2, ?3, 'pending', 0, NULL, ?4)",
        )?;
        for (index, note_id) in note_ids.iter().enumerate() {
            statement.execute(params![job.id, index as i64, note_id, now])?;
        }
        Ok(())
    }

    fn get_job(&self, conn: &Connection, id: &str) -> Result<Option<AiJob>> {
        conn.query_row(
            "SELECT id, workspace_id, kind, status, total, completed, failed, payload_json, error,
                    created_at, updated_at, started_at, finished_at
             FROM ai_jobs WHERE id = ?1",
            params![id],
            map_job,
        )
        .optional()
        .map_err(Into::into)
    }

    fn list_jobs(&self, conn: &Connection, workspace_id: &str, limit: usize) -> Result<Vec<AiJob>> {
        let mut statement = conn.prepare(
            "SELECT id, workspace_id, kind, status, total, completed, failed, payload_json, error,
                    created_at, updated_at, started_at, finished_at
             FROM ai_jobs WHERE workspace_id = ?1
             ORDER BY updated_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![workspace_id, limit as i64], map_job)?;
        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }

    fn set_job_status(
        &self,
        conn: &Connection,
        id: &str,
        status: AiJobStatus,
        error: Option<&str>,
    ) -> Result<()> {
        let now = now_iso();
        let started_at = matches!(status, AiJobStatus::Running).then_some(now.as_str());
        let finished_at = matches!(
            status,
            AiJobStatus::Completed | AiJobStatus::Canceled | AiJobStatus::Failed
        )
        .then_some(now.as_str());
        conn.execute(
            "UPDATE ai_jobs
             SET status = ?1, error = ?2, updated_at = ?3,
                 started_at = CASE WHEN ?1 = 'running' THEN COALESCE(started_at, ?4) ELSE started_at END,
                 finished_at = CASE WHEN ?1 IN ('completed','canceled','failed') THEN ?4 ELSE finished_at END
             WHERE id = ?5",
            params![status.as_str(), error, now, started_at.or(finished_at), id],
        )?;
        Ok(())
    }

    fn next_pending_item(&self, conn: &Connection, job_id: &str) -> Result<Option<AiJobItem>> {
        let row = conn
            .query_row(
                "SELECT job_id, item_index, note_id, status, attempts, error
                 FROM ai_job_items WHERE job_id = ?1 AND status = 'pending'
                 ORDER BY item_index ASC LIMIT 1",
                params![job_id],
                map_item,
            )
            .optional()?;
        if let Some(item) = &row {
            conn.execute(
                "UPDATE ai_job_items SET status = 'running', attempts = attempts + 1, updated_at = ?1
                 WHERE job_id = ?2 AND item_index = ?3",
                params![now_iso(), item.job_id, item.item_index as i64],
            )?;
        }
        Ok(row)
    }

    fn list_job_items(&self, conn: &Connection, job_id: &str) -> Result<Vec<AiJobItem>> {
        let mut statement = conn.prepare(
            "SELECT job_id, item_index, note_id, status, attempts, error
             FROM ai_job_items WHERE job_id = ?1 ORDER BY item_index ASC",
        )?;
        let rows = statement.query_map(params![job_id], map_item)?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    }

    fn requeue_failed_items(&self, conn: &Connection, job_id: &str) -> Result<usize> {
        Ok(conn.execute(
            "UPDATE ai_job_items SET status = 'pending', error = NULL, updated_at = ?1
             WHERE job_id = ?2 AND status IN ('failed', 'running')",
            params![now_iso(), job_id],
        )?)
    }

    fn finish_job_item(
        &self,
        conn: &Connection,
        job_id: &str,
        item_index: usize,
        status: AiJobItemStatus,
        error: Option<&str>,
    ) -> Result<()> {
        conn.execute(
            "UPDATE ai_job_items SET status = ?1, error = ?2, updated_at = ?3
             WHERE job_id = ?4 AND item_index = ?5",
            params![status.as_str(), error, now_iso(), job_id, item_index as i64],
        )?;
        Ok(())
    }

    fn refresh_job_counts(&self, conn: &Connection, job_id: &str) -> Result<()> {
        conn.execute(
            "UPDATE ai_jobs
             SET completed = (
                    SELECT COUNT(*) FROM ai_job_items WHERE job_id = ?1 AND status = 'completed'
                 ),
                 failed = (
                    SELECT COUNT(*) FROM ai_job_items WHERE job_id = ?1 AND status = 'failed'
                 ),
                 updated_at = ?2
             WHERE id = ?1",
            params![job_id, now_iso()],
        )?;
        Ok(())
    }

    fn recover_interrupted(&self, conn: &Connection, workspace_id: &str) -> Result<usize> {
        let now = now_iso();
        let count = conn.execute(
            "UPDATE ai_jobs SET status = 'paused', updated_at = ?1
             WHERE workspace_id = ?2 AND status = 'running'",
            params![now, workspace_id],
        )?;
        conn.execute(
            "UPDATE ai_job_items SET status = 'pending', updated_at = ?1
             WHERE status = 'running' AND job_id IN (
                 SELECT id FROM ai_jobs WHERE workspace_id = ?2 AND status = 'paused'
             )",
            params![now, workspace_id],
        )?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::db::Db;
    use serde_json::json;

    fn seeded_db() -> Db {
        let db = Db::in_memory().expect("内存数据库");
        db.with_conn(|conn| {
            conn.execute(
                "INSERT INTO workspaces (id, name, root_path, created_at, updated_at)
                 VALUES ('ws', '测试', '/tmp/ws', '2026-10-20T00:00:00+08:00',
                         '2026-10-20T00:00:00+08:00')",
                [],
            )?;
            conn.execute(
                "INSERT INTO notes
                    (id, workspace_id, title, file_path, status, created_at, updated_at)
                 VALUES ('note_a', 'ws', '索引设计', '00_Inbox/索引设计.md', 'active',
                         '2026-10-20T00:00:00+08:00', '2026-10-20T00:00:00+08:00')",
                [],
            )?;
            Ok(())
        })
        .expect("测试数据");
        db
    }

    fn suggestion(id: &str, source_hash: &str) -> SuggestionRow {
        SuggestionRow {
            id: id.to_string(),
            workspace_id: "ws".to_string(),
            note_id: "note_a".to_string(),
            note_title: "索引设计".to_string(),
            kind: "tags".to_string(),
            status: "pending".to_string(),
            model: "rule.organize".to_string(),
            prompt_version: "v1".to_string(),
            evidence: json!([
                {
                    "noteId": "note_a",
                    "title": "索引设计",
                    "heading": "FTS5",
                    "snippet": "SQLite FTS5 使用倒排索引。",
                    "charStart": 0,
                    "charEnd": 20
                }
            ]),
            payload: json!({
                "type": "tags",
                "tags": [
                    { "tag": "SQLite", "reason": "正文直接提到", "existing": false }
                ]
            }),
            source_hash: source_hash.to_string(),
            before_raw: None,
            applied_hash: None,
            created_at: "2026-10-20T00:00:00+08:00".to_string(),
            updated_at: "2026-10-20T00:00:00+08:00".to_string(),
            applied_at: None,
            undone_at: None,
            error: None,
        }
    }

    fn job(id: &str) -> AiJob {
        AiJob {
            id: id.to_string(),
            workspace_id: "ws".to_string(),
            kind: "organize.tags".to_string(),
            status: AiJobStatus::Queued,
            total: 2,
            completed: 0,
            failed: 0,
            payload: json!({ "kind": "tags" }),
            error: None,
            created_at: "2026-10-20T00:00:00+08:00".to_string(),
            updated_at: "2026-10-20T00:00:00+08:00".to_string(),
            started_at: None,
            finished_at: None,
        }
    }

    #[test]
    fn only_the_latest_pending_suggestion_survives_and_accept_undo_keeps_raw_bytes() {
        let db = seeded_db();
        let repo = SqliteAiOrganizeRepository;
        db.with_conn(|conn| {
            repo.replace_pending(conn, &suggestion("s1", "hash-old"))?;
            repo.replace_pending(conn, &suggestion("s2", "hash-new"))?;
            let pending = repo.list_suggestions(conn, "ws", Some("pending"), Some("tags"), 10)?;
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].id, "s2");
            assert_eq!(pending[0].payload["type"], "tags");

            assert!(repo.accept_suggestion(conn, "s2", "原始文件字节\n", "接受后哈希")?);
            assert!(!repo.accept_suggestion(conn, "s2", "再次接受", "另一个哈希")?);
            let accepted = repo.get_suggestion(conn, "s2")?.expect("已接受建议");
            assert_eq!(accepted.status, "accepted");
            assert_eq!(accepted.before_raw.as_deref(), Some("原始文件字节\n"));
            assert_eq!(accepted.applied_hash.as_deref(), Some("接受后哈希"));

            assert_eq!(
                repo.undo_suggestion(conn, "s2")?,
                Some(("原始文件字节\n".to_string(), Some("接受后哈希".to_string())))
            );
            let undone = repo.get_suggestion(conn, "s2")?.expect("撤销记录");
            assert!(undone.undone_at.is_some());
            assert!(!repo.reject_suggestion(conn, "s2")?, "已接受记录不能再拒绝");
            Ok(())
        })
        .expect("建议状态机");
    }

    #[test]
    fn jobs_persist_items_counts_and_recover_running_work_as_paused() {
        let db = seeded_db();
        let repo = SqliteAiOrganizeRepository;
        let duplicate = db.with_transaction(|conn| {
            repo.create_job(
                conn,
                &job("job1"),
                &["note_a".to_string(), "note_a".to_string()],
            )
        });
        assert!(duplicate.is_err(), "同一任务不允许重复笔记");
        db.with_conn(|conn| {
            repo.create_job(conn, &job("job1"), &["note_a".to_string()])?;
            let item = repo.next_pending_item(conn, "job1")?.expect("待处理项");
            assert_eq!(item.note_id, "note_a");
            assert_eq!(item.status, AiJobItemStatus::Pending);
            repo.set_job_status(conn, "job1", AiJobStatus::Running, None)?;

            let recovered = repo.recover_interrupted(conn, "ws")?;
            assert_eq!(recovered, 1);
            let recovered_item = repo.next_pending_item(conn, "job1")?.expect("恢复后可重试");
            assert_eq!(recovered_item.status, AiJobItemStatus::Pending);
            repo.finish_job_item(
                conn,
                "job1",
                recovered_item.item_index,
                AiJobItemStatus::Completed,
                None,
            )?;
            repo.refresh_job_counts(conn, "job1")?;
            let saved = repo.get_job(conn, "job1")?.expect("任务");
            assert_eq!(saved.status, AiJobStatus::Paused);
            assert_eq!(saved.completed, 1);
            Ok(())
        })
        .expect("任务队列状态机");
    }
}
