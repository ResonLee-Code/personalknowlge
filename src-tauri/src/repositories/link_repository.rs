//! 双向链接仓储（`note_links` 表）。

use rusqlite::{params, Connection, Row};

use crate::core::error::Result;
use crate::domain::link::LinkType;

/// 一行链接记录。
#[derive(Debug, Clone)]
pub struct NoteLinkRow {
    pub id: i64,
    pub source_note_id: String,
    pub target_note_id: Option<String>,
    pub target_title: String,
    pub target_anchor: Option<String>,
    pub link_type: LinkType,
    pub context: String,
    pub position: i64,
}

/// 解析目标时需要的笔记索引信息。
#[derive(Debug, Clone)]
pub struct NoteTitleRow {
    pub id: String,
    pub title: String,
    pub file_path: String,
    pub category_id: Option<String>,
    pub updated_at: String,
}

pub trait LinkRepository {
    fn delete_for_source(&self, conn: &Connection, source_note_id: &str) -> Result<()>;
    fn insert(
        &self,
        conn: &Connection,
        workspace_id: &str,
        source_note_id: &str,
        row: &NoteLinkRow,
    ) -> Result<()>;
    fn list_for_source(&self, conn: &Connection, source_note_id: &str) -> Result<Vec<NoteLinkRow>>;
    fn list_for_target(&self, conn: &Connection, target_note_id: &str) -> Result<Vec<NoteLinkRow>>;
    fn list_unresolved(&self, conn: &Connection, workspace_id: &str) -> Result<Vec<NoteLinkRow>>;
    /// 全库**已解析**的链接（不含外链），用于知识图谱等派生视图。
    fn list_resolved(&self, conn: &Connection, workspace_id: &str) -> Result<Vec<NoteLinkRow>>;
    fn list_titles(&self, conn: &Connection, workspace_id: &str) -> Result<Vec<NoteTitleRow>>;
    /// 图谱专用：只取 `(source, target, link_type)`，跳过标题等无用列（大库下省一半以上分配）
    fn list_resolved_pairs(
        &self,
        conn: &Connection,
        workspace_id: &str,
    ) -> Result<Vec<(String, String, String)>>;
    fn clear_workspace(&self, conn: &Connection, workspace_id: &str) -> Result<()>;
    fn unbind_target(&self, conn: &Connection, target_note_id: &str) -> Result<usize>;
    fn rebind_target(
        &self,
        conn: &Connection,
        workspace_id: &str,
        title: &str,
        note_id: &str,
    ) -> Result<usize>;
    fn rename_target(
        &self,
        conn: &Connection,
        note_id: &str,
        new_title: &str,
    ) -> Result<usize>;
    fn counts(&self, conn: &Connection, note_id: &str) -> Result<(i64, i64, i64)>;
}

pub struct SqliteLinkRepository;

const COLUMNS: &str =
    "id, source_note_id, target_note_id, target_title, target_anchor, link_type, context, position";

fn map_row(row: &Row<'_>) -> rusqlite::Result<NoteLinkRow> {
    let link_type: String = row.get("link_type")?;
    Ok(NoteLinkRow {
        id: row.get("id")?,
        source_note_id: row.get("source_note_id")?,
        target_note_id: row.get("target_note_id")?,
        target_title: row.get("target_title")?,
        target_anchor: row.get("target_anchor")?,
        link_type: LinkType::from_str(&link_type),
        context: row.get::<_, Option<String>>("context")?.unwrap_or_default(),
        position: row.get("position")?,
    })
}

impl LinkRepository for SqliteLinkRepository {
    fn delete_for_source(&self, conn: &Connection, source_note_id: &str) -> Result<()> {
        conn.execute(
            "DELETE FROM note_links WHERE source_note_id = ?1",
            params![source_note_id],
        )?;
        Ok(())
    }

