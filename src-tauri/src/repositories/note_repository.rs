//! 笔记仓储。

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, Row};

use crate::core::error::Result;
use crate::domain::note::{Note, NoteListQuery, NoteSort, NoteStatus};

pub trait NoteRepository {
    fn insert(&self, conn: &Connection, note: &Note) -> Result<()>;
    fn update(&self, conn: &Connection, note: &Note) -> Result<()>;
    fn get(&self, conn: &Connection, id: &str) -> Result<Option<Note>>;
    fn get_by_path(
        &self,
        conn: &Connection,
        workspace_id: &str,
        file_path: &str,
    ) -> Result<Option<Note>>;
    fn list(
        &self,
        conn: &Connection,
        workspace_id: &str,
        query: &NoteListQuery,
    ) -> Result<(Vec<Note>, i64)>;
    fn list_by_status(
        &self,
        conn: &Connection,
        workspace_id: &str,
        status: NoteStatus,
    ) -> Result<Vec<Note>>;
    fn update_content_meta(
        &self,
        conn: &Connection,
        id: &str,
        updated_at: &str,
        content_hash: &str,
        word_count: i64,
        last_indexed_at: Option<&str>,
    ) -> Result<()>;
    fn update_identity(
        &self,
        conn: &Connection,
        id: &str,
        title: &str,
        file_path: &str,
        category_id: Option<&str>,
        updated_at: &str,
    ) -> Result<()>;
    fn set_category(&self, conn: &Connection, id: &str, category_id: Option<&str>) -> Result<()>;
    fn set_favorite(&self, conn: &Connection, id: &str, favorite: bool) -> Result<()>;
    /// 更新文件快照（mtime + size），扫描时用于短路
    fn set_file_stamp(
        &self,
        conn: &Connection,
        id: &str,
        file_size: Option<i64>,
        file_mtime: Option<&str>,
    ) -> Result<()>;

    /// 更新来源与时效（来源为 frontmatter 字段，写入时由 NoteService 同步到文件）
    fn set_source(
        &self,
        conn: &Connection,
        id: &str,
        source_type: Option<&str>,
        source_ref: Option<&str>,
        source_reliability: Option<i64>,
        valid_until: Option<&str>,
    ) -> Result<()>;
    fn set_status(
        &self,
        conn: &Connection,
        id: &str,
        status: NoteStatus,
        trash_path: Option<&str>,
    ) -> Result<()>;
    fn delete(&self, conn: &Connection, id: &str) -> Result<()>;
    fn count_for_category(&self, conn: &Connection, category_id: &str) -> Result<i64>;
    fn count_by_status(
        &self,
        conn: &Connection,
        workspace_id: &str,
        status: NoteStatus,
    ) -> Result<i64>;
}

pub struct SqliteNoteRepository;

const NOTE_COLUMNS: &str = "id, workspace_id, title, file_path, category_id, status, content_hash,
     word_count, favorite, created_at, updated_at, last_indexed_at,
     source_type, source_ref, source_reliability, valid_until, file_size, file_mtime";

fn map_note(row: &Row<'_>) -> rusqlite::Result<Note> {
    let status: String = row.get("status")?;
    Ok(Note {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        title: row.get("title")?,
        file_path: row.get("file_path")?,
        category_id: row.get("category_id")?,
        status: NoteStatus::from_str(&status),
        content_hash: row.get("content_hash")?,
        word_count: row.get("word_count")?,
        favorite: row.get::<_, i64>("favorite")? != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        last_indexed_at: row.get("last_indexed_at")?,
        source_type: row.get("source_type")?,
        source_ref: row.get("source_ref")?,
        source_reliability: row.get("source_reliability")?,
        valid_until: row.get("valid_until")?,
        file_size: row.get("file_size")?,
        file_mtime: row.get("file_mtime")?,
    })
}

