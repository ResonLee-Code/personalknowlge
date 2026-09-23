//! 标签仓储。

use std::collections::HashMap;

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, Row};

use crate::core::error::Result;
use crate::domain::tag::Tag;

pub trait TagRepository {
    fn insert(&self, conn: &Connection, tag: &Tag) -> Result<()>;
    fn find_by_key(&self, conn: &Connection, workspace_id: &str, key: &str)
        -> Result<Option<Tag>>;
    fn list(&self, conn: &Connection, workspace_id: &str) -> Result<Vec<Tag>>;
    fn get(&self, conn: &Connection, id: &str) -> Result<Option<Tag>>;
    fn delete(&self, conn: &Connection, id: &str) -> Result<()>;
    fn set_note_tags(&self, conn: &Connection, note_id: &str, tag_ids: &[String]) -> Result<()>;
}

pub struct SqliteTagRepository;

const COLUMNS: &str = "t.id, t.workspace_id, t.name, t.created_at";

fn map_tag(row: &Row<'_>) -> rusqlite::Result<Tag> {
    Ok(Tag {
        id: row.get("id")?,
        workspace_id: row.get("workspace_id")?,
        name: row.get("name")?,
        created_at: row.get("created_at")?,
        note_count: row.get::<_, Option<i64>>("note_count")?.unwrap_or(0),
    })
}

impl TagRepository for SqliteTagRepository {
    fn insert(&self, conn: &Connection, tag: &Tag) -> Result<()> {
        conn.execute(
            "INSERT INTO tags (id, workspace_id, name, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![tag.id, tag.workspace_id, tag.name, tag.created_at],
        )?;
        Ok(())
    }

    fn find_by_key(
        &self,
        conn: &Connection,
        workspace_id: &str,
        key: &str,
    ) -> Result<Option<Tag>> {
        let sql = format!(
            "SELECT {COLUMNS}, 0 AS note_count FROM tags t
             WHERE t.workspace_id = ?1 AND lower(t.name) = ?2"
        );
        let mut statement = conn.prepare(&sql)?;
        let mut rows = statement.query_map(params![workspace_id, key], map_tag)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn list(&self, conn: &Connection, workspace_id: &str) -> Result<Vec<Tag>> {
        let sql = format!(
            "SELECT {COLUMNS},
                    (SELECT COUNT(*) FROM note_tags nt
                     JOIN notes n ON n.id = nt.note_id
                     WHERE nt.tag_id = t.id AND n.status = 'active') AS note_count
             FROM tags t
             WHERE t.workspace_id = ?1
             ORDER BY note_count DESC, t.name COLLATE NOCASE ASC"
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params![workspace_id], map_tag)?;
        let mut tags = Vec::new();
        for row in rows {
            tags.push(row?);
        }
        Ok(tags)
    }

    fn get(&self, conn: &Connection, id: &str) -> Result<Option<Tag>> {
        let sql = format!(
            "SELECT {COLUMNS},
                    (SELECT COUNT(*) FROM note_tags nt WHERE nt.tag_id = t.id) AS note_count
             FROM tags t WHERE t.id = ?1"
        );
        let mut statement = conn.prepare(&sql)?;
        let mut rows = statement.query_map(params![id], map_tag)?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    fn delete(&self, conn: &Connection, id: &str) -> Result<()> {
        conn.execute("DELETE FROM tags WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn set_note_tags(&self, conn: &Connection, note_id: &str, tag_ids: &[String]) -> Result<()> {
        conn.execute("DELETE FROM note_tags WHERE note_id = ?1", params![note_id])?;
        for tag_id in tag_ids {
            conn.execute(
                "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, tag_id],
            )?;
        }
        Ok(())
    }
}

/// 读取一篇笔记的标签。
pub fn tags_for_note(conn: &Connection, note_id: &str) -> Result<Vec<Tag>> {
    let sql = format!(
        "SELECT {COLUMNS}, 0 AS note_count FROM tags t
         JOIN note_tags nt ON nt.tag_id = t.id
         WHERE nt.note_id = ?1
         ORDER BY t.name COLLATE NOCASE ASC"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(params![note_id], map_tag)?;
    let mut tags = Vec::new();
    for row in rows {
        tags.push(row?);
    }
    Ok(tags)
}

/// 批量读取多篇笔记的标签名。
pub fn tag_names_for_notes(
    conn: &Connection,
    note_ids: &[String],
) -> Result<HashMap<String, Vec<String>>> {
    let mut result: HashMap<String, Vec<String>> = HashMap::new();
    if note_ids.is_empty() {
        return Ok(result);
    }
    let placeholders = vec!["?"; note_ids.len()].join(", ");
    let sql = format!(
        "SELECT nt.note_id, t.name FROM note_tags nt
         JOIN tags t ON t.id = nt.tag_id
         WHERE nt.note_id IN ({placeholders})
         ORDER BY t.name COLLATE NOCASE ASC"
    );
    let values: Vec<Value> = note_ids.iter().cloned().map(Value::Text).collect();
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(params_from_iter(values.iter()), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (note_id, name) = row?;
        result.entry(note_id).or_default().push(name);
    }
    Ok(result)
}