    fn insert(
        &self,
        conn: &Connection,
        workspace_id: &str,
        source_note_id: &str,
        row: &NoteLinkRow,
    ) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO note_links
                (workspace_id, source_note_id, target_note_id, target_title, target_anchor,
                 link_type, context, position, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                workspace_id,
                source_note_id,
                row.target_note_id,
                row.target_title,
                row.target_anchor,
                row.link_type.as_str(),
                row.context,
                row.position,
                crate::core::time::now_iso(),
            ],
        )?;
        Ok(())
    }

    fn list_for_source(&self, conn: &Connection, source_note_id: &str) -> Result<Vec<NoteLinkRow>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM note_links WHERE source_note_id = ?1 ORDER BY position ASC"
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params![source_note_id], map_row)?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    fn list_for_target(&self, conn: &Connection, target_note_id: &str) -> Result<Vec<NoteLinkRow>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM note_links WHERE target_note_id = ?1
             ORDER BY source_note_id ASC, position ASC"
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params![target_note_id], map_row)?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    fn list_unresolved(&self, conn: &Connection, workspace_id: &str) -> Result<Vec<NoteLinkRow>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM note_links
             WHERE workspace_id = ?1 AND target_note_id IS NULL AND link_type <> 'url'
             ORDER BY source_note_id ASC, position ASC"
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params![workspace_id], map_row)?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    fn list_resolved(&self, conn: &Connection, workspace_id: &str) -> Result<Vec<NoteLinkRow>> {
        let sql = format!(
            "SELECT {COLUMNS} FROM note_links
             WHERE workspace_id = ?1 AND target_note_id IS NOT NULL AND link_type <> 'url'
             ORDER BY source_note_id ASC, position ASC"
        );
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map(params![workspace_id], map_row)?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    fn list_resolved_pairs(
        &self,
        conn: &Connection,
        workspace_id: &str,
    ) -> Result<Vec<(String, String, String)>> {
        let mut statement = conn.prepare(
            "SELECT source_note_id, target_note_id, link_type FROM note_links
             WHERE workspace_id = ?1 AND target_note_id IS NOT NULL AND link_type <> 'url'
             ORDER BY source_note_id ASC, position ASC",
        )?;
        let rows = statement.query_map(params![workspace_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut pairs = Vec::new();
        for row in rows {
            pairs.push(row?);
        }
        Ok(pairs)
    }

    fn list_titles(&self, conn: &Connection, workspace_id: &str) -> Result<Vec<NoteTitleRow>> {
        let mut statement = conn.prepare(
            "SELECT id, title, file_path, category_id, updated_at FROM notes
             WHERE workspace_id = ?1 AND status = 'active'",
        )?;
        let rows = statement.query_map(params![workspace_id], |row| {
            Ok(NoteTitleRow {
                id: row.get(0)?,
                title: row.get(1)?,
                file_path: row.get(2)?,
                category_id: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        let mut list = Vec::new();
        for row in rows {
            list.push(row?);
        }
        Ok(list)
    }

    fn clear_workspace(&self, conn: &Connection, workspace_id: &str) -> Result<()> {
        conn.execute(
            "DELETE FROM note_links WHERE workspace_id = ?1",
            params![workspace_id],
        )?;
        Ok(())
    }

    fn unbind_target(&self, conn: &Connection, target_note_id: &str) -> Result<usize> {
        let affected = conn.execute(
            "UPDATE note_links SET target_note_id = NULL WHERE target_note_id = ?1",
            params![target_note_id],
        )?;
        Ok(affected)
    }

    fn rebind_target(
        &self,
        conn: &Connection,
        workspace_id: &str,
        title: &str,
        note_id: &str,
    ) -> Result<usize> {
        let affected = conn.execute(
            "UPDATE note_links SET target_note_id = ?3
             WHERE workspace_id = ?1 AND lower(target_title) = lower(?2)
               AND (target_note_id IS NULL OR target_note_id <> ?3)",
            params![workspace_id, title, note_id],
        )?;
        Ok(affected)
    }

    fn rename_target(&self, conn: &Connection, note_id: &str, new_title: &str) -> Result<usize> {
        let affected = conn.execute(
            "UPDATE note_links SET target_title = ?2, target_note_id = ?1
             WHERE target_note_id = ?1",
            params![note_id, new_title],
        )?;
        Ok(affected)
    }

    fn counts(&self, conn: &Connection, note_id: &str) -> Result<(i64, i64, i64)> {
        let backlinks: i64 = conn.query_row(
            "SELECT COUNT(*) FROM note_links WHERE target_note_id = ?1",
            params![note_id],
            |row| row.get(0),
        )?;
        let outgoing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM note_links WHERE source_note_id = ?1 AND link_type <> 'url'",
            params![note_id],
            |row| row.get(0),
        )?;
        let unresolved: i64 = conn.query_row(
            "SELECT COUNT(*) FROM note_links
             WHERE source_note_id = ?1 AND target_note_id IS NULL AND link_type <> 'url'",
            params![note_id],
            |row| row.get(0),
        )?;
        Ok((backlinks, outgoing, unresolved))
    }
}