impl NoteRepository for SqliteNoteRepository {
    fn insert(&self, conn: &Connection, note: &Note) -> Result<()> {
        conn.execute(
            "INSERT INTO notes (id, workspace_id, title, file_path, category_id, status,
                                content_hash, word_count, favorite, created_at, updated_at,
                                last_indexed_at, source_type, source_ref, source_reliability,
                                valid_until, file_size, file_mtime)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                     ?17, ?18)",
            params![
                note.id,
                note.workspace_id,
                note.title,
                note.file_path,
                note.category_id,
                note.status.as_str(),
                note.content_hash,
                note.word_count,
                i64::from(note.favorite),
                note.created_at,
                note.updated_at,
                note.last_indexed_at,
                note.source_type,
                note.source_ref,
                note.source_reliability,
                note.valid_until,
                note.file_size,
                note.file_mtime,
            ],
        )?;
        Ok(())
    }

    fn update(&self, conn: &Connection, note: &Note) -> Result<()> {
        conn.execute(
            "UPDATE notes SET title = ?2, file_path = ?3, category_id = ?4, status = ?5,
                              content_hash = ?6, word_count = ?7, favorite = ?8,
                              source_type = ?13, source_ref = ?14,
                              source_reliability = ?15, valid_until = ?16,
                              file_size = ?17, file_mtime = ?18,
                              created_at = ?9, updated_at = ?10, last_indexed_at = ?11
             WHERE id = ?1",
            params![
                note.id,
                note.title,
                note.file_path,
                note.category_id,
                note.status.as_str(),
                note.content_hash,
                note.word_count,
                i64::from(note.favorite),
                note.created_at,
                note.updated_at,
                note.last_indexed_at,
                note.source_type,
                note.source_ref,
                note.source_reliability,
                note.valid_until,
                note.file_size,
                note.file_mtime,
            ],
        )?;
        Ok(())
    }

    fn get(&self, conn: &Connection, id: &str) -> Result<Option<Note>> {
        let sql = format!("SELECT {NOTE_COLUMNS} FROM notes WHERE id = ?1");
        let mut statement = conn.prepare(&sql)?;
        let mut rows = statement.query_map(params![id], map_note)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn get_by_path(
        &self,
        conn: &Connection,
        workspace_id: &str,
        file_path: &str,
    ) -> Result<Option<Note>> {
        let sql =
            format!("SELECT {NOTE_COLUMNS} FROM notes WHERE workspace_id = ?1 AND file_path = ?2");
        let mut statement = conn.prepare(&sql)?;
        let mut rows = statement.query_map(params![workspace_id, file_path], map_note)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn list(
        &self,
        conn: &Connection,
        workspace_id: &str,
        query: &NoteListQuery,
    ) -> Result<(Vec<Note>, i64)> {
        let mut params: Vec<Value> = vec![Value::Text(workspace_id.to_string())];
        let mut conditions = vec!["n.workspace_id = ?".to_string()];

        let status = query.status.unwrap_or(NoteStatus::Active);
        conditions.push("n.status = ?".to_string());
        params.push(Value::Text(status.as_str().to_string()));

        if let Some(category_id) = &query.category_id {
            // 选中分类时同时包含其子分类
            conditions.push(
                "(n.category_id = ? OR n.category_id IN (
                     WITH RECURSIVE sub(id) AS (
                         SELECT id FROM categories WHERE id = ?
                         UNION ALL
                         SELECT c.id FROM categories c JOIN sub ON c.parent_id = sub.id
                     ) SELECT id FROM sub))"
                    .to_string(),
            );
            params.push(Value::Text(category_id.clone()));
            params.push(Value::Text(category_id.clone()));
        }
        if query.inbox_only {
            conditions.push("n.category_id IS NULL".to_string());
        }
        if query.favorite_only {
            conditions.push("n.favorite = 1".to_string());
        }
        if let Some(tag_id) = &query.tag_id {
            conditions.push(
                "EXISTS (SELECT 1 FROM note_tags nt WHERE nt.note_id = n.id AND nt.tag_id = ?)"
                    .to_string(),
            );
            params.push(Value::Text(tag_id.clone()));
        }
        if let Some(keyword) = &query.keyword {
            let trimmed = keyword.trim();
            if !trimmed.is_empty() {
                conditions.push("n.title LIKE ? ESCAPE '\\'".to_string());
                let escaped = trimmed
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_");
                params.push(Value::Text(format!("%{escaped}%")));
            }
        }

        let where_clause = conditions.join(" AND ");
        let count_sql = format!("SELECT COUNT(*) FROM notes n WHERE {where_clause}");
        let mut statement = conn.prepare(&count_sql)?;
        let total: i64 = statement.query_row(params_from_iter(params.iter()), |row| row.get(0))?;

        let order = query.sort.unwrap_or(NoteSort::UpdatedDesc).order_by();
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(200).clamp(1, 500);
        let offset = (page - 1) * page_size;

        let sql = format!(
            "SELECT {NOTE_COLUMNS} FROM notes n WHERE {where_clause} ORDER BY {order} LIMIT ? OFFSET ?"
        );
        let mut list_params = params.clone();
        list_params.push(Value::Integer(page_size));
        list_params.push(Value::Integer(offset));

        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(list_params.iter()), map_note)?;
        let mut notes = Vec::new();
        for row in rows {
            notes.push(row?);
        }
        Ok((notes, total))
    }

    fn list_by_status(
        &self,
        conn: &Connection,
        workspace_id: &str,
        status: NoteStatus,
    ) -> Result<Vec<Note>> {
        let sql = format!(
            "SELECT {NOTE_COLUMNS} FROM notes WHERE workspace_id = ?1 AND status = ?2
             ORDER BY updated_at DESC"
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params![workspace_id, status.as_str()], map_note)?;
        let mut notes = Vec::new();
        for row in rows {
            notes.push(row?);
        }
        Ok(notes)
    }

    fn update_content_meta(
        &self,
        conn: &Connection,
        id: &str,
        updated_at: &str,
        content_hash: &str,
        word_count: i64,
        last_indexed_at: Option<&str>,
    ) -> Result<()> {
        // 传入 NULL 表示保留原有索引时间
        conn.execute(
            "UPDATE notes
             SET updated_at = ?2,
                 content_hash = ?3,
                 word_count = ?4,
                 last_indexed_at = COALESCE(?5, last_indexed_at)
             WHERE id = ?1",
            params![id, updated_at, content_hash, word_count, last_indexed_at],
        )?;
        Ok(())
    }

    fn update_identity(
        &self,
        conn: &Connection,
        id: &str,
        title: &str,
        file_path: &str,
        category_id: Option<&str>,
        updated_at: &str,
    ) -> Result<()> {
        conn.execute(
            "UPDATE notes
             SET title = ?2, file_path = ?3, category_id = ?4, updated_at = ?5
             WHERE id = ?1",
            params![id, title, file_path, category_id, updated_at],
        )?;
        Ok(())
    }

    fn set_category(&self, conn: &Connection, id: &str, category_id: Option<&str>) -> Result<()> {
        conn.execute(
            "UPDATE notes SET category_id = ?2 WHERE id = ?1",
            params![id, category_id],
        )?;
        Ok(())
    }

    fn set_file_stamp(
        &self,
        conn: &Connection,
        id: &str,
        file_size: Option<i64>,
        file_mtime: Option<&str>,
    ) -> Result<()> {
        conn.execute(
            "UPDATE notes SET file_size = ?2, file_mtime = ?3 WHERE id = ?1",
            params![id, file_size, file_mtime],
        )?;
        Ok(())
    }

    fn set_source(
        &self,
        conn: &Connection,
        id: &str,
        source_type: Option<&str>,
        source_ref: Option<&str>,
        source_reliability: Option<i64>,
        valid_until: Option<&str>,
    ) -> Result<()> {
        conn.execute(
            "UPDATE notes SET source_type = ?2, source_ref = ?3, source_reliability = ?4,
                              valid_until = ?5
             WHERE id = ?1",
            params![id, source_type, source_ref, source_reliability, valid_until],
        )?;
        Ok(())
    }

    fn set_favorite(&self, conn: &Connection, id: &str, favorite: bool) -> Result<()> {
        conn.execute(
            "UPDATE notes SET favorite = ?2 WHERE id = ?1",
            params![id, i64::from(favorite)],
        )?;
        Ok(())
    }

    fn set_status(
        &self,
        conn: &Connection,
        id: &str,
        status: NoteStatus,
        trash_path: Option<&str>,
    ) -> Result<()> {
        conn.execute(
            "UPDATE notes SET status = ?2, trash_path = ?3 WHERE id = ?1",
            params![id, status.as_str(), trash_path],
        )?;
        Ok(())
    }

    fn delete(&self, conn: &Connection, id: &str) -> Result<()> {
        conn.execute("DELETE FROM notes WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn count_for_category(&self, conn: &Connection, category_id: &str) -> Result<i64> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE category_id = ?1 AND status = 'active'",
            params![category_id],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    fn count_by_status(
        &self,
        conn: &Connection,
        workspace_id: &str,
        status: NoteStatus,
    ) -> Result<i64> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM notes WHERE workspace_id = ?1 AND status = ?2",
            params![workspace_id, status.as_str()],
            |row| row.get(0),
        )?;
        Ok(count)
    }
}

/// 读取笔记的回收站路径。
pub fn trash_path_of(conn: &Connection, id: &str) -> Result<Option<String>> {
    let mut statement = conn.prepare("SELECT trash_path FROM notes WHERE id = ?1")?;
    let mut rows = statement.query_map(params![id], |row| row.get::<_, Option<String>>(0))?;
    match rows.next() {
        Some(row) => Ok(row?),
        None => Ok(None),
    }
}
